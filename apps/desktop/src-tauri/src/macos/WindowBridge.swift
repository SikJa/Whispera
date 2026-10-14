import AppKit

private final class WhisperaWindowHitRegion {
    weak var window: NSWindow?
    var paths: [NSBezierPath] = []
    var dragging = false

    init(_ window: NSWindow) { self.window = window }

    func containsPointer() -> Bool {
        guard let window, let content = window.contentView else { return false }
        let point = content.convert(window.convertPoint(fromScreen: NSEvent.mouseLocation), from: nil)
        let topLeft = NSPoint(x: point.x - content.bounds.minX,
                              y: content.isFlipped ? point.y - content.bounds.minY : content.bounds.maxY - point.y)
        return paths.contains { $0.contains(topLeft) }
    }

    func update() {
        guard let window else { return }
        if NSEvent.pressedMouseButtons == 0 { dragging = false }
        window.ignoresMouseEvents = !window.isVisible || (!dragging && !containsPointer())
    }
}

// Main-thread-only state. No method swizzling and no replacement of Tao's NSWindow subclass.
private var whisperaWindowRegions: [Int: WhisperaWindowHitRegion] = [:]
private var whisperaWindowTimer: Timer?
private var whisperaWindowLocalMonitor: Any?
private var whisperaWindowGlobalMonitor: Any?
private var whisperaWindowCloseObserver: NSObjectProtocol?
private var whisperaWindowVisibilityObservers: [NSObjectProtocol] = []
private var whisperaWindowPollInterval: TimeInterval = 0

private func whisperaWindowStopTracking() {
    whisperaWindowTimer?.invalidate()
    whisperaWindowTimer = nil
    whisperaWindowPollInterval = 0
    if let monitor = whisperaWindowLocalMonitor { NSEvent.removeMonitor(monitor) }
    if let monitor = whisperaWindowGlobalMonitor { NSEvent.removeMonitor(monitor) }
    whisperaWindowLocalMonitor = nil
    whisperaWindowGlobalMonitor = nil
    for observer in whisperaWindowVisibilityObservers { NotificationCenter.default.removeObserver(observer) }
    whisperaWindowVisibilityObservers.removeAll()
    if let observer = whisperaWindowCloseObserver { NotificationCenter.default.removeObserver(observer) }
    whisperaWindowCloseObserver = nil
}

private func whisperaWindowRefreshTimer() {
    let visible = whisperaWindowRegions.values.contains { $0.window?.isVisible == true }
    let interval: TimeInterval = visible ? 1.0 / 30.0 : 1.0
    guard interval != whisperaWindowPollInterval || whisperaWindowTimer == nil else { return }
    whisperaWindowTimer?.invalidate()
    let timer = Timer(timeInterval: interval, repeats: true) { _ in whisperaWindowUpdateRegions() }
    timer.tolerance = visible ? 0.005 : 0.25
    RunLoop.main.add(timer, forMode: .common)
    whisperaWindowPollInterval = interval
    whisperaWindowTimer = timer
}

private func whisperaWindowUpdateRegions() {
    for (number, state) in Array(whisperaWindowRegions) {
        if state.window == nil { whisperaWindowRegions.removeValue(forKey: number) }
        else { state.update() }
    }
    if whisperaWindowRegions.isEmpty { whisperaWindowStopTracking() }
    else { whisperaWindowRefreshTimer() }
}

