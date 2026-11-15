import AppKit
import ApplicationServices
import Foundation

struct Failure: Error, CustomStringConvertible {
    let description: String
    init(_ message: String) { description = message }
}

func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    return AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success ? value : nil
}

func label(_ element: AXUIElement) -> String {
    for name in [kAXTitleAttribute, kAXDescriptionAttribute] {
        if let value = attribute(element, name) as? String, !value.isEmpty { return value }
    }
    return ""
}

func role(_ element: AXUIElement) -> String { attribute(element, kAXRoleAttribute) as? String ?? "" }

func children(_ element: AXUIElement) -> [AXUIElement] {
    attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? []
}

func walk(_ root: AXUIElement, visit: (AXUIElement) -> Void) throws {
    var pending: [(AXUIElement, Int)] = [(root, 0)]
    var count = 0
    let deadline = Date().addingTimeInterval(8)
    while let (element, depth) = pending.popLast() {
        count += 1
        guard Date() < deadline else { throw Failure("Accessibility tree inspection timed out") }
        guard count <= 30_000, depth <= 100 else { throw Failure("Accessibility tree exceeds the inspection limit") }
        visit(element)
        pending.append(contentsOf: children(element).reversed().map { ($0, depth + 1) })
    }
}

func frame(_ element: AXUIElement) throws -> CGRect {
    guard let position = attribute(element, kAXPositionAttribute), let size = attribute(element, kAXSizeAttribute),
          CFGetTypeID(position) == AXValueGetTypeID(), CFGetTypeID(size) == AXValueGetTypeID() else {
        throw Failure("Control has no accessible screen bounds: \(label(element))")
    }
    var point = CGPoint.zero
    var extent = CGSize.zero
    guard AXValueGetValue(unsafeBitCast(position, to: AXValue.self), .cgPoint, &point),
          AXValueGetValue(unsafeBitCast(size, to: AXValue.self), .cgSize, &extent), extent.width > 0, extent.height > 0 else {
        throw Failure("Control has invalid screen bounds: \(label(element))")
    }
    return CGRect(origin: point, size: extent)
}

func match(_ element: AXUIElement, _ selector: [String: Any]) -> Bool {
    let roles: [String: Set<String>] = [
        "button": ["AXButton", "AXCheckBox", "AXRadioButton", "AXPopUpButton", "AXMenuItem"],
        "field": ["AXTextField", "AXTextArea", "AXComboBox"],
        "tab": ["AXRadioButton", "AXButton"],
    ]
    if let wanted = selector["role"] as? String {
        guard roles[wanted]?.contains(role(element)) ?? (role(element) == wanted) else { return false }
    }
    let actual = label(element)
    let wanted = selector["name"] as? String ?? ""
    switch selector["match"] as? String ?? "exact" {
    case "prefix": return actual.hasPrefix(wanted)
    case "contains": return actual.contains(wanted)
    default: return actual == wanted
    }
}

func find(_ root: AXUIElement, _ selector: [String: Any]) throws -> AXUIElement {
    var roots = [root]
    if let within = selector["within"] as? String {
        roots = []
        try walk(root) { if label($0) == within { roots.append($0) } }
        guard roots.count == 1 else { throw Failure("Expected one container named '\(within)', found \(roots.count)") }
    }
    var matches: [AXUIElement] = []
    try walk(roots[0]) { if match($0, selector) { matches.append($0) } }
    guard matches.count == 1 else {
        throw Failure("Expected one control '\(selector["name"] ?? "")', found \(matches.count). Use role, within, or a more specific name.")
    }
    if let enabled = attribute(matches[0], kAXEnabledAttribute) as? Bool, !enabled {
        throw Failure("Control is disabled: \(label(matches[0]))")
    }
    return matches[0]
}

func emitMouse(_ type: CGEventType, _ point: CGPoint) {
    CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: point, mouseButton: .left)?.post(tap: .cghidEventTap)
}

func move(to point: CGPoint, seconds: Double = 0.18, dragging: Bool = false) {
    let start = CGEvent(source: nil)?.location ?? point
    let frames = max(1, Int(seconds * 60))
    for step in 1...frames {
        let t = Double(step) / Double(frames)
        let eased = t * t * (3 - 2 * t)
        emitMouse(dragging ? .leftMouseDragged : .mouseMoved,
                  CGPoint(x: start.x + (point.x - start.x) * eased, y: start.y + (point.y - start.y) * eased))
        Thread.sleep(forTimeInterval: seconds / Double(frames))
    }
}

func click(_ point: CGPoint) {
    move(to: point)
    emitMouse(.leftMouseDown, point)
    Thread.sleep(forTimeInterval: 0.06)
    emitMouse(.leftMouseUp, point)
}

func key(_ chord: String) throws {
    let keys: [String: CGKeyCode] = [
        "a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8, "v": 9,
        "b": 11, "q": 12, "w": 13, "e": 14, "r": 15, "y": 16, "t": 17,
        "1": 18, "2": 19, "3": 20, "4": 21, "6": 22, "5": 23, "9": 25, "7": 26, "8": 28, "0": 29,
        "o": 31, "u": 32, "i": 34, "p": 35, "return": 36, "enter": 36, "l": 37, "j": 38, "k": 40,
        "n": 45, "m": 46, "tab": 48, "space": 49, "backspace": 51, "escape": 53, "esc": 53,
        "left": 123, "right": 124, "down": 125, "up": 126,
    ]
    var parts = chord.lowercased().split(separator: "+").map { $0.trimmingCharacters(in: .whitespaces) }
    guard let main = parts.popLast(), let code = keys[main] else { throw Failure("Unsupported key chord: \(chord)") }
    var flags: CGEventFlags = []
    for modifier in parts {
        switch modifier {
        case "cmd", "command", "super": flags.insert(.maskCommand)
        case "ctrl", "control": flags.insert(.maskControl)
        case "alt", "option": flags.insert(.maskAlternate)
        case "shift": flags.insert(.maskShift)
        default: throw Failure("Unsupported modifier: \(modifier)")
        }
    }
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down)
        event?.flags = flags
        event?.post(tap: .cghidEventTap)
        Thread.sleep(forTimeInterval: 0.04)
    }
}

