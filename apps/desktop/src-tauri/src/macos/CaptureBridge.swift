// macOS 13+, compiled as part of the shared static library (no executable entry).
// APIs: developer.apple.com/documentation/screencapturekit and /avfaudio/avaudioengine
import AppKit
import AVFoundation
import AudioToolbox
import CoreMedia
import CoreVideo
import Darwin
import ScreenCaptureKit

typealias CaptureCallback = @convention(c) (UnsafeMutableRawPointer?, Int32, UnsafePointer<UInt8>?, Int, Double) -> Void

private struct CaptureConfiguration: Decodable {
    let mode: String
    let display: UInt32
    let x: Double
    let y: Double
    let width: Double
    let height: Double
    let output_width: Int
    let output_height: Int
    let ink: Bool
    let cursor: Bool
}
private struct CaptureFailure: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}
private final class CaptureReply<T> {
    private let lock = NSLock()
    private var value: Result<T, Error>?
    private var expired = false
    private let semaphore = DispatchSemaphore(value: 0)
    @discardableResult
    func finish(_ result: Result<T, Error>) -> Bool {
        lock.lock()
        guard !expired, value == nil else { lock.unlock(); return false }
        value = result
        lock.unlock()
        semaphore.signal()
        return true
    }
    func wait(_ seconds: Double = 10) throws -> T {
        guard semaphore.wait(timeout: .now() + seconds) == .success else {
            lock.lock(); expired = true; lock.unlock()
            throw CaptureFailure(message: "macOS no respondio a tiempo a la captura")
        }
        lock.lock(); let result = value; lock.unlock()
        guard let result = result else { throw CaptureFailure(message: "Respuesta de captura vacia") }
        return try result.get()
    }
}
private func captureOnMain<T>(_ body: () -> T) -> T {
    if Thread.isMainThread { return body() }
    return DispatchQueue.main.sync(execute: body)
}
private final class WeakCapture {
    weak var value: NativeCapture?
    init(_ value: NativeCapture) { self.value = value }
}
private enum CaptureRegistry {
    static let lock = NSLock()
    static var ink = Set<CGWindowID>()
    static var streams: [WeakCapture] = []
    // Main-thread-only objects. Timers never activate or focus their windows.
    static var borders: [Int: Timer] = [:]
    static func allowedInk() -> Set<CGWindowID> { lock.lock(); defer { lock.unlock() }; return ink }
    static func add(_ value: NativeCapture) {
        lock.lock(); streams.removeAll { $0.value == nil }; streams.append(WeakCapture(value)); lock.unlock()
    }
    static func setInk(_ id: CGWindowID, enabled: Bool) {
        lock.lock()
        if enabled { ink.insert(id) } else { ink.remove(id) }
        let current = streams.compactMap { $0.value }
        lock.unlock()
        for stream in current { stream.refreshFilter() }
    }
}

