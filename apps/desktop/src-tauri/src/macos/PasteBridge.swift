import AppKit
import ApplicationServices
import CoreGraphics

private struct WhisperaPasteDestination {
    let application: NSRunningApplication
    let applicationElement: AXUIElement
    let window: AXUIElement
    let field: AXUIElement
    let editable: Bool
    let capturedAt: TimeInterval
}

private let whisperaPasteRegistryLock = NSLock()
private let whisperaPasteOperationLock = NSLock()
private var whisperaPasteRecent: WhisperaPasteDestination?
private var whisperaPasteTargets: [UInt64: WhisperaPasteDestination] = [:]
private var whisperaPasteTokens: [UInt64] = []
private var whisperaPasteNextToken: UInt64 = 0
private var whisperaPasteTimer: DispatchSourceTimer?
private let whisperaPasteTrackerQueue = DispatchQueue(label: "whispera.paste.tracker", qos: .utility)

private func whisperaPasteAttribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value
}

private func whisperaPasteElement(_ element: AXUIElement, _ name: String) -> AXUIElement? {
    guard let value = whisperaPasteAttribute(element, name), CFGetTypeID(value) == AXUIElementGetTypeID() else { return nil }
    let result = value as! AXUIElement
    AXUIElementSetMessagingTimeout(result, 0.08)
    return result
}

private func whisperaPasteSettable(_ element: AXUIElement, _ name: String) -> Bool {
    var settable = DarwinBoolean(false)
    return AXUIElementIsAttributeSettable(element, name as CFString, &settable) == .success && settable.boolValue
}

private func whisperaPasteIsEditable(_ field: AXUIElement, _ role: String) -> Bool {
    if [kAXTextFieldRole, kAXTextAreaRole, kAXComboBoxRole].contains(role) { return true }
    // Some contenteditable web fields expose a generic role but implement text selection.
    return whisperaPasteSettable(field, kAXSelectedTextAttribute)
}

private func whisperaPasteProtected(_ field: AXUIElement) -> Bool {
    let subrole = whisperaPasteAttribute(field, kAXSubroleAttribute) as? String
    return subrole == kAXSecureTextFieldSubrole
}

private func whisperaPasteSnapshot(_ application: NSRunningApplication) -> (WhisperaPasteDestination?, Int32) {
    guard AXIsProcessTrusted() else { return (nil, 1) }
    guard application.processIdentifier != ProcessInfo.processInfo.processIdentifier, !application.isTerminated else { return (nil, 2) }
    let appElement = AXUIElementCreateApplication(application.processIdentifier)
    AXUIElementSetMessagingTimeout(appElement, 0.08)
    guard let window = whisperaPasteElement(appElement, kAXFocusedWindowAttribute),
          let field = whisperaPasteElement(appElement, kAXFocusedUIElementAttribute),
          let fieldWindow = whisperaPasteElement(field, kAXWindowAttribute), CFEqual(window, fieldWindow),
          let role = whisperaPasteAttribute(field, kAXRoleAttribute) as? String else { return (nil, 5) }
    guard !whisperaPasteProtected(field),
          (whisperaPasteAttribute(field, kAXEnabledAttribute) as? Bool) != false else { return (nil, 8) }
    guard NSWorkspace.shared.frontmostApplication?.processIdentifier == application.processIdentifier else { return (nil, 5) }
    return (WhisperaPasteDestination(application: application, applicationElement: appElement,
                                     window: window, field: field, editable: whisperaPasteIsEditable(field, role),
                                     capturedAt: ProcessInfo.processInfo.systemUptime), 0)
}

private func whisperaPasteRemember() {
    autoreleasepool {
        guard let application = NSWorkspace.shared.frontmostApplication,
              application.processIdentifier != ProcessInfo.processInfo.processIdentifier else { return }
        let (destination, _) = whisperaPasteSnapshot(application)
        whisperaPasteRegistryLock.lock()
        // Forget an old field if the user moved to an inaccessible/protected destination.
        whisperaPasteRecent = destination
        whisperaPasteRegistryLock.unlock()
    }
}

