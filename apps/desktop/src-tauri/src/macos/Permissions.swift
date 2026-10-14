import AppKit
import AVFoundation
import ApplicationServices
import CoreGraphics

@_cdecl("whispera_permission_status")
public func whisperaPermissionStatus(_ kind: Int32) -> Int32 {
    switch kind {
    case 0: return AXIsProcessTrusted() ? 1 : 0
    case 1: return CGPreflightScreenCaptureAccess() ? 1 : 0
    case 2:
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized: return 1
        case .notDetermined: return 0
        default: return 2
        }
    default: return -1
    }
}

@_cdecl("whispera_permission_request")
public func whisperaPermissionRequest(_ kind: Int32) -> Int32 {
    switch kind {
    case 0:
        let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
        return AXIsProcessTrustedWithOptions(options) ? 1 : 0
    case 1:
        return CGRequestScreenCaptureAccess() ? 1 : 0
    case 2:
        if AVCaptureDevice.authorizationStatus(for: .audio) != .notDetermined {
            return whisperaPermissionStatus(2)
        }
        let signal = DispatchSemaphore(value: 0)
        AVCaptureDevice.requestAccess(for: .audio) { _ in signal.signal() }
        // This C entry is invoked only on a Rust blocking worker, never on AppKit's main thread.
        _ = signal.wait(timeout: .now() + 120)
        return whisperaPermissionStatus(2)
    default: return -1
    }
}
