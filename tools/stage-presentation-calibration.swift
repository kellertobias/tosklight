// Offline schema calibration probe. Compile only; launching is an explicit operator action.
// SDK presentedTime is actual on-screen host time, unlike requested time or callback arrival.
// Usage: probe --output ABS.jsonl --ready-file ABS.json --go-file ABS.token
// Optional --stop-file ABS.token keeps the completed probe idle for trace export/identity checks;
// atomically create it with the same goToken to release. Hard lifetime is 50s, including waits.
// All paths must be new, canonical, caller-owned paths with existing parents under .artifacts.
// Read the ready JSON, attach the owned trace to its PID, then atomically create the go file
// containing exactly ready.goToken. No nextDrawable/present is requested before this handshake.
// Export ca-client-present-request, ca-client-presented-handler and time-info separately;
// do not assume their surface/drawable ids or Requested Time fields match this SDK observation.
import AppKit
import CoreGraphics
import Darwin
import Foundation
import IOSurface
import Metal
import QuartzCore

struct Options {
    let output: String
    let ready: String
    let go: String
    let stop: String?
    static func read() throws -> Options {
        let args = Array(CommandLine.arguments.dropFirst())
        guard args.count == 6 || args.count == 8 else { throw ProbeError.message("Usage: probe --output ABS.jsonl --ready-file ABS.json --go-file ABS.token [--stop-file ABS.token]") }
        var values: [String: String] = [:]
        for index in stride(from: 0, to: args.count, by: 2) {
            let key = args[index]
            guard ["--output", "--ready-file", "--go-file", "--stop-file"].contains(key), values[key] == nil else {
                throw ProbeError.message("Unknown or duplicate option \(key)")
            }
            let path = args[index + 1]
            let url = URL(fileURLWithPath: path)
            guard path.hasPrefix("/"), url.standardizedFileURL.path == path,
                  url.deletingLastPathComponent().resolvingSymlinksInPath().path == url.deletingLastPathComponent().path,
                  FileManager.default.fileExists(atPath: url.deletingLastPathComponent().path),
                  !FileManager.default.fileExists(atPath: path) else {
                throw ProbeError.message("Paths must be canonical, absolute, distinct and not yet exist; parents must exist: \(path)")
            }
            values[key] = path
        }
        guard Set(values.values).count == values.count, let output = values["--output"],
              let ready = values["--ready-file"], let go = values["--go-file"] else {
            throw ProbeError.message("All three distinct paths are required")
        }
        return Options(output: output, ready: ready, go: go, stop: values["--stop-file"])
    }
}
enum ProbeError: Error { case message(String) }

func anchor() -> [String: Any] {
    let before = mach_absolute_time()
    let host = CACurrentMediaTime()
    let after = mach_absolute_time()
    return ["machBefore": String(before), "caHostSeconds": host, "machAfter": String(after)]
}
func exclusiveFile(_ path: String) throws -> FileHandle {
    let descriptor = open(path, O_CREAT | O_EXCL | O_WRONLY | O_NOFOLLOW, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw ProbeError.message("Cannot exclusively create \(path): \(String(cString: strerror(errno)))") }
    return FileHandle(fileDescriptor: descriptor, closeOnDealloc: true)
}

@MainActor
final class Probe: NSObject, NSApplicationDelegate, NSWindowDelegate {
    let options: Options
    let log: FileHandle
    let runToken = UUID().uuidString
    let layerToken = UUID().uuidString
    let launched = ISO8601DateFormatter().string(from: Date())
    var window: NSWindow!
    var layer: CAMetalLayer!
    var queue: MTLCommandQueue!
    var display: CGDirectDisplayID = 0
    var timer: Timer?
    var sequence = 0
    var callbacks = 0
    var completions = 0
    var positivePresentations = 0
    var bytes = 0
    var rows = 0
    var finished = false
    var finishing = false
    var loggingFailed = false
    var started = false
    var waitingStop = false
    let lifetimeDeadline = CACurrentMediaTime() + 50
    var outstanding = false
    // Apple recommends prompt drawable release after commit; this corrects lifetime only,
    // without claiming why prior probes lacked callbacks. See https://developer.apple.com/documentation/quartzcore/cametallayer
    weak var pending: CAMetalDrawable?
    var exitCode: Int32 = 0
    var waitingDeadline = CACurrentMediaTime() + 20
    var renderingDeadline = Double.infinity