func type(_ text: String) {
    let units = Array(text.utf16)
    // One event retains complete surrogate pairs and avoids changing the clipboard.
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: down)
        units.withUnsafeBufferPointer { event?.keyboardSetUnicodeString(stringLength: units.count, unicodeString: $0.baseAddress!) }
        event?.post(tap: .cghidEventTap)
        Thread.sleep(forTimeInterval: 0.04)
    }
}

@main struct NativeUI {
    static func main() {
        do {
            _ = NSApplication.shared
            NSApp.setActivationPolicy(.accessory)
            let request = try JSONSerialization.jsonObject(with: FileHandle.standardInput.readDataToEndOfFile()) as? [String: Any] ?? [:]
            let result = try run(request)
            let bytes = try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys])
            print(String(data: bytes, encoding: .utf8)!)
        } catch { fputs("\(error)\n", stderr); exit(1) }
    }

    static func run(_ request: [String: Any]) throws -> [String: Any] {
        let operation = request["operation"] as? String ?? "check"
        if operation == "check" { return ["trusted": AXIsProcessTrusted()] }
        guard AXIsProcessTrusted() else { throw Failure("Accessibility access is required. Enable the launching terminal/app in System Settings > Privacy & Security > Accessibility, then rerun.") }
        let specification = request["app"] as? [String: Any] ?? [:]
        let bundle = specification["bundleId"] as? String ?? ""
        let path = specification["path"] as? String
        let pid = specification["pid"] as? Int32
        let candidates = NSWorkspace.shared.runningApplications.filter {
            $0.bundleIdentifier == bundle && (path == nil || $0.bundleURL?.standardizedFileURL.path == URL(fileURLWithPath: path!).standardizedFileURL.path)
                && (pid == nil || $0.processIdentifier == pid)
        }
        guard candidates.count == 1 else { throw Failure("Expected one running app '\(bundle)', found \(candidates.count). Open the intended app or specify its path.") }
        let app = candidates[0]
        let appAX = AXUIElementCreateApplication(app.processIdentifier)
        AXUIElementSetMessagingTimeout(appAX, 1)
        let windows = attribute(appAX, kAXWindowsAttribute) as? [AXUIElement] ?? []
        let wantedWindow = specification["window"] as? String
        let selected = windows.filter { wantedWindow == nil || label($0) == wantedWindow }
        guard selected.count == 1 else { throw Failure("Expected one window in '\(bundle)', found \(selected.count). Set apps.<alias>.window to its exact title.") }
        let window = selected[0]
        if operation == "inspect" {
            var controls: [[String: Any]] = []
            try walk(window) { element in
                if !label(element).isEmpty {
                    controls.append(["role": role(element), "name": label(element), "enabled": attribute(element, kAXEnabledAttribute) as? Bool ?? true])
                }
            }
            return ["pid": app.processIdentifier, "window": label(window), "controls": controls]
        }
        if operation == "waitFor" {
            let found = try find(window, request["selector"] as? [String: Any] ?? [:])
            return ["name": label(found), "role": role(found)]
        }
        app.activate(options: [.activateAllWindows])
        AXUIElementPerformAction(window, kAXRaiseAction as CFString)
        Thread.sleep(forTimeInterval: 0.12)
        guard NSWorkspace.shared.frontmostApplication?.processIdentifier == app.processIdentifier else { throw Failure("Could not focus the intended app; no input sent") }
        if operation == "focus" { return ["pid": app.processIdentifier, "window": label(window)] }
        let bounds = try frame(window)
        func point(_ value: [String: Any]) throws -> CGPoint {
            guard let x = value["x"] as? Double, let y = value["y"] as? Double,
                  x.isFinite, y.isFinite, x >= 0, y >= 0, x < bounds.width, y < bounds.height else { throw Failure("Canvas point is outside the target window") }
            return CGPoint(x: bounds.minX + x, y: bounds.minY + y)
        }
        switch operation {
        case "click", "fill":
            var selector = request["selector"] as? [String: Any] ?? [:]
            if operation == "fill" { selector["role"] = "field" }
            let found = try find(window, selector)
            let control = try frame(found)
            let centre = CGPoint(x: control.midX, y: control.midY)
            guard bounds.contains(centre) else { throw Failure("Control is outside the visible window: \(label(found)). Scroll it into view first.") }
            click(centre)
            if operation == "fill" {
                try key("cmd+a")
                let text = request["text"] as? String ?? ""
                if !text.isEmpty { type(text) } else { try key("backspace") }
            }
            return ["name": label(found), "role": role(found)]
        case "key": try key(request["key"] as? String ?? "")
        case "clickAt": click(try point(request["point"] as? [String: Any] ?? [:]))
        case "drag":
            let from = try point(request["from"] as? [String: Any] ?? [:])
            let to = try point(request["to"] as? [String: Any] ?? [:])
            let seconds = (request["durationMs"] as? Double ?? 1000) / 1000
            guard seconds > 0, seconds <= 10 else { throw Failure("Drag must last at most ten seconds") }
            move(to: from)
            emitMouse(.leftMouseDown, from)
            defer { emitMouse(.leftMouseUp, to) }
            move(to: to, seconds: seconds, dragging: true)
        default: throw Failure("Unsupported operation: \(operation)")
        }
        return ["pid": app.processIdentifier, "operation": operation]
    }
}
