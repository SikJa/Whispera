import Foundation
import CoreAudio
import AudioToolbox

private struct WhisperaAudioControl: Codable {
    let kind: String
    let channel: UInt32
    let value: Double
}
private struct WhisperaAudioDevice: Codable {
    let uid: String
    let controls: [WhisperaAudioControl]
}
private enum WhisperaAudioFailure: Int32, Error { case noOutput = 1, unsupported = 2, hardware = 3, invalid = 4, missing = 5 }

private func whisperaAudioAddress(_ selector: AudioObjectPropertySelector,
                                  _ scope: AudioObjectPropertyScope = kAudioObjectPropertyScopeOutput,
                                  _ channel: UInt32 = kAudioObjectPropertyElementMain) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress(mSelector: selector, mScope: scope, mElement: channel)
}

private func whisperaAudioUInt(_ device: AudioObjectID, _ property: AudioObjectPropertyAddress) throws -> UInt32 {
    var address = property
    var value: UInt32 = 0
    var size = UInt32(MemoryLayout<UInt32>.size)
    guard AudioObjectGetPropertyData(device, &address, 0, nil, &size, &value) == noErr,
          size == MemoryLayout<UInt32>.size else { throw WhisperaAudioFailure.hardware }
    return value
}

private func whisperaAudioDevices() throws -> [AudioDeviceID] {
    var address = whisperaAudioAddress(kAudioHardwarePropertyDevices, kAudioObjectPropertyScopeGlobal)
    var size: UInt32 = 0
    guard AudioObjectGetPropertyDataSize(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size) == noErr,
          size <= 128 * MemoryLayout<AudioDeviceID>.stride,
          Int(size) % MemoryLayout<AudioDeviceID>.stride == 0 else { throw WhisperaAudioFailure.hardware }
    if size == 0 { return [] }
    let capacity = size
    var devices = [AudioDeviceID](repeating: 0, count: Int(size) / MemoryLayout<AudioDeviceID>.stride)
    let status = devices.withUnsafeMutableBytes { bytes in
        AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, bytes.baseAddress!)
    }
    guard status == noErr, size <= capacity,
          Int(size) % MemoryLayout<AudioDeviceID>.stride == 0 else { throw WhisperaAudioFailure.hardware }
    return Array(devices.prefix(Int(size) / MemoryLayout<AudioDeviceID>.stride))
}

private func whisperaAudioUID(_ device: AudioDeviceID) throws -> String {
    var address = whisperaAudioAddress(kAudioDevicePropertyDeviceUID, kAudioObjectPropertyScopeGlobal)
    // HAL returns an owned CFStringRef through a C out-pointer, not Swift ARC storage.
    var value: Unmanaged<CFString>?
    var size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
    guard AudioObjectGetPropertyData(device, &address, 0, nil, &size, &value) == noErr,
          let value else { throw WhisperaAudioFailure.hardware }
    let uid = value.takeRetainedValue() as String
    guard size == MemoryLayout<Unmanaged<CFString>?>.size, !uid.isEmpty else { throw WhisperaAudioFailure.hardware }
    return uid
}

