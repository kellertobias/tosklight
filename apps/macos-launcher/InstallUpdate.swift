import AppKit
import Darwin

private let knownIdentifiers: Set<String> = [
    "de.tokenet.light", "de.tokenet.tosklight.visualizer",
    "de.tokenet.tosklight.media", "de.tokenet.tosklight.launcher"
]
private let launcherIdentifier = "de.tokenet.tosklight.launcher"
private let files = FileManager.default
private let applicationNames = [
    "de.tokenet.light": "ToskLight Control",
    "de.tokenet.tosklight.visualizer": "ToskLight Architect",
    "de.tokenet.tosklight.media": "ToskLight Pixel",
    "de.tokenet.tosklight.launcher": "ToskLight Launcher"
]

private struct Manifest: Decodable {
    struct Item: Decodable {
        let source: String
        let destination: String
        let bundleIdentifier: String
    }
    let launcherPID: Int
    let launcherURL: String
    let resultURL: String
    let items: [Item]
    let reopen: Bool?
}

private struct InstallResult: Encodable {
    let success: Bool
    let message: String
    let backups: [String]
}

private struct InstallError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

private func fail(_ message: String) -> InstallError { InstallError(message: message) }

private func failureMessage(_ error: Error) -> String {
    let detail = error as NSError
    let permissionFailure = (detail.domain == NSPOSIXErrorDomain && [Int(EACCES), Int(EPERM)].contains(detail.code)) ||
        (detail.domain == NSCocoaErrorDomain && [NSFileWriteNoPermissionError, NSFileReadNoPermissionError].contains(detail.code))
    if permissionFailure {
        return "\(error.localizedDescription) Move the complete suite to a writable Applications folder, or update it with Homebrew. No administrator access is requested."
    }
    if let underlying = detail.userInfo[NSUnderlyingErrorKey] as? Error,
       failureMessage(underlying).contains("No administrator access is requested.") {
        return "\(error.localizedDescription) Move the complete suite to a writable Applications folder, or update it with Homebrew. No administrator access is requested."
    }
    return error.localizedDescription
}

private func fileStatus(_ url: URL) throws -> stat? {
    var status = stat()
    let result = url.path.withCString { Darwin.lstat($0, &status) }
    if result == 0 { return status }
    if errno == ENOENT { return nil }
    throw fail("Cannot inspect \(url.path): \(String(cString: strerror(errno)))")
}

private func location(_ value: String) throws -> URL {
    guard !value.contains("\0") else { throw fail("Application paths must not contain null characters.") }
    let url: URL
    if value.hasPrefix("file:") {
        guard let parsed = URL(string: value), parsed.isFileURL,
              parsed.host == nil || parsed.host == "" || parsed.host == "localhost" else {
            throw fail("Expected a local file URL: \(value)")
        }
        url = parsed
    } else {
        guard value.hasPrefix("/") else { throw fail("Expected an absolute path: \(value)") }
        url = URL(fileURLWithPath: value)
    }
    guard !url.path.split(separator: "/").contains(where: { $0 == "." || $0 == ".." }) else {
        throw fail("Application paths must not contain dot segments: \(value)")
    }
    return url.standardizedFileURL
}

private func checkPath(_ url: URL) throws {
    var current = URL(fileURLWithPath: "/", isDirectory: true)
    for component in url.pathComponents.dropFirst() {
        current.appendPathComponent(component)
        if let status = try fileStatus(current), status.st_mode & mode_t(S_IFMT) == mode_t(S_IFLNK) {
            throw fail("Symbolic links are not allowed in update paths: \(current.path)")
        }
    }
}

private func checkApplication(_ url: URL, identifier: String, signed: Bool) throws {
    try checkPath(url)
    guard url.pathExtension.lowercased() == "app", let status = try fileStatus(url),
          status.st_mode & mode_t(S_IFMT) == mode_t(S_IFDIR) else {
        throw fail("The update is missing an application bundle: \(url.path)")
    }
    var enumerationError: Error?
    guard let children = files.enumerator(at: url, includingPropertiesForKeys: [.isSymbolicLinkKey],
                                         options: [], errorHandler: { _, error in
                                             enumerationError = error
                                             return false
                                         }) else {
        throw fail("Cannot inspect the application bundle: \(url.path)")
    }
    for case let child as URL in children {
        if let status = try fileStatus(child), status.st_mode & mode_t(S_IFMT) == mode_t(S_IFLNK) {
            throw fail("Symbolic links are not allowed inside application bundles: \(child.path)")
        }
    }
    if let error = enumerationError { throw error }
    guard Bundle(url: url)?.bundleIdentifier == identifier else {
        throw fail("Application identity does not match \(identifier): \(url.path)")
    }
    if signed {
        try command("/usr/bin/codesign", ["--verify", "--deep", "--strict", url.path])
    }
}