    init(options: Options) throws {
        self.options = options
        self.log = try exclusiveFile(options.output)
        super.init()
    }
    func record(_ event: String, _ fields: [String: Any] = [:]) {
        guard !finished else { return }
        var row = fields
        row.merge(["event": event, "pid": ProcessInfo.processInfo.processIdentifier,
                   "processStartedUTC": launched, "runToken": runToken, "layerToken": layerToken,
                   "anchor": anchor()]) { _, new in new }
        do {
            var data = try JSONSerialization.data(withJSONObject: row, options: [.sortedKeys])
            data.append(10)
            guard data.count <= 8192, rows < 512, bytes + data.count <= 512 * 1024 else {
                throw ProbeError.message("Bounded event log capacity exceeded")
            }
            try log.write(contentsOf: data)
            rows += 1
            bytes += data.count
        } catch {
            fputs("Calibration logging failure: \(error)\n", stderr)
            loggingFailed = true
            finish("logging-failed", code: 1)
        }
    }
    func applicationDidFinishLaunching(_ notification: Notification) {
        do {
            try prepareWindow()
            var info = mach_timebase_info_data_t()
            guard mach_timebase_info(&info) == KERN_SUCCESS else { throw ProbeError.message("Missing Mach timebase") }
            record("ready", ["displayId": display, "machNumer": info.numer, "machDenom": info.denom,
                             "goToken": runToken, "maximumFrames": 60, "waitLimitSeconds": 20,
                             "renderLimitSeconds": 15, "drainLimitSeconds": 2, "hardLifetimeSeconds": 50,
                             "stopFile": options.stop.map { $0 as Any } ?? NSNull(),
                             "displaySyncEnabled": layer.displaySyncEnabled, "windowVisibility": visibility()])
            guard !finished else { return }
            let ready = try exclusiveFile(options.ready)
            try ready.write(contentsOf: JSONSerialization.data(withJSONObject: [
                "pid": ProcessInfo.processInfo.processIdentifier, "goToken": runToken,
                "displayId": display, "output": options.output, "readyAnchor": anchor(),
                "stopFile": options.stop.map { $0 as Any } ?? NSNull(), "hardLifetimeSeconds": 50
            ], options: [.sortedKeys]))
            try ready.close()
            waitingDeadline = CACurrentMediaTime() + 20
            timer = Timer.scheduledTimer(withTimeInterval: 0.05, repeats: true) { [weak self] _ in
                MainActor.assumeIsolated { self?.tick() }
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + max(0, lifetimeDeadline - CACurrentMediaTime())) { [weak self] in
                self?.finish("hard-lifetime-timeout", code: 1)
            }
        } catch { record("error", ["detail": String(describing: error)]); finish("setup-failed", code: 1) }
    }
    func prepareWindow() throws {
        let candidates = NSScreen.screens.filter { screen in
            guard let number = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber else { return false }
            let id = CGDirectDisplayID(number.uint32Value)
            return CGDisplayIsBuiltin(id) != 0 && CGDisplayIsOnline(id) != 0 && CGDisplayIsAsleep(id) == 0
                && CGDisplayIsInMirrorSet(id) == 0 && CGDisplayMirrorsDisplay(id) == kCGNullDirectDisplay
        }
        guard candidates.count == 1, let screen = candidates.first,
              let number = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber,
              let device = MTLCreateSystemDefaultDevice(), let commands = device.makeCommandQueue() else {
            throw ProbeError.message("Unique online, awake, unmirrored built-in screen and Metal device required")
        }
        display = CGDirectDisplayID(number.uint32Value)
        queue = commands
        let area = screen.visibleFrame
        guard area.width >= 680, area.height >= 440 else { throw ProbeError.message("Built-in work area too small") }
        let frame = NSRect(x: area.midX - 320, y: area.midY - 180, width: 640, height: 360)
        window = NSWindow(contentRect: frame, styleMask: [.titled, .closable], backing: .buffered, defer: false, screen: screen)
        window.title = "Owned Stage presentation calibration"
        window.isReleasedWhenClosed = false
        window.delegate = self
        // Only this verified internal probe floats; never activate or request key/main focus.
        window.level = .floating
        guard let content = window.contentView else { throw ProbeError.message("Missing content view") }
        layer = CAMetalLayer()
        layer.device = device
        layer.pixelFormat = .bgra8Unorm
        layer.framebufferOnly = true
        layer.displaySyncEnabled = true
        layer.presentsWithTransaction = false
        layer.maximumDrawableCount = 2
        layer.allowsNextDrawableTimeout = true
        content.wantsLayer = true
        guard let backing = content.layer else { throw ProbeError.message("AppKit backing layer missing") }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        backing.addSublayer(layer) // Match raw-window-metal: preserve AppKit's view-delegated root.
        layer.frame = backing.bounds
        layer.contentsScale = screen.backingScaleFactor
        layer.drawableSize = content.convertToBacking(content.bounds).size
        layer.isHidden = false
        layer.opacity = 1
        CATransaction.commit()
        guard attached() else { throw ProbeError.message("Metal sublayer is not attached to the AppKit root") }
        guard contained() else { throw ProbeError.message("Hidden internal screen containment verification failed") }
        CATransaction.flush()
        window.orderFrontRegardless()
        CATransaction.flush()
        guard contained() else { throw ProbeError.message("Shown window escaped internal screen") }
    }
    func visibility() -> [String: Any] {
        guard let window else { return ["created": false] }
        return ["created": true, "visible": window.isVisible,
                "occlusionVisible": window.occlusionState.contains(.visible),
                "onActiveSpace": window.isOnActiveSpace, "key": window.isKeyWindow,
                "main": window.isMainWindow, "applicationActive": NSApplication.shared.isActive,
                "windowNumber": window.windowNumber, "windowLevel": window.level.rawValue,
                "framePoints": [window.frame.origin.x, window.frame.origin.y, window.frame.width, window.frame.height],
                "layerAttachment": layerDetails()]
    }
    func attached() -> Bool {
        guard let content = window?.contentView, let backing = content.layer, let layer else { return false }
        return content.window === window && backing !== layer && layer.superlayer === backing
            && layer.frame == backing.bounds && layer.contentsScale == window.backingScaleFactor
    }
    func layerDetails() -> [String: Any] {
        guard let layer else { return ["created": false] }
        func details(_ value: CALayer) -> [String: Any] {
            return ["class": String(describing: type(of: value)),
                "delegateClass": value.delegate.map { String(describing: type(of: $0)) } ?? "nil",
                "superlayerClass": value.superlayer.map { String(describing: type(of: $0)) } ?? "nil",
                "frame": [value.frame.origin.x, value.frame.origin.y, value.frame.width, value.frame.height],
                "bounds": [value.bounds.origin.x, value.bounds.origin.y, value.bounds.width, value.bounds.height],
                "hidden": value.isHidden, "opacity": value.opacity, "contentsScale": value.contentsScale]
        }
        var chain: [[String: Any]] = []
        var current: CALayer? = layer
        while let value = current, chain.count < 8 { chain.append(details(value)); current = value.superlayer }
        return ["attachedToAppKitBacking": attached(), "metal": details(layer), "chain": chain,
            "drawableSize": [layer.drawableSize.width, layer.drawableSize.height]]
    }
    func contained() -> Bool {
        guard let screen = window?.screen,
              let number = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber else { return false }
        let id = CGDirectDisplayID(number.uint32Value)
        return id == display && CGDisplayIsBuiltin(id) != 0 && CGDisplayIsOnline(id) != 0
            && CGDisplayIsAsleep(id) == 0 && CGDisplayIsInMirrorSet(id) == 0
            && CGDisplayMirrorsDisplay(id) == kCGNullDirectDisplay && screen.visibleFrame.contains(window.frame)
    }
    func tick() {
        guard !finished else { return }
        guard contained() else { finish("internal-containment-lost", code: 1); return }
        guard attached() else { finish("layer-attachment-lost", code: 1); return }
        if waitingStop {
            guard let stop = options.stop else { finish("missing-stop-configuration", code: 1); return }
            var status = stat()
            guard lstat(stop, &status) == 0 else { return }
            guard status.st_uid == getuid(), status.st_mode & S_IFMT == S_IFREG,
                  status.st_size > 0, status.st_size <= 128,
                  (try? String(contentsOfFile: stop, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)) == runToken else {
                finish("invalid-stop-token-or-owner", code: 1); return
            }
            finish("owned-stop-received", code: 0)
        } else if !started {
            if CACurrentMediaTime() >= waitingDeadline { finish("go-timeout", code: 1); return }
            var status = stat()
            guard lstat(options.go, &status) == 0 else { return }
            guard status.st_uid == getuid(), status.st_mode & S_IFMT == S_IFREG,
                  status.st_size > 0, status.st_size <= 128,
                  (try? String(contentsOfFile: options.go, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)) == runToken else {
                finish("invalid-go-token-or-owner", code: 1); return
            }
            started = true
            renderingDeadline = CACurrentMediaTime() + 15
            record("go", ["windowVisibility": visibility()])
            submit()
        } else if (sequence < 60 || outstanding) && CACurrentMediaTime() >= renderingDeadline {
            finish("presentation-timeout-unknown", code: 1)
        }
    }
    func submit() {
        guard !finished, !outstanding, sequence < 60, contained() else { return }
        guard CACurrentMediaTime() < renderingDeadline else { finish("presentation-timeout-unknown", code: 1); return }
        guard attached() else { finish("layer-attachment-lost-before-acquire", code: 1); return }
        CATransaction.flush()
        autoreleasepool {
            let acquire = anchor()
            guard let drawable = layer.nextDrawable(), let command = queue.makeCommandBuffer() else {
                finish("drawable-or-command-unavailable", code: 1); return
            }
            guard contained() else { finish("internal-containment-lost", code: 1); return }
            guard attached() else { finish("layer-attachment-lost-before-submit", code: 1); return }
            outstanding = true
            pending = drawable
            let current = sequence
            sequence += 1
            let label = "stage-presentation-calibration-\(runToken)-\(current)"
            command.label = label
            let render = MTLRenderPassDescriptor()
            render.colorAttachments[0].texture = drawable.texture
            render.colorAttachments[0].loadAction = .clear
            render.colorAttachments[0].storeAction = .store
            render.colorAttachments[0].clearColor = current % 2 == 0 ? MTLClearColorMake(0.08, 0.16, 0.24, 1) : MTLClearColorMake(0.24, 0.12, 0.08, 1)
            guard let encoder = command.makeRenderCommandEncoder(descriptor: render) else { finish("encoder-unavailable", code: 1); return }
            encoder.endEncoding()
            let delays = [0.0371, 0.0537, 0.0713]
            let requested: Double? = current % 2 == 0 ? nil : CACurrentMediaTime() + delays[(current / 2) % delays.count]
            let identity: [String: Any] = ["sequence": current, "drawableId": drawable.drawableID,
                "commandLabel": label, "requestedHostSeconds": requested.map { $0 as Any } ?? NSNull(),
                "surfaceId": drawable.texture.iosurface.map { IOSurfaceGetID($0) as Any } ?? NSNull()]
            drawable.addPresentedHandler { [weak self] shown in
                let time = shown.presentedTime
                let callback = anchor()
                DispatchQueue.main.async { self?.presented(identity, time: time, callback: callback) }
            }
            command.addCompletedHandler { [weak self] completed in
                let fields: [String: Any] = ["identity": identity, "completionAnchor": anchor(),
                    "commandStatus": completed.status.rawValue, "commandSucceeded": completed.status == .completed && completed.error == nil,
                    "gpuStartHostSeconds": completed.gpuStartTime,
                    "gpuEndHostSeconds": completed.gpuEndTime, "error": completed.error.map(String.init(describing:)) ?? ""]
                DispatchQueue.main.async { self?.completed(fields) }
            }
            record("submit", ["identity": identity, "acquireAnchor": acquire, "commitAnchor": anchor(),
                              "windowVisibility": visibility()])
            guard !finished else { return }
            if let requested { command.present(drawable, atTime: requested) } else { command.present(drawable) }
            command.commit()
        }
    }
    func presented(_ identity: [String: Any], time: Double, callback: [String: Any]) {
        guard !finished else { return }
        callbacks += 1
        if time > 0 { positivePresentations += 1 }
        record("presented-handler", ["identity": identity, "presentedHostSeconds": time,
            "callbackAnchor": callback, "classification": time > 0 ? "reported-on-screen" : "not-presented-or-skipped-unknown"])
        outstanding = false
        pending = nil
        if sequence < 60 { DispatchQueue.main.async { [weak self] in self?.submit() } }
        else {
            DispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self] in
                guard let self else { return }
                self.finish("bounded-probe-complete", code: self.callbacks == 60 && self.completions == 60 && self.positivePresentations == 60 ? 0 : 1)
            }
        }
    }
    func completed(_ fields: [String: Any]) {
        guard !finished else { return }
        completions += 1
        record("command-completed", fields)
        if fields["commandSucceeded"] as? Bool != true { finish("command-failed", code: 1) }
    }
    func finish(_ reason: String, code: Int32) {
        guard !finished, !finishing else { return }
        if code == 0, reason == "bounded-probe-complete", options.stop != nil {
            waitingStop = true
            record("probe-complete-awaiting-stop", ["submitted": sequence, "callbacks": callbacks,
                "positivePresentedTimes": positivePresentations, "completions": completions,
                "remainingHardLifetimeSeconds": max(0, lifetimeDeadline - CACurrentMediaTime()),
                "boundary": "No more drawables, commands or presents while awaiting owned stop token"])
            return
        }
        finishing = true
        record("summary", ["reason": reason, "submitted": sequence, "callbacks": callbacks,
            "positivePresentedTimes": positivePresentations, "completions": completions,
            "pendingUnknown": outstanding, "exitCode": code, "windowVisibility": visibility(),
            "pendingPresentedHostSecondsAtShutdown": pending.map { $0.presentedTime as Any } ?? NSNull(),
            "boundary": "Calibration only; no inferred trace/display equivalence or Stage FPS acceptance"])
        finished = true
        exitCode = loggingFailed ? 1 : code
        timer?.invalidate()
        pending = nil
        window?.orderOut(nil)
        window?.close()
        try? log.synchronize()
        try? log.close()
        // NSApplication.stop can leave this command-line AppKit run loop waiting for an event.
        // This owned CLI probe has already flushed/closed its only log and hidden its window.
        Darwin.exit(exitCode)
    }
    func windowDidChangeOcclusionState(_ notification: Notification) {
        record("occlusion-state-changed", ["windowVisibility": visibility()])
    }
    func windowDidMove(_ notification: Notification) { if !contained() { finish("window-moved-outside-internal", code: 1) } }
    func windowDidChangeScreen(_ notification: Notification) { if !contained() { finish("screen-changed", code: 1) } }
    func windowWillClose(_ notification: Notification) { finish("window-closed", code: 1) }
}

MainActor.assumeIsolated {
if #available(macOS 10.15.4, *) {
    do {
        let options = try Options.read()
        let app = NSApplication.shared
        app.setActivationPolicy(.accessory)
        let probe = try Probe(options: options)
        app.delegate = probe
        app.run()
        exit(probe.exitCode)
    } catch {
        fputs("Calibration failed before launch: \(error)\n", stderr)
        exit(1)
    }
} else {
    fputs("Calibration requires macOS 10.15.4 or later\n", stderr)
    exit(1)
}
}