private func whisperaAudioChannelCount(_ device: AudioDeviceID) throws -> UInt32 {
    var streams = whisperaAudioAddress(kAudioDevicePropertyStreams)
    if !AudioObjectHasProperty(device, &streams) { return 0 }
    var streamBytes: UInt32 = 0
    guard AudioObjectGetPropertyDataSize(device, &streams, 0, nil, &streamBytes) == noErr else { throw WhisperaAudioFailure.hardware }
    if streamBytes == 0 { return 0 }
    var address = whisperaAudioAddress(kAudioDevicePropertyStreamConfiguration)
    var size: UInt32 = 0
    guard AudioObjectGetPropertyDataSize(device, &address, 0, nil, &size) == noErr,
          size >= MemoryLayout<AudioBufferList>.size, size <= 65536 else { throw WhisperaAudioFailure.hardware }
    let capacity = size
    let memory = UnsafeMutableRawPointer.allocate(byteCount: Int(size), alignment: MemoryLayout<AudioBufferList>.alignment)
    defer { memory.deallocate() }
    memory.initializeMemory(as: UInt8.self, repeating: 0, count: Int(size))
    guard AudioObjectGetPropertyData(device, &address, 0, nil, &size, memory) == noErr,
          size >= MemoryLayout<AudioBufferList>.size, size <= capacity,
          let offset = MemoryLayout<AudioBufferList>.offset(of: \.mBuffers) else { throw WhisperaAudioFailure.hardware }
    let header = memory.bindMemory(to: AudioBufferList.self, capacity: 1)
    // Validate the flexible array before its count can drive pointer traversal.
    guard Int(header.pointee.mNumberBuffers) <= (Int(size) - offset) / MemoryLayout<AudioBuffer>.stride else { throw WhisperaAudioFailure.hardware }
    let list = UnsafeMutableAudioBufferListPointer(header)
    var count: UInt32 = 0
    for buffer in list {
        guard buffer.mNumberChannels <= 256 - count else { throw WhisperaAudioFailure.unsupported }
        count += buffer.mNumberChannels
    }
    return count
}

private func whisperaAudioCanSet(_ device: AudioDeviceID, _ selector: AudioObjectPropertySelector, _ channel: UInt32) -> Bool {
    var address = whisperaAudioAddress(selector, kAudioObjectPropertyScopeOutput, channel)
    var settable = DarwinBoolean(false)
    return AudioObjectHasProperty(device, &address)
        && AudioObjectIsPropertySettable(device, &address, &settable) == noErr && settable.boolValue
}

private func whisperaAudioReadControl(_ device: AudioDeviceID, _ kind: String, _ channel: UInt32) throws -> WhisperaAudioControl {
    if kind == "mute" {
        let value = try whisperaAudioUInt(device, whisperaAudioAddress(kAudioDevicePropertyMute, kAudioObjectPropertyScopeOutput, channel))
        return WhisperaAudioControl(kind: kind, channel: channel, value: value == 0 ? 0 : 1)
    }
    var address = whisperaAudioAddress(kAudioDevicePropertyVolumeScalar, kAudioObjectPropertyScopeOutput, channel)
    var value: Float32 = 0
    var size = UInt32(MemoryLayout<Float32>.size)
    guard AudioObjectGetPropertyData(device, &address, 0, nil, &size, &value) == noErr,
          size == MemoryLayout<Float32>.size,
          value.isFinite, value >= 0, value <= 1 else { throw WhisperaAudioFailure.hardware }
    return WhisperaAudioControl(kind: kind, channel: channel, value: Double(value))
}

private func whisperaAudioSnapshot() throws -> [WhisperaAudioDevice] {
    var result: [WhisperaAudioDevice] = []
    for device in try whisperaAudioDevices() {
        guard try whisperaAudioUInt(device, whisperaAudioAddress(kAudioDevicePropertyDeviceIsAlive, kAudioObjectPropertyScopeGlobal)) != 0 else { continue }
        let channels = try whisperaAudioChannelCount(device)
        if channels == 0 { continue }
        var controls: [WhisperaAudioControl] = []
        if whisperaAudioCanSet(device, kAudioDevicePropertyMute, 0) {
            controls = [try whisperaAudioReadControl(device, "mute", 0)]
        } else if whisperaAudioCanSet(device, kAudioDevicePropertyVolumeScalar, 0) {
            controls = [try whisperaAudioReadControl(device, "volume", 0)]
        } else {
            for channel in 1...channels {
                if whisperaAudioCanSet(device, kAudioDevicePropertyMute, channel) {
                    controls.append(try whisperaAudioReadControl(device, "mute", channel))
                } else if whisperaAudioCanSet(device, kAudioDevicePropertyVolumeScalar, channel) {
                    controls.append(try whisperaAudioReadControl(device, "volume", channel))
                } else { throw WhisperaAudioFailure.unsupported }
            }
        }
        result.append(WhisperaAudioDevice(uid: try whisperaAudioUID(device), controls: controls))
    }
    guard !result.isEmpty else { throw WhisperaAudioFailure.noOutput }
    return result
}