private func command(_ executable: String, _ arguments: [String]) throws {
    let process = Process()
    let output = Pipe()
    process.executableURL = URL(fileURLWithPath: executable)
    process.arguments = arguments
    process.standardOutput = output
    process.standardError = output
    try process.run()
    let data = output.fileHandleForReading.readDataToEndOfFile()
    process.waitUntilExit()
    guard process.terminationStatus == 0 else {
        let detail = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        throw fail("\(URL(fileURLWithPath: executable).lastPathComponent) failed for the update. \(detail)")
    }
}

private func writableParent(of url: URL) throws {
    try checkPath(url)
    let parent = url.deletingLastPathComponent()
    guard let status = try fileStatus(parent), status.st_mode & mode_t(S_IFMT) == mode_t(S_IFDIR),
          files.isWritableFile(atPath: parent.path) else {
        throw fail("Cannot write to \(parent.path). Move the complete suite to a writable Applications folder, or update it with Homebrew. No administrator access is requested.")
    }
}

private func isInside(_ url: URL, _ parent: URL) -> Bool {
    let candidate = url.path.lowercased()
    let boundary = parent.path.lowercased()
    return candidate == boundary || candidate.hasPrefix(boundary + "/")
}

private final class Replacement {
    let source: URL
    let destination: URL
    let identifier: String
    let stage: URL
    let backup: URL
    let original: stat?
    var movedOriginal = false
    var installed = false

    init(item: Manifest.Item) throws {
        source = try location(item.source)
        destination = try location(item.destination)
        identifier = item.bundleIdentifier
        try checkApplication(source, identifier: identifier, signed: true)
        guard destination.pathExtension.lowercased() == "app" else {
            throw fail("The update destination must be an .app bundle: \(destination.path)")
        }
        guard !destination.deletingLastPathComponent().pathComponents.contains(where: { $0.lowercased().hasSuffix(".app") }) else {
            throw fail("Update destinations must not be nested inside another application: \(destination.path)")
        }
        try writableParent(of: destination)
        original = try fileStatus(destination)
        if original != nil { try checkApplication(destination, identifier: identifier, signed: false) }
        let parent = destination.deletingLastPathComponent()
        let token = UUID().uuidString
        stage = parent.appendingPathComponent(".tosklight-update-\(token).app", isDirectory: true)
        backup = parent.appendingPathComponent(".tosklight-backup-\(token)-\(destination.lastPathComponent)", isDirectory: true)
    }

    func checkOriginal() throws {
        try writableParent(of: destination)
        let current = try fileStatus(destination)
        guard original?.st_dev == current?.st_dev, original?.st_ino == current?.st_ino else {
            throw fail("An application changed while the update was being prepared. Close the applications and try again: \(destination.path)")
        }
        if current != nil { try checkApplication(destination, identifier: identifier, signed: false) }
    }
}

private func waitForLauncher(_ pid: Int) throws {
    guard pid >= 0, pid <= Int(Int32.max), pid != Int(getpid()) else {
        throw fail("The launcher process identifier is invalid.")
    }
    if pid == 0 { return }
    let deadline = Date().addingTimeInterval(30)
    while kill(pid_t(pid), 0) == 0 || errno == EPERM {
        guard Date() < deadline else {
            throw fail("ToskLight Launcher did not quit within 30 seconds. Quit the launcher and try the update again.")
        }
        Thread.sleep(forTimeInterval: 0.1)
    }
}

private func checkOtherApplications() throws {
    let running = NSWorkspace.shared.runningApplications.filter {
        guard let identifier = $0.bundleIdentifier else { return false }
        return identifier != launcherIdentifier && knownIdentifiers.contains(identifier) && !$0.isTerminated
    }
    guard running.isEmpty else {
        let names = running.map { $0.localizedName ?? $0.bundleIdentifier ?? "ToskLight" }.sorted().joined(separator: ", ")
        throw fail("Quit these applications before updating: \(names). Their shows and settings have not been changed.")
    }
}

private func rollback(_ replacements: [Replacement]) -> [String] {
    var errors: [String] = []
    for replacement in replacements.reversed() {
        do {
            if replacement.installed {
                try checkPath(replacement.destination)
                try files.removeItem(at: replacement.destination)
                replacement.installed = false
            }
            if replacement.movedOriginal {
                try checkPath(replacement.destination)
                guard try fileStatus(replacement.destination) == nil else {
                    throw fail("The destination is occupied: \(replacement.destination.path)")
                }
                try files.moveItem(at: replacement.backup, to: replacement.destination)
                replacement.movedOriginal = false
            }
        } catch {
            errors.append("Could not restore \(replacement.destination.path): \(error.localizedDescription)")
        }
    }
    return errors
}

