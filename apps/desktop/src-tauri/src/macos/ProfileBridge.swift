import Foundation

@_cdecl("whispera_profile_directory")
func whisperaProfileDirectory(_ buffer: UnsafeMutablePointer<UInt8>?, _ capacity: Int) -> Int {
    guard let buffer, capacity > 0,
          let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first else {
        return -1
    }
    let path = Array(base.appendingPathComponent("Whispera", isDirectory: true).path.utf8)
    guard !path.isEmpty, path.count < capacity else { return -1 }
    buffer.update(from: path, count: path.count)
    buffer[path.count] = 0
    return path.count
}