private final class NativeCapture: NSObject, SCStreamOutput, SCStreamDelegate {
    let config: CaptureConfiguration
    let callback: CaptureCallback
    let context: UnsafeMutableRawPointer?
    let queue = DispatchQueue(label: "app.whispera.capture.samples", qos: .userInitiated)
    private let queueKey = DispatchSpecificKey<Bool>()
    private let microphoneSlots = DispatchSemaphore(value: 4)
    private let microphoneFailure = DispatchSemaphore(value: 1)
    var stream: SCStream?
    var display: SCDisplay?
    var engine: AVAudioEngine?
    var engineObserver: NSObjectProtocol?
    // Access these only on queue. stop disarms and drains it before Rust frees context.
    var enabled = false
    var converter: AVAudioConverter?
    var inputFormat: AVAudioFormat?
    let audioFormat = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 48_000, channels: 2, interleaved: true)!

    init(_ config: CaptureConfiguration, _ callback: @escaping CaptureCallback, _ context: UnsafeMutableRawPointer?) {
        self.config = config; self.callback = callback; self.context = context
        super.init()
        queue.setSpecific(key: queueKey, value: true)
    }
    private func onSamples<T>(_ body: () -> T) -> T {
        if DispatchQueue.getSpecific(key: queueKey) == true { return body() }
        return queue.sync(execute: body)
    }
    func shareableContent() throws -> SCShareableContent {
        let reply = CaptureReply<SCShareableContent>()
        SCShareableContent.getExcludingDesktopWindows(false, onScreenWindowsOnly: false) { content, error in
            if let content = content { reply.finish(.success(content)) }
            else { reply.finish(.failure(error ?? CaptureFailure(message: "No hay contenido compartible"))) }
        }
        return try reply.wait()
    }
    func filter(_ content: SCShareableContent, _ display: SCDisplay) -> SCContentFilter {
        let own = content.applications.filter { $0.processID == ProcessInfo.processInfo.processIdentifier }
        let ids = config.ink ? CaptureRegistry.allowedInk() : []
        let ink = content.windows.filter { ids.contains($0.windowID) }
        // New HUDs/selectors are excluded from their very first frame, even during
        // replay. Only the explicitly registered primary ink surface is included.
        return SCContentFilter(display: display, excludingApplications: own, exceptingWindows: ink)
    }
    func start() throws {
        if config.mode == "microphone" { try startMicrophone(); return }
        guard CGPreflightScreenCaptureAccess() else {
            throw CaptureFailure(message: "Permiti la pantalla en Configuracion > Aplicacion > Permisos de macOS y volve a abrir Whispera")
        }
        let content = try shareableContent()
        let id = config.display == 0 ? CGMainDisplayID() : config.display
        guard let display = content.displays.first(where: { $0.displayID == id }) else {
            throw CaptureFailure(message: "La pantalla elegida ya no esta conectada")
        }
        onSamples { self.display = display }
        let settings = SCStreamConfiguration()
        settings.width = config.output_width; settings.height = config.output_height
        settings.pixelFormat = kCVPixelFormatType_32BGRA
        settings.colorSpaceName = CGColorSpace.sRGB
        settings.queueDepth = 3
        settings.showsCursor = config.cursor
        settings.minimumFrameInterval = CMTime(value: 1, timescale: config.mode == "system" ? 1 : 30)
        settings.capturesAudio = config.mode == "system"
        settings.excludesCurrentProcessAudio = true
        settings.sampleRate = 48_000; settings.channelCount = 2
        if config.mode != "system" {
            let rect = CGRect(x: config.x, y: config.y, width: config.width, height: config.height)
            let bounds = CGRect(origin: .zero, size: CGDisplayBounds(id).size)
            guard [config.x, config.y, config.width, config.height].allSatisfy({ $0.isFinite }),
                  rect.width > 0, rect.height > 0, bounds.insetBy(dx: -0.5, dy: -0.5).contains(rect),
                  settings.width > 0, settings.height > 0, settings.width <= 8192, settings.height <= 8192 else {
                throw CaptureFailure(message: "El area de captura esta fuera de la pantalla")
            }
            // sourceRect uses display-local points; output size uses backing pixels.
            settings.sourceRect = rect
        }
        let stream = SCStream(filter: filter(content, display), configuration: settings, delegate: self)
        onSamples { self.stream = stream }
        try stream.addStreamOutput(self, type: .screen, sampleHandlerQueue: queue)
        if settings.capturesAudio { try stream.addStreamOutput(self, type: .audio, sampleHandlerQueue: queue) }
        let reply = CaptureReply<Void>()
        onSamples { enabled = true }
        stream.startCapture { error in
            let accepted: Bool
            if let error = error { accepted = reply.finish(.failure(error)) }
            else { accepted = reply.finish(.success(())) }
            // startCapture may finish after the FFI start timed out and stopped.
            // A second stop after that completion prevents an orphaned live stream.
            if !accepted { stream.stopCapture { _ in } }
        }
        try reply.wait()
        CaptureRegistry.add(self)
    }
    func startMicrophone() throws {
        let authorization = AVCaptureDevice.authorizationStatus(for: .audio)
        if authorization != .authorized {
            throw CaptureFailure(message: "Permiti el microfono en Configuracion > Aplicacion > Permisos de macOS antes de grabar")
        }
        let engine = AVAudioEngine()
        let input = engine.inputNode
        let format = input.outputFormat(forBus: 0)
        guard format.sampleRate.isFinite, format.sampleRate > 0, format.channelCount > 0 else { throw CaptureFailure(message: "No hay microfono disponible") }
        onSamples { enabled = true }
        input.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak self] buffer, time in
            guard let self = self else { return }
            // The engine owns/reuses buffer after this tap returns. Copy before
            // dispatching; never block its audio thread on Rust or the sample queue.
            guard self.microphoneSlots.wait(timeout: .now()) == .success else {
                self.microphoneFailed("El microfono no pudo procesarse a tiempo"); return
            }
            guard let copy = self.copyPCM(buffer) else {
                self.microphoneSlots.signal()
                self.microphoneFailed("No se pudo copiar el audio del microfono"); return
            }
            let stamp = time.isHostTimeValid ? AVAudioTime.seconds(forHostTime: time.hostTime) : CMClockGetTime(CMClockGetHostTimeClock()).seconds
            self.queue.async {
                defer { self.microphoneSlots.signal() }
                guard self.enabled else { return }
                self.audio(copy, end: stamp + Double(copy.frameLength) / copy.format.sampleRate)
            }
        }
        self.engine = engine
        engineObserver = NotificationCenter.default.addObserver(forName: .AVAudioEngineConfigurationChange, object: engine, queue: nil) { [weak self] _ in
            self?.queue.async { [weak self] in self?.fail("El microfono cambio o se desconecto; reinicia la captura") }
        }
        try engine.start()
    }
    private func microphoneFailed(_ message: String) {
        if microphoneFailure.wait(timeout: .now()) == .success {
            queue.async { [weak self] in self?.fail(message) }
        }
    }
    private func copyPCM(_ input: AVAudioPCMBuffer) -> AVAudioPCMBuffer? {
        guard input.frameLength > 0,
              let output = AVAudioPCMBuffer(pcmFormat: input.format, frameCapacity: input.frameLength) else { return nil }
        output.frameLength = input.frameLength
        let source = UnsafeMutableAudioBufferListPointer(input.mutableAudioBufferList)
        let target = UnsafeMutableAudioBufferListPointer(output.mutableAudioBufferList)
        guard source.count == target.count else { return nil }
        for index in 0..<source.count {
            guard let from = source[index].mData, let to = target[index].mData,
                  source[index].mDataByteSize <= target[index].mDataByteSize else { return nil }
            memcpy(to, from, Int(source[index].mDataByteSize))
            target[index].mDataByteSize = source[index].mDataByteSize
        }
        return output
    }
    func refreshFilter() {
        guard config.ink else { return }
        SCShareableContent.getExcludingDesktopWindows(false, onScreenWindowsOnly: false) { [weak self] content, error in
            guard let self = self else { return }
            self.queue.async {
                guard self.enabled, let stream = self.stream, let display = self.display else { return }
                guard let content = content else { self.fail(error?.localizedDescription ?? "No se pudo excluir el overlay"); return }
                stream.updateContentFilter(self.filter(content, display)) { [weak self] error in
                    if let error = error { self?.queue.async { [weak self] in self?.fail(error.localizedDescription) } }
                }
            }
        }
    }
    func stop() {
        let stream = onSamples { () -> SCStream? in
            enabled = false
            let current = self.stream
            self.stream = nil; display = nil; converter = nil; inputFormat = nil
            return current
        }
        let engine = self.engine; self.engine = nil
        let observer = engineObserver; engineObserver = nil
        let cleanup = {
            if let observer = observer { NotificationCenter.default.removeObserver(observer) }
            if let engine = engine { engine.inputNode.removeTap(onBus: 0); engine.stop() }
            if let stream = stream {
                let reply = CaptureReply<Void>()
                stream.stopCapture { error in
                    if let error = error { reply.finish(.failure(error)) }
                    else { reply.finish(.success(())) }
                }
                _ = try? reply.wait(5)
                try? stream.removeStreamOutput(self, type: .screen)
                if self.config.mode == "system" { try? stream.removeStreamOutput(self, type: .audio) }
            }
        }
        // Do not wait for framework completions on their own delivery queue or UI.
        // Rust can free context now: every later delivery sees enabled == false.
        if Thread.isMainThread || DispatchQueue.getSpecific(key: queueKey) == true {
            DispatchQueue.global(qos: .userInitiated).async { cleanup() }
        } else { cleanup() }
    }
    func fail(_ message: String) {
        guard enabled else { return }
        enabled = false
        let data = Data(message.utf8)
        data.withUnsafeBytes { callback(context, 2, $0.baseAddress?.assumingMemoryBound(to: UInt8.self), $0.count, 0) }
    }
    func stream(_ stream: SCStream, didStopWithError error: Error) {
        queue.async { [weak self] in self?.fail(error.localizedDescription) }
    }
    func stream(_ stream: SCStream, didOutputSampleBuffer sample: CMSampleBuffer, of type: SCStreamOutputType) {
        guard enabled, sample.isValid, CMSampleBufferDataIsReady(sample) else { return }
        if type == .audio { systemAudio(sample); return }
        guard type == .screen, config.mode != "system",
              let attachments = CMSampleBufferGetSampleAttachmentsArray(sample, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
              let status = attachments.first?[.status] as? Int, status == SCFrameStatus.complete.rawValue,
              let pixels = sample.imageBuffer else { return }
        let width = CVPixelBufferGetWidth(pixels), height = CVPixelBufferGetHeight(pixels)
        guard width == config.output_width, height == config.output_height else { fail("ScreenCaptureKit cambio el tamano de la imagen"); return }
        guard CVPixelBufferLockBaseAddress(pixels, .readOnly) == kCVReturnSuccess else { return }
        defer { CVPixelBufferUnlockBaseAddress(pixels, .readOnly) }
        guard let base = CVPixelBufferGetBaseAddress(pixels) else { return }
        let stride = CVPixelBufferGetBytesPerRow(pixels)
        guard stride >= width * 4 else { fail("ScreenCaptureKit entrego una imagen incompleta"); return }
        if stride == width * 4 {
            callback(context, 0, base.assumingMemoryBound(to: UInt8.self), stride * height, 0)
        } else {
            var packed = Data(count: width * height * 4)
            packed.withUnsafeMutableBytes { target in
                guard let target = target.baseAddress else { return }
                for row in 0..<height { memcpy(target.advanced(by: row * width * 4), base.advanced(by: row * stride), width * 4) }
            }
            packed.withUnsafeBytes { callback(context, 0, $0.baseAddress?.assumingMemoryBound(to: UInt8.self), $0.count, 0) }
        }
    }
    func systemAudio(_ sample: CMSampleBuffer) {
        guard sample.numSamples > 0, sample.numSamples <= Int(Int32.max),
              let description = sample.formatDescription else { return }
        let format = AVAudioFormat(cmAudioFormatDescription: description)
        guard format.sampleRate.isFinite, format.sampleRate > 0 else { fail("Formato de audio del sistema invalido"); return }
        guard let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(sample.numSamples)) else { return }
        buffer.frameLength = buffer.frameCapacity
        let status = CMSampleBufferCopyPCMDataIntoAudioBufferList(sample, at: 0, frameCount: Int32(sample.numSamples), into: buffer.mutableAudioBufferList)
        guard status == noErr else { fail("No se pudo leer el audio del sistema (\(status))"); return }
        let end = sample.presentationTimeStamp.seconds + Double(buffer.frameLength) / format.sampleRate
        audio(buffer, end: end)
    }
    func audio(_ input: AVAudioPCMBuffer, end: Double) {
        guard end.isFinite, input.frameLength > 0,
              input.format.sampleRate.isFinite, input.format.sampleRate > 0 else { fail("Tiempo o formato de audio invalido"); return }
        if inputFormat != input.format {
            converter = AVAudioConverter(from: input.format, to: audioFormat)
            inputFormat = input.format
        }
        let frames = ceil(Double(input.frameLength) * 48_000 / input.format.sampleRate) + 64
        guard frames.isFinite, frames <= Double(UInt32.max) else { fail("Bloque de audio demasiado grande"); return }
        let capacity = AVAudioFrameCount(frames)
        guard let converter = converter, let output = AVAudioPCMBuffer(pcmFormat: audioFormat, frameCapacity: capacity) else { fail("No se pudo convertir el audio"); return }
        var consumed = false
        var error: NSError?
        let status = converter.convert(to: output, error: &error) { _, state in
            if consumed { state.pointee = .noDataNow; return nil }
            consumed = true; state.pointee = .haveData; return input
        }
        guard status != .error else { fail(error?.localizedDescription ?? "Fallo el conversor de audio"); return }
        guard output.frameLength > 0 else { return }
        let buffers = UnsafeMutableAudioBufferListPointer(output.mutableAudioBufferList)
        let bytes = Int(output.frameLength) * 2 * MemoryLayout<Float>.size
        guard buffers.count == 1, Int(buffers[0].mDataByteSize) >= bytes,
              let samples = buffers[0].mData else { fail("El conversor no entrego audio estereo intercalado"); return }
        let age = CMClockGetTime(CMClockGetHostTimeClock()).seconds - end
        callback(context, 1, samples.assumingMemoryBound(to: UInt8.self), bytes, age)
    }
}

