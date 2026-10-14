import AVFoundation
import Foundation

@_cdecl("whispera_sound_play")
func whisperaSoundPlay(_ bytes: UnsafePointer<UInt8>?, _ length: Int) -> Int32 {
    guard let bytes, length > 0, length <= 4 * 1024 * 1024 else { return -1 }
    return autoreleasepool {
        do {
            let player = try AVAudioPlayer(data: Data(bytes: bytes, count: length))
            guard player.duration.isFinite, player.duration > 0, player.duration <= 10,
                  player.prepareToPlay(), player.play() else { return -1 }
            // Rust calls on its sound worker; retain the player until playback ends.
            let deadline = ProcessInfo.processInfo.systemUptime + player.duration + 1
            while player.isPlaying && ProcessInfo.processInfo.systemUptime < deadline {
                Thread.sleep(forTimeInterval: 0.01)
            }
            if player.isPlaying { player.stop(); return -1 }
            return 0
        } catch {
            return -1
        }
    }
}