@_cdecl("whispera_system_audio_snapshot")
func whisperaSystemAudioSnapshot(_ buffer: UnsafeMutablePointer<UInt8>?, _ capacity: Int) -> Int {
    guard let buffer, capacity > 0 else { return -4 }
    return autoreleasepool {
        do {
            let data = try JSONEncoder().encode(whisperaAudioSnapshot())
            guard data.count <= capacity else { return -4 }
            data.copyBytes(to: buffer, count: data.count)
            return data.count
        } catch let error as WhisperaAudioFailure { return -Int(error.rawValue) }
        catch { return -3 }
    }
}

private func whisperaAudioSetControl(_ device: AudioDeviceID, _ control: WhisperaAudioControl, _ restore: Bool) throws {
    guard ["mute", "volume"].contains(control.kind), control.channel <= 256,
          control.value.isFinite, (0...1).contains(control.value),
          control.kind != "mute" || control.value == 0 || control.value == 1 else { throw WhisperaAudioFailure.invalid }
    let mute = control.kind == "mute"
    let selector = mute ? kAudioDevicePropertyMute : kAudioDevicePropertyVolumeScalar
    guard whisperaAudioCanSet(device, selector, control.channel) else { throw WhisperaAudioFailure.unsupported }
    var address = whisperaAudioAddress(selector, kAudioObjectPropertyScopeOutput, control.channel)
    let expected = restore ? control.value : (mute ? 1 : 0)
    let status: OSStatus
    if mute {
        var value = UInt32(expected)
        status = AudioObjectSetPropertyData(device, &address, 0, nil, UInt32(MemoryLayout<UInt32>.size), &value)
    } else {
        var value = Float32(expected)
        status = AudioObjectSetPropertyData(device, &address, 0, nil, UInt32(MemoryLayout<Float32>.size), &value)
    }
    guard status == noErr else { throw WhisperaAudioFailure.hardware }
    for _ in 0..<5 {
        let actual = try whisperaAudioReadControl(device, control.kind, control.channel)
        if abs(actual.value - expected) <= 0.0001 { return }
        Thread.sleep(forTimeInterval: 0.02)
    }
    throw WhisperaAudioFailure.hardware
}

@_cdecl("whispera_system_audio_set")
func whisperaSystemAudioSet(_ bytes: UnsafePointer<UInt8>?, _ length: Int, _ restore: Int32) -> Int32 {
    guard let bytes, length > 0, length <= 1024 * 1024 else { return 4 }
    return autoreleasepool {
        do {
            let original = try JSONDecoder().decode(WhisperaAudioDevice.self, from: Data(bytes: bytes, count: length))
            guard !original.uid.isEmpty, !original.controls.isEmpty, original.controls.count <= 256 else { return 4 }
            // AudioObjectIDs are ephemeral. Recover by stable UID even after reboot/hotplug.
            var found: AudioDeviceID?
            for device in try whisperaAudioDevices() {
                if (try? whisperaAudioUID(device)) == original.uid { found = device; break }
            }
            guard let device = found else { return 5 }
            var failure: WhisperaAudioFailure?
            for control in original.controls {
                do { try whisperaAudioSetControl(device, control, restore != 0) }
                catch let error as WhisperaAudioFailure { failure = error }
                catch { failure = .hardware }
            }
            return failure?.rawValue ?? 0
        } catch let error as WhisperaAudioFailure { return error.rawValue }
        catch { return 4 }
    }
}