@_cdecl("whispera_paste_tracker_start")
func whisperaPasteTrackerStart() {
    whisperaPasteRegistryLock.lock()
    defer { whisperaPasteRegistryLock.unlock() }
    guard whisperaPasteTimer == nil else { return }
    let timer = DispatchSource.makeTimerSource(queue: whisperaPasteTrackerQueue)
    timer.schedule(deadline: .now(), repeating: .milliseconds(250), leeway: .milliseconds(40))
    timer.setEventHandler { whisperaPasteRemember() }
    whisperaPasteTimer = timer
    timer.resume()
}

private func whisperaPasteCapture(_ requiredPID: UInt32?, _ token: UnsafeMutablePointer<UInt64>?) -> Int32 {
    guard let token else { return 10 }
    token.pointee = 0
    guard AXIsProcessTrusted() else { return 1 }
    guard let application = NSWorkspace.shared.frontmostApplication else { return 2 }
    var destination: WhisperaPasteDestination?
    if application.processIdentifier == ProcessInfo.processInfo.processIdentifier {
        whisperaPasteRegistryLock.lock()
        destination = whisperaPasteRecent
        whisperaPasteRegistryLock.unlock()
        // A recorder click may focus Whispera before its command reaches Rust.
        guard let recent = destination, ProcessInfo.processInfo.systemUptime - recent.capturedAt <= 30 else { return 2 }
    } else {
        let (snapshot, status) = whisperaPasteSnapshot(application)
        guard status == 0 else { return status }
        destination = snapshot
    }
    guard let destination, !destination.application.isTerminated else { return 3 }
    if let requiredPID, requiredPID != UInt32(destination.application.processIdentifier) { return 2 }
    whisperaPasteRegistryLock.lock()
    defer { whisperaPasteRegistryLock.unlock() }
    whisperaPasteNextToken &+= 1
    if whisperaPasteNextToken == 0 { whisperaPasteNextToken = 1 }
    let next = whisperaPasteNextToken
    whisperaPasteTargets[next] = destination
    whisperaPasteTokens.append(next)
    while whisperaPasteTokens.count > 16 { whisperaPasteTargets.removeValue(forKey: whisperaPasteTokens.removeFirst()) }
    token.pointee = next
    return 0
}

@_cdecl("whispera_paste_capture")
func whisperaPasteCaptureCurrent(_ token: UnsafeMutablePointer<UInt64>?) -> Int32 {
    autoreleasepool { whisperaPasteCapture(nil, token) }
}

@_cdecl("whispera_paste_capture_for_pid")
func whisperaPasteCaptureForPID(_ pid: UInt32, _ token: UnsafeMutablePointer<UInt64>?) -> Int32 {
    autoreleasepool { whisperaPasteCapture(pid, token) }
}

private func whisperaPasteLookup(_ token: UInt64) -> WhisperaPasteDestination? {
    whisperaPasteRegistryLock.lock()
    defer { whisperaPasteRegistryLock.unlock() }
    return whisperaPasteTargets[token]
}

private func whisperaPasteStillValid(_ destination: WhisperaPasteDestination) -> Bool {
    guard !destination.application.isTerminated,
          let running = NSRunningApplication(processIdentifier: destination.application.processIdentifier),
          running.launchDate == destination.application.launchDate,
          whisperaPasteAttribute(destination.window, kAXRoleAttribute) as? String == kAXWindowRole,
          let window = whisperaPasteElement(destination.field, kAXWindowAttribute), CFEqual(window, destination.window),
          whisperaPasteAttribute(destination.field, kAXRoleAttribute) != nil,
          (whisperaPasteAttribute(destination.field, kAXEnabledAttribute) as? Bool) != false,
          !whisperaPasteProtected(destination.field) else { return false }
    return true
}

private func whisperaPasteFocused(_ destination: WhisperaPasteDestination) -> Bool {
    guard NSWorkspace.shared.frontmostApplication?.processIdentifier == destination.application.processIdentifier,
          let window = whisperaPasteElement(destination.applicationElement, kAXFocusedWindowAttribute), CFEqual(window, destination.window),
          let field = whisperaPasteElement(destination.applicationElement, kAXFocusedUIElementAttribute), CFEqual(field, destination.field)
    else { return false }
    return NSWorkspace.shared.frontmostApplication?.processIdentifier == destination.application.processIdentifier
}