private func whisperaWindowInstallTracking() {
    guard whisperaWindowCloseObserver == nil else { return }
    let mask: NSEvent.EventTypeMask = [.mouseMoved, .leftMouseDragged, .rightMouseDragged,
                                       .otherMouseDragged, .leftMouseDown, .leftMouseUp,
                                       .rightMouseDown, .rightMouseUp, .otherMouseDown, .otherMouseUp]
    whisperaWindowGlobalMonitor = NSEvent.addGlobalMonitorForEvents(matching: mask) { _ in
        whisperaWindowUpdateRegions()
    }
    whisperaWindowLocalMonitor = NSEvent.addLocalMonitorForEvents(matching: mask) { event in
        if let state = whisperaWindowRegions[event.windowNumber],
           [.leftMouseDown, .rightMouseDown, .otherMouseDown].contains(event.type), state.containsPointer() {
            state.dragging = true
        }
        whisperaWindowUpdateRegions()
        return event
    }
    // Events handle input immediately. The visibility-aware fallback also covers
    // Spaces changes and stationary pointers without keeping the hidden tray busy.
    for name in [NSWindow.didExposeNotification, NSWindow.didBecomeKeyNotification,
                 NSWindow.didMoveNotification, NSWindow.didResizeNotification,
                 NSWindow.didMiniaturizeNotification, NSWindow.didDeminiaturizeNotification,
                 NSApplication.didHideNotification, NSApplication.didUnhideNotification] {
        whisperaWindowVisibilityObservers.append(NotificationCenter.default.addObserver(forName: name,
                                                                                         object: nil, queue: .main) { _ in
            whisperaWindowUpdateRegions()
        })
    }
    whisperaWindowCloseObserver = NotificationCenter.default.addObserver(forName: NSWindow.willCloseNotification,
                                                                         object: nil, queue: .main) { note in
        if let window = note.object as? NSWindow { whisperaWindowRegions.removeValue(forKey: window.windowNumber) }
        guard whisperaWindowRegions.isEmpty else { return }
        whisperaWindowStopTracking()
    }
}

@_cdecl("whispera_window_apply")
func whisperaWindowApply(_ raw: UnsafeMutableRawPointer?) -> Int32 {
    guard Thread.isMainThread, let raw else { return -1 }
    let window = Unmanaged<NSWindow>.fromOpaque(raw).takeUnretainedValue()
    window.styleMask.remove([.titled, .closable, .miniaturizable])
    window.titleVisibility = .hidden
    window.titlebarAppearsTransparent = true
    window.backgroundColor = .clear
    window.isOpaque = false
    window.hasShadow = false
    window.level = .floating
    window.hidesOnDeactivate = false
    window.isExcludedFromWindowsMenu = true
    window.collectionBehavior.remove([.managed, .moveToActiveSpace, .fullScreenPrimary])
    window.collectionBehavior.formUnion([.canJoinAllSpaces, .fullScreenAuxiliary, .ignoresCycle])
    if whisperaWindowRegions[window.windowNumber] == nil {
        whisperaWindowRegions[window.windowNumber] = WhisperaWindowHitRegion(window)
        // Until the frontend sends its first real region, the transparent surface
        // must not intercept the desktop beneath it.
        window.ignoresMouseEvents = true
    }
    whisperaWindowInstallTracking()
    whisperaWindowUpdateRegions()
    return 0
}

@_cdecl("whispera_window_regions")
func whisperaWindowSetRegions(_ raw: UnsafeMutableRawPointer?, _ rectangles: UnsafePointer<Double>?, _ count: Int) -> Int32 {
    guard Thread.isMainThread, let raw, let rectangles, count > 0, count <= 32 else { return -1 }
    let window = Unmanaged<NSWindow>.fromOpaque(raw).takeUnretainedValue()
    let state = whisperaWindowRegions[window.windowNumber] ?? WhisperaWindowHitRegion(window)
    var paths: [NSBezierPath] = []
    for index in 0..<count {
        let offset = index * 4
        let values = (0..<4).map { rectangles[offset + $0] }
        guard values.allSatisfy({ $0.isFinite && abs($0) <= 16384 }), values[2] > 0, values[3] > 0 else { return -1 }
        let rect = NSRect(x: values[0] - 5, y: values[1] - 5, width: values[2] + 10, height: values[3] + 10)
        paths.append(NSBezierPath(roundedRect: rect, xRadius: 6, yRadius: 6))
    }
    state.paths = paths
    whisperaWindowRegions[window.windowNumber] = state
    whisperaWindowInstallTracking()
    whisperaWindowUpdateRegions()
    return 0
}

@_cdecl("whispera_window_menu_bar_mode")
func whisperaWindowMenuBarMode(_ enabled: Int32) -> Int32 {
    guard Thread.isMainThread else { return -1 }
    return NSApplication.shared.setActivationPolicy(enabled != 0 ? .accessory : .regular) ? 0 : -1
}