@_cdecl("whispera_capture_nonblocking")
func whisperaCaptureNonblocking(_ descriptor: Int32) -> Int32 {
    let flags = fcntl(descriptor, F_GETFL)
    guard flags >= 0 else { return -1 }
    return fcntl(descriptor, F_SETFL, flags | O_NONBLOCK)
}

@_cdecl("whispera_capture_start")
func whisperaCaptureStart(_ json: UnsafePointer<CChar>?, _ callback: @escaping CaptureCallback,
                          _ context: UnsafeMutableRawPointer?, _ error: UnsafeMutablePointer<CChar>?, _ capacity: Int) -> UnsafeMutableRawPointer? {
    do {
        guard !Thread.isMainThread, let json = json else { throw CaptureFailure(message: "La captura debe iniciarse fuera del hilo de UI") }
        let config = try JSONDecoder().decode(CaptureConfiguration.self, from: Data(String(cString: json).utf8))
        let capture = NativeCapture(config, callback, context)
        do { try capture.start() } catch { capture.stop(); throw error }
        return Unmanaged.passRetained(capture).toOpaque()
    } catch let failure {
        if let error = error, capacity > 0 {
            let bytes = Array(failure.localizedDescription.utf8.prefix(capacity - 1))
            for (index, byte) in bytes.enumerated() { error[index] = CChar(bitPattern: byte) }
            error[bytes.count] = 0
        }
        return nil
    }
}
@_cdecl("whispera_capture_stop")
func whisperaCaptureStop(_ handle: UnsafeMutableRawPointer?) {
    guard let handle = handle else { return }
    let capture = Unmanaged<NativeCapture>.fromOpaque(handle).takeRetainedValue()
    capture.stop()
}
@_cdecl("whispera_capture_display")
func whisperaCaptureDisplay(_ window: UnsafeMutableRawPointer?, _ x: Int32, _ y: Int32,
                           _ width: UInt32, _ height: UInt32, _ out: UnsafeMutablePointer<Double>?) -> Int32 {
    guard let out = out else { return 0 }
    // Rust resolves window handles on main immediately before this call.
    guard window == nil || Thread.isMainThread else { return 0 }
    let lookup: () -> [Double]? = {
        let screen: NSScreen?
        if let window = window { screen = Unmanaged<NSWindow>.fromOpaque(window).takeUnretainedValue().screen }
        else {
            screen = NSScreen.screens.first { item in
                guard let number = item.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber else { return false }
                let bounds = CGDisplayBounds(number.uint32Value), scale = item.backingScaleFactor
                return Int32((bounds.minX * scale).rounded()) == x && Int32((bounds.minY * scale).rounded()) == y &&
                    UInt32((bounds.width * scale).rounded()) == width && UInt32((bounds.height * scale).rounded()) == height
            }
        }
        guard let screen = screen, let number = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber else { return nil }
        let bounds = CGDisplayBounds(number.uint32Value), scale = screen.backingScaleFactor
        // Tao scales the global origin per display as well as its size. Preserve
        // the display ID: mixed-DPI physical coordinate ranges can overlap.
        return [Double(number.uint32Value), (bounds.minX * scale).rounded(), (bounds.minY * scale).rounded(), scale]
    }
    let values: [Double]?
    if Thread.isMainThread { values = lookup() }
    else {
        let reply = CaptureReply<[Double]?>()
        DispatchQueue.main.async { reply.finish(.success(lookup())) }
        values = (try? reply.wait(5)) ?? nil
    }
    guard let values = values else { return 0 }
    // A timed-out lookup never writes through the caller's expired out pointer.
    for index in 0..<4 { out[index] = values[index] }
    return 1
}
@_cdecl("whispera_capture_window")
func whisperaCaptureWindow(_ pointer: UnsafeMutableRawPointer?, _ action: Int32, _ x: Double, _ y: Double, _ width: Double, _ height: Double) {
    guard let pointer = pointer else { return }
    captureOnMain {
        let window = Unmanaged<NSWindow>.fromOpaque(pointer).takeUnretainedValue()
        switch action {
        // Stable capture layers: frozen desktop, ink, frame, tools, HUD. The
        // independent still-image session stays above an ongoing video.
        case 20...34: window.level = NSWindow.Level(rawValue: NSWindow.Level.floating.rawValue + Int(action - 20))
        case 0: window.sharingType = .none
        case 1: window.orderFrontRegardless()
        case 2, 3:
            CaptureRegistry.borders.removeValue(forKey: window.windowNumber)?.invalidate()
            window.ignoresMouseEvents = false
            if action == 3 {
                let outer = CGRect(x: x - 8, y: y - 8, width: width + 16, height: height + 16)
                let inner = CGRect(x: x + 8, y: y + 8, width: max(0, width - 16), height: max(0, height - 16))
                let update: (Timer?) -> Void = { [weak window] _ in
                    guard let window = window else { return }
                    let local = window.convertPoint(fromScreen: NSEvent.mouseLocation)
                    let point = CGPoint(x: local.x, y: window.frame.height - local.y)
                    window.ignoresMouseEvents = !(outer.contains(point) && !inner.contains(point))
                }
                update(nil)
                let timer = Timer(timeInterval: 1.0 / 60, repeats: true) { update($0) }
                RunLoop.main.add(timer, forMode: .common)
                CaptureRegistry.borders[window.windowNumber] = timer
            }
        case 4, 5:
            window.sharingType = action == 4 ? .readOnly : .none
            CaptureRegistry.setInk(CGWindowID(window.windowNumber), enabled: action == 4)
        case 6, 7:
            let size = action == 6 ? CGSize(width: width, height: height) : window.frame.size
            let origin = CGPoint(x: x, y: CGDisplayBounds(CGMainDisplayID()).height - y - size.height)
            window.setFrame(CGRect(origin: origin, size: size), display: true)
        default: break
        }
    }
}