private func install(_ manifest: Manifest, resultURL: URL, progress: (String) -> Void) throws -> InstallResult {
    guard geteuid() != 0 else { throw fail("Run ToskLight Launcher as your normal macOS user, without sudo.") }
    guard manifest.items.count == 4, Set(manifest.items.map(\.bundleIdentifier)) == knownIdentifiers else {
        throw fail("The update must contain exactly one Control, Architect, Pixel, and Launcher application.")
    }
    // Validate every source and destination before creating staging or backup directories.
    let replacements = try manifest.items.map {
        progress("Checking \(applicationNames[$0.bundleIdentifier] ?? "application")…")
        return try Replacement(item: $0)
    }
    let launcher = try location(manifest.launcherURL)
    guard replacements.first(where: { $0.identifier == launcherIdentifier })?.destination == launcher else {
        throw fail("The launcher reopen path must match its update destination.")
    }
    for (index, replacement) in replacements.enumerated() {
        guard !isInside(resultURL, replacement.source), !isInside(resultURL, replacement.destination) else {
            throw fail("The update result must be stored outside application bundles.")
        }
        for other in replacements {
            guard !isInside(replacement.source, other.destination), !isInside(other.destination, replacement.source) else {
                throw fail("Update sources and destinations must be separate application bundles.")
            }
        }
        for other in replacements.dropFirst(index + 1) {
            guard !isInside(replacement.destination, other.destination), !isInside(other.destination, replacement.destination) else {
                throw fail("Update destinations must be four separate application bundles.")
            }
        }
    }
    progress("Waiting for ToskLight Launcher to quit…")
    try waitForLauncher(manifest.launcherPID)
    try checkOtherApplications()
    defer {
        for replacement in replacements {
            try? files.removeItem(at: replacement.stage)
        }
    }
    do {
        for replacement in replacements {
            progress("Preparing \(applicationNames[replacement.identifier] ?? "application")…")
            try checkApplication(replacement.source, identifier: replacement.identifier, signed: true)
            try writableParent(of: replacement.destination)
            guard try fileStatus(replacement.stage) == nil else { throw fail("The update staging path is occupied.") }
            try files.copyItem(at: replacement.source, to: replacement.stage)
            try checkApplication(replacement.stage, identifier: replacement.identifier, signed: true)
        }
        try checkOtherApplications()
        for replacement in replacements { try replacement.checkOriginal() }
        // Launcher is replaced last so any earlier failure leaves its original bundle available.
        let ordered = replacements.sorted { $0.identifier != launcherIdentifier && $1.identifier == launcherIdentifier }
        for replacement in ordered {
            progress("Installing \(applicationNames[replacement.identifier] ?? "application")…")
            try checkOtherApplications()
            try replacement.checkOriginal()
            if replacement.original != nil {
                try files.moveItem(at: replacement.destination, to: replacement.backup)
                replacement.movedOriginal = true
            }
            try files.moveItem(at: replacement.stage, to: replacement.destination)
            replacement.installed = true
        }
    } catch {
        progress("Restoring the previous applications…")
        let rollbackErrors = rollback(replacements)
        let retained = replacements.filter { (try? fileStatus($0.backup)) != nil }.map { $0.backup.path }
        let detail = rollbackErrors.isEmpty
            ? "All replaced applications were restored."
            : "Some applications could not be restored. Keep the listed backups. " + rollbackErrors.joined(separator: " ")
        return InstallResult(success: false, message: "\(failureMessage(error)) \(detail)", backups: retained)
    }
    var retained: [String] = []
    for replacement in replacements where replacement.movedOriginal {
        do { try files.removeItem(at: replacement.backup) }
        catch { retained.append(replacement.backup.path) }
    }
    let message = retained.isEmpty
        ? "ToskLight Control, Architect, Pixel, and Launcher were updated. Shows and settings were preserved."
        : "The suite was updated. Previous application backups could not be removed and are listed in the result. Shows and settings were preserved."
    return InstallResult(success: true, message: message, backups: retained)
}

private final class InstallProgress {
    private let window: NSWindow
    private let detail = NSTextField(wrappingLabelWithString: "Checking the downloaded applications…")