private func whisperaPasteActivate(_ destination: WhisperaPasteDestination) {
    if (whisperaPasteAttribute(destination.window, kAXMinimizedAttribute) as? Bool) == true {
        AXUIElementSetAttributeValue(destination.window, kAXMinimizedAttribute as CFString, kCFBooleanFalse)
    }
    destination.application.activate(options: [.activateIgnoringOtherApps])
    AXUIElementPerformAction(destination.window, kAXRaiseAction as CFString)
    AXUIElementSetAttributeValue(destination.applicationElement, kAXFocusedWindowAttribute as CFString, destination.window)
    AXUIElementSetAttributeValue(destination.window, kAXMainAttribute as CFString, kCFBooleanTrue)
    AXUIElementSetAttributeValue(destination.field, kAXFocusedAttribute as CFString, kCFBooleanTrue)
}

@_cdecl("whispera_paste_focus")
func whisperaPasteFocus(_ token: UInt64, _ restore: Int32) -> Int32 {
    autoreleasepool {
        guard AXIsProcessTrusted() else { return 1 }
        guard let destination = whisperaPasteLookup(token), whisperaPasteStillValid(destination) else { return 3 }
        if restore != 0 { whisperaPasteActivate(destination) }
        return whisperaPasteFocused(destination) ? 0 : 5
    }
}

private func whisperaPasteModifiersDown() -> Bool {
    let flags = CGEventSource.flagsState(.combinedSessionState)
    return !flags.intersection([.maskCommand, .maskControl, .maskAlternate, .maskShift, .maskSecondaryFn]).isEmpty
}

@_cdecl("whispera_paste_restore")
func whisperaPasteRestore(_ token: UInt64, _ expected: UnsafePointer<UInt8>?, _ length: Int, _ checkText: Int32) -> Int32 {
    guard whisperaPasteOperationLock.try() else { return 9 }
    defer { whisperaPasteOperationLock.unlock() }
    return autoreleasepool {
        guard AXIsProcessTrusted() else { return 1 }
        guard CGPreflightPostEventAccess() else { return 7 }
        guard length >= 0, length <= 16 * 1024 * 1024, checkText == 0 || expected != nil else { return 10 }
        let text: String? = checkText != 0 ? String(bytes: UnsafeBufferPointer(start: expected, count: length), encoding: .utf8) : nil
        if checkText != 0 && text == nil { return 10 }
        guard let destination = whisperaPasteLookup(token), whisperaPasteStillValid(destination) else { return 3 }
        if checkText != 0 && !destination.editable { return 8 }
        let releasedBy = ProcessInfo.processInfo.systemUptime + 2
        while whisperaPasteModifiersDown() {
            if ProcessInfo.processInfo.systemUptime >= releasedBy { return 4 }
            Thread.sleep(forTimeInterval: 0.02)
        }
        whisperaPasteActivate(destination)
        let readyBy = ProcessInfo.processInfo.systemUptime + 1.5
        for attempt in 0..<8 {
            if ProcessInfo.processInfo.systemUptime >= readyBy { break }
            Thread.sleep(forTimeInterval: 0.06)
            if !whisperaPasteStillValid(destination) { return 3 }
            if !whisperaPasteModifiersDown() && whisperaPasteFocused(destination) {
                let board = NSPasteboard.general
                let revision = board.changeCount
                if let text, board.string(forType: .string) != text { return 6 }
                guard let source = CGEventSource(stateID: .privateState),
                      let down = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: true),
                      let up = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: false) else { return 7 }
                // Explicit event flags avoid synthesizing releases of physical modifiers.
                down.flags = .maskCommand
                up.flags = .maskCommand
                guard AXIsProcessTrusted(), CGPreflightPostEventAccess() else { return 7 }
                guard whisperaPasteFocused(destination), !whisperaPasteModifiersDown() else { continue }
                guard board.changeCount == revision else { return 6 }
                // Target the captured PID, not the global event stream. Submit exactly once;
                // CGEvent has no acknowledgement of insertion, so never retry the paste.
                down.postToPid(destination.application.processIdentifier)
                up.postToPid(destination.application.processIdentifier)
                return 0
            }
            if attempt == 3 { whisperaPasteActivate(destination) }
        }
        return 5
    }
}