    init() {
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 150),
                          styleMask: [.titled], backing: .buffered, defer: false)
        window.title = "ToskLight Update"
        window.isReleasedWhenClosed = false
        let content = NSStackView()
        content.orientation = .vertical
        content.alignment = .leading
        content.spacing = 14
        content.translatesAutoresizingMaskIntoConstraints = false
        let title = NSTextField(labelWithString: "Installing ToskLight updates…")
        title.font = .systemFont(ofSize: 17, weight: .semibold)
        let indicator = NSProgressIndicator()
        indicator.style = .bar
        indicator.isIndeterminate = true
        indicator.startAnimation(nil)
        detail.textColor = .secondaryLabelColor
        content.addArrangedSubview(title)
        content.addArrangedSubview(detail)
        content.addArrangedSubview(indicator)
        window.contentView!.addSubview(content)
        NSLayoutConstraint.activate([
            content.leadingAnchor.constraint(equalTo: window.contentView!.leadingAnchor, constant: 24),
            content.trailingAnchor.constraint(equalTo: window.contentView!.trailingAnchor, constant: -24),
            content.centerYAnchor.constraint(equalTo: window.contentView!.centerYAnchor),
            indicator.widthAnchor.constraint(equalTo: content.widthAnchor)
        ])
        window.center()
        window.makeKeyAndOrderFront(nil)
        NSApplication.shared.activate(ignoringOtherApps: true)
    }

    func update(_ message: String) {
        DispatchQueue.main.async { self.detail.stringValue = message }
    }

    func close() { window.close() }
}

@main
private struct InstallUpdate {
    private static func outcome(_ manifest: Manifest, resultURL: URL, progress: (String) -> Void = { _ in }) -> InstallResult {
        do { return try install(manifest, resultURL: resultURL, progress: progress) }
        catch { return InstallResult(success: false, message: failureMessage(error), backups: []) }
    }

    static func main() {
        var manifest: Manifest?
        var resultURL: URL?
        do {
            guard CommandLine.arguments.count == 2 else { throw fail("Usage: InstallUpdate MANIFEST.json") }
            let input = try location(CommandLine.arguments[1])
            try checkPath(input)
            let decoded = try JSONDecoder().decode(Manifest.self, from: Data(contentsOf: input))
            manifest = decoded
            let output = try location(decoded.resultURL)
            try writableParent(of: output)
            guard try fileStatus(output) == nil else { throw fail("The update result path already exists. Start a fresh update from the launcher.") }
            guard !output.pathComponents.contains(where: { $0.lowercased().hasSuffix(".app") }) else {
                throw fail("The update result must be stored outside application bundles.")
            }
            for item in decoded.items {
                let source = try location(item.source)
                let destination = try location(item.destination)
                guard !isInside(output, source), !isInside(output, destination) else {
                    throw fail("The update result must be stored outside application bundles.")
                }
            }
            resultURL = output
        } catch {
            finish(InstallResult(success: false, message: failureMessage(error), backups: []),
                   manifest: manifest, resultURL: resultURL)
            return
        }
        guard let manifest = manifest, let resultURL = resultURL else { exit(1) }
        if manifest.reopen == false {
            finish(outcome(manifest, resultURL: resultURL), manifest: manifest, resultURL: resultURL)
            return
        }
        let application = NSApplication.shared
        application.setActivationPolicy(.accessory)
        let progress = InstallProgress()
        DispatchQueue.global(qos: .userInitiated).async {
            let result = outcome(manifest, resultURL: resultURL, progress: progress.update)
            DispatchQueue.main.async {
                progress.close()
                finish(result, manifest: manifest, resultURL: resultURL)
            }
        }
        application.run()
    }

    private static func finish(_ result: InstallResult, manifest: Manifest?, resultURL: URL?) {
        var savedResult = false
        do {
            guard let output = resultURL else { throw fail(result.message) }
            try checkPath(output)
            let data = try JSONEncoder().encode(result)
            try data.write(to: output, options: .atomic)
            savedResult = true
        } catch {
            FileHandle.standardError.write(Data((error.localizedDescription + "\n").utf8))
        }
        // Validation can fail before the launcher's delayed quit. Do not send the
        // result to that old process and then lose it when the old launcher exits.
        if let manifest = manifest {
            do { try waitForLauncher(manifest.launcherPID) }
            catch {
                FileHandle.standardError.write(Data((error.localizedDescription + "\n").utf8))
                exit(1)
            }
        }
        if let manifest = manifest, manifest.reopen != false,
           let launcher = try? location(manifest.launcherURL),
           (try? checkApplication(launcher, identifier: launcherIdentifier, signed: false)) != nil {
            let arguments = ["-a", launcher.path] + (resultURL.map { ["--args", "--update-result", $0.path] } ?? [])
            do { try command("/usr/bin/open", arguments) }
            catch { FileHandle.standardError.write(Data(("Could not reopen ToskLight Launcher: \(error.localizedDescription)\n").utf8)) }
        }
        exit(result.success && savedResult ? 0 : 1)
    }
}
