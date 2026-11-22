import AppKit

struct Product: Decodable {
    let id: String
    let name: String
    let role: String
    let bundleIdentifier: String
    let icon: String
}

struct ApplicationCatalog {
    let products: [Product]
    let directories: [URL]

    init(bundle: Bundle = .main, environment: [String: String] = ProcessInfo.processInfo.environment) throws {
        guard let manifest = bundle.url(forResource: "products", withExtension: "json") else {
            throw NSError(domain: "ToskLightLauncher", code: 1, userInfo: [NSLocalizedDescriptionKey: "The launcher is missing its application catalog. Reinstall ToskLight Launcher."])
        }
        products = try JSONDecoder().decode([Product].self, from: Data(contentsOf: manifest))
        if let directory = environment["TOSKLIGHT_LAUNCHER_APPLICATIONS_DIR"] {
            directories = [URL(fileURLWithPath: directory, isDirectory: true)]
        } else {
            directories = [bundle.bundleURL.deletingLastPathComponent(),
                           FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Applications"),
                           URL(fileURLWithPath: "/Applications", isDirectory: true)]
        }
    }

    func application(for product: Product) -> URL? {
        for directory in directories {
            let candidates = (try? FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil, options: [.skipsHiddenFiles])) ?? []
            for candidate in candidates.sorted(by: { $0.lastPathComponent < $1.lastPathComponent }) {
                guard candidate.pathExtension == "app",
                      Bundle(url: candidate)?.bundleIdentifier == product.bundleIdentifier else { continue }
                return candidate
            }
        }
        return nil
    }

    func installedVersion(for product: Product) -> String? {
        guard let url = application(for: product),
              let data = try? Data(contentsOf: url.appendingPathComponent("Contents/Info.plist")),
              let plist = (try? PropertyListSerialization.propertyList(from: data, format: nil)) as? [String: Any] else { return nil }
        return plist["CFBundleShortVersionString"] as? String
    }
}

final class LauncherDelegate: NSObject, NSApplicationDelegate {
    let catalog: ApplicationCatalog
    private var window: NSWindow!
    private var buttons: [NSButton] = []
    private var installedLabels: [NSTextField] = []
    private var latestLabels: [NSTextField] = []
    private var updateLinks: [NSButton] = []
    private let status = NSTextField(wrappingLabelWithString: "")
    private let checkButton = NSButton(title: "Check for updates", target: nil, action: nil)
    private let updateButton = NSButton(title: "Update all ToskLight applications", target: nil, action: nil)
    private let cancelButton = NSButton(title: "Cancel", target: nil, action: nil)
    private let progress = NSProgressIndicator()
    private let updateClient = SuiteUpdateClient()
    private var release: SuiteRelease?
    private var busy = false
    private var checking = false
    private var canceling = false

    init(catalog: ApplicationCatalog) { self.catalog = catalog }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let appMenu = NSMenu()
        appMenu.addItem(withTitle: "About ToskLight Launcher", action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
        let chooseItem = appMenu.addItem(withTitle: "Show launcher", action: #selector(showChooser), keyEquivalent: "1")
        chooseItem.target = self
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Quit ToskLight Launcher", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        let menu = NSMenu()
        let item = NSMenuItem()
        item.submenu = appMenu
        menu.addItem(item)
        NSApp.mainMenu = menu

        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 740, height: 550),
                          styleMask: [.titled, .closable, .miniaturizable], backing: .buffered, defer: false)
        window.title = "ToskLight Launcher"
        window.isReleasedWhenClosed = false
        window.center()
        let content = NSStackView()
        content.orientation = .vertical
        content.alignment = .leading
        content.spacing = 12
        content.edgeInsets = NSEdgeInsets(top: 26, left: 26, bottom: 22, right: 26)
        content.translatesAutoresizingMaskIntoConstraints = false
        window.contentView!.addSubview(content)
        NSLayoutConstraint.activate([
            content.leadingAnchor.constraint(equalTo: window.contentView!.leadingAnchor),
            content.trailingAnchor.constraint(equalTo: window.contentView!.trailingAnchor),
            content.topAnchor.constraint(equalTo: window.contentView!.topAnchor),
            content.bottomAnchor.constraint(equalTo: window.contentView!.bottomAnchor)
        ])
        let titleRow = NSStackView()
        titleRow.orientation = .horizontal
        titleRow.spacing = 16
        let title = NSTextField(labelWithString: "ToskLight")
        title.font = .systemFont(ofSize: 27, weight: .semibold)
        titleRow.addArrangedSubview(title)
        let launcherVersion = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "Unknown"
        let versionLabel = NSTextField(labelWithString: "Launcher version \(launcherVersion)")
        versionLabel.textColor = .secondaryLabelColor
        versionLabel.font = .systemFont(ofSize: 13)
        titleRow.addArrangedSubview(versionLabel)
        content.addArrangedSubview(titleRow)
        let subtitle = NSTextField(labelWithString: "Choose an application.")
        subtitle.font = .systemFont(ofSize: 15)
        subtitle.textColor = .secondaryLabelColor
        content.addArrangedSubview(subtitle)

        let choices = NSStackView()
        choices.orientation = .horizontal
        choices.distribution = .fillEqually
        choices.alignment = .top
        choices.spacing = 12
        for (index, product) in catalog.products.enumerated() {
            let button = NSButton(title: product.name, target: self, action: #selector(openProduct(_:)))
            button.tag = index
            button.bezelStyle = .regularSquare
            button.imagePosition = .imageAbove
            button.imageScaling = .scaleProportionallyDown
            button.font = .systemFont(ofSize: 15, weight: .medium)
            button.toolTip = product.role
            button.setAccessibilityLabel(product.name)
            if let url = Bundle.main.url(forResource: product.icon, withExtension: nil), let image = NSImage(contentsOf: url) {
                image.size = NSSize(width: 72, height: 72)
                button.image = image
            }
            button.heightAnchor.constraint(equalToConstant: 152).isActive = true
            let column = NSStackView()
            column.orientation = .vertical
            column.alignment = .centerX
            column.spacing = 8
            column.addArrangedSubview(button)
            let role = NSTextField(labelWithString: product.role)
            role.font = .systemFont(ofSize: 13)
            role.textColor = .secondaryLabelColor
            column.addArrangedSubview(role)
            let installed = NSTextField(labelWithString: "Installed: …")
            installed.font = .systemFont(ofSize: 13)
            column.addArrangedSubview(installed)
            installedLabels.append(installed)
            let latest = NSTextField(labelWithString: "Latest: checking…")
            latest.font = .systemFont(ofSize: 13)
            latest.textColor = .secondaryLabelColor
            column.addArrangedSubview(latest)
            latestLabels.append(latest)
            let update = NSButton(title: "Update", target: self, action: #selector(updateAll))
            update.tag = index
            update.isBordered = false
            update.attributedTitle = NSAttributedString(string: "Update", attributes: [
                .foregroundColor: NSColor.linkColor, .underlineStyle: NSUnderlineStyle.single.rawValue
            ])
            update.setAccessibilityLabel("Update \(product.name)")
            update.toolTip = "Install the latest complete ToskLight suite."
            update.isHidden = true
            column.addArrangedSubview(update)
            updateLinks.append(update)
            button.widthAnchor.constraint(equalTo: column.widthAnchor).isActive = true
            choices.addArrangedSubview(column)
            buttons.append(button)
        }
        content.addArrangedSubview(choices)
        choices.widthAnchor.constraint(equalTo: content.widthAnchor, constant: -52).isActive = true
        let updates = NSStackView()
        updates.orientation = .horizontal
        updates.spacing = 12
        checkButton.target = self
        checkButton.action = #selector(checkForUpdates)
        checkButton.bezelStyle = .rounded
        updateButton.target = self
        updateButton.action = #selector(updateAll)
        updateButton.bezelStyle = .rounded
        updateButton.isEnabled = false
        let changeLog = NSButton(title: "Change log", target: self, action: #selector(openChangeLog))
        changeLog.bezelStyle = .rounded
        for button in [checkButton, updateButton, changeLog] { updates.addArrangedSubview(button) }
        cancelButton.target = self
        cancelButton.action = #selector(cancelUpdate)
        cancelButton.bezelStyle = .rounded
        cancelButton.isHidden = true
        updates.addArrangedSubview(cancelButton)
        content.addArrangedSubview(updates)
        progress.style = .bar
        progress.minValue = 0
        progress.maxValue = 1
        progress.isHidden = true
        content.addArrangedSubview(progress)
        progress.widthAnchor.constraint(equalTo: content.widthAnchor, constant: -52).isActive = true
        status.font = .systemFont(ofSize: 13)
        status.textColor = .secondaryLabelColor
        content.addArrangedSubview(status)
        status.widthAnchor.constraint(equalTo: content.widthAnchor, constant: -52).isActive = true
        let downloads = NSButton(title: "Download the suite…", target: self, action: #selector(openDownloads))
        downloads.bezelStyle = .rounded
        downloads.controlSize = .small
        content.addArrangedSubview(downloads)
        showLauncher()
        if !showUpdateResult() { checkForUpdates() }
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        showLauncher()
        return true
    }

    private func showLauncher() {
        let missing = catalog.products.filter { catalog.application(for: $0) == nil }
        if !busy && !checking {
            status.stringValue = missing.isEmpty
                ? "Pin this launcher in the Dock. Each application keeps its own icon while running."
                : "Not installed here: \(missing.map(\.name).joined(separator: ", ")). Download the macOS suite to add them."
        }
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        refreshVersions()
    }

    @objc private func showChooser() { showLauncher() }

    private func refreshVersions() {
        var differing = release.map {
            (Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String) != $0.version
        } ?? false
        for (index, product) in catalog.products.enumerated() {
            let installed = catalog.installedVersion(for: product)
            installedLabels[index].stringValue = installed.map { "Installed: \($0)" }
                ?? (catalog.application(for: product) == nil ? "Not installed" : "Installed: unknown version")
            latestLabels[index].stringValue = checking ? "Latest: checking…" : (release.map { "Latest: \($0.version)" } ?? "Latest: not checked")
            let mismatch = release.map { installed != $0.version } ?? false
            differing = differing || mismatch
            updateLinks[index].isHidden = !mismatch || checking
            updateLinks[index].isEnabled = !busy && !checking
        }
        updateButton.isEnabled = differing && !busy && !checking
        checkButton.isEnabled = !busy && !checking
    }

    @objc private func checkForUpdates() {
        guard !busy && !checking else { return }
        checking = true
        refreshVersions()
        status.stringValue = "Checking GitHub Releases…"
        updateClient.check { [weak self] result in
            guard let self = self else { return }
            self.checking = false
            switch result {
            case .success(let found):
                self.release = found
                self.status.stringValue = "Latest GitHub release: \(found.version). Update installs the complete suite; close product applications first."
            case .failure(let error):
                self.release = nil
                self.status.stringValue = "Could not check for updates: \(error.localizedDescription) Try Check for updates again."
            }
            self.refreshVersions()
        }
    }

    @objc private func openChangeLog() {
        let url = release?.notesURL ?? URL(string: "https://github.com/kellertobias/tosklight/releases/latest")!
        if !NSWorkspace.shared.open(url) { showError("Could not open the change log.", detail: url.absoluteString) }
    }

    private func runningProducts() -> [Product] {
        catalog.products.filter { product in
            NSWorkspace.shared.runningApplications.contains { $0.bundleIdentifier == product.bundleIdentifier }
        }
    }

    @objc private func updateAll() {
        guard let latest = release, !busy else { return }
        let running = runningProducts()
        guard running.isEmpty else {
            showError("Close the applications before updating.", detail: "Save your work and quit \(running.map(\.name).joined(separator: ", ")), then click Update again.")
            return
        }
        busy = true
        canceling = false
        cancelButton.isHidden = false
        cancelButton.isEnabled = true
        buttons.forEach { $0.isEnabled = false }
        refreshVersions()
        progress.isHidden = false
        progress.isIndeterminate = true
        progress.startAnimation(nil)
        updateClient.prepare(release: latest, progress: { [weak self] message, fraction in
            guard let self = self, !self.canceling else { return }
            self.status.stringValue = message
            self.progress.isIndeterminate = fraction == nil
            if let fraction = fraction { self.progress.doubleValue = fraction }
        }, completion: { [weak self] result in
            guard let self = self else { return }
            if self.canceling {
                if case .success(let prepared) = result { try? prepared.cleanup() }
                self.busy = false
                self.canceling = false
                self.buttons.forEach { $0.isEnabled = true }
                self.progress.stopAnimation(nil)
                self.progress.isHidden = true
                self.cancelButton.isHidden = true
                self.refreshVersions()
                self.status.stringValue = "Update canceled. Installed applications were not changed."
                return
            }
            switch result {
            case .success(let prepared):
                do { try self.install(prepared) }
                catch { try? prepared.cleanup(); self.updateFailed(error) }
            case .failure(let error): self.updateFailed(error)
            }
        })
    }

    private func updateFailed(_ error: Error) {
        busy = false
        buttons.forEach { $0.isEnabled = true }
        progress.stopAnimation(nil)
        progress.isHidden = true
        cancelButton.isHidden = true
        refreshVersions()
        showError("Could not update ToskLight.", detail: error.localizedDescription)
    }

    private func install(_ prepared: PreparedSuiteUpdate) throws {
        guard runningProducts().isEmpty else {
            throw NSError(domain: "ToskLightLauncher", code: 2, userInfo: [NSLocalizedDescriptionKey: "A product application started during the download. Quit it and try Update again."])
        }
        let current = Bundle.main.bundleURL.resolvingSymlinksInPath()
        let parent = current.deletingLastPathComponent()
        var items: [[String: String]] = []
        for product in catalog.products {
            guard let source = prepared.applications[product.id] else { throw updateError("The release is missing \(product.name).") }
            let destination = (catalog.application(for: product) ?? parent.appendingPathComponent("\(product.name).app")).resolvingSymlinksInPath()
            items.append(["source": source.resolvingSymlinksInPath().path, "destination": destination.path, "bundleIdentifier": product.bundleIdentifier])
        }
        guard let newLauncher = prepared.applications["launcher"],
              let helperSource = Bundle.main.url(forAuxiliaryExecutable: "tosklight-suite-update") else {
            throw updateError("The update is missing ToskLight Launcher or its installer. Download a fresh macOS suite.")
        }
        items.append(["source": newLauncher.resolvingSymlinksInPath().path, "destination": current.path, "bundleIdentifier": "de.tokenet.tosklight.launcher"])
        for item in items {
            let destinationParent = URL(fileURLWithPath: item["destination"]!).deletingLastPathComponent().path
            guard FileManager.default.isWritableFile(atPath: destinationParent) else {
                throw updateError("Cannot write to \(destinationParent). Update through Homebrew, or move the complete suite to your own Applications folder.")
            }
        }
        let directory = prepared.directory.resolvingSymlinksInPath()
        let helper = directory.appendingPathComponent("tosklight-suite-update")
        try FileManager.default.copyItem(at: helperSource, to: helper)
        let result = directory.appendingPathComponent("update-result.json")
        let manifest: [String: Any] = ["launcherPID": ProcessInfo.processInfo.processIdentifier,
                                       "launcherURL": current.path, "resultURL": result.path, "items": items]
        let manifestURL = directory.appendingPathComponent("install.json")
        try JSONSerialization.data(withJSONObject: manifest, options: [.prettyPrinted]).write(to: manifestURL, options: .atomic)
        let process = Process()
        process.executableURL = helper
        process.arguments = [manifestURL.path]
        let logURL = directory.appendingPathComponent("installer.log")
        FileManager.default.createFile(atPath: logURL.path, contents: nil)
        let log = try FileHandle(forWritingTo: logURL)
        process.standardOutput = log
        process.standardError = log
        try process.run()
        cancelButton.isHidden = true
        status.stringValue = "Installing updates. The launcher will close and reopen when installation finishes."
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { NSApp.terminate(nil) }
    }

    @objc private func cancelUpdate() {
        guard busy else { return }
        canceling = true
        cancelButton.isEnabled = false
        status.stringValue = "Canceling the update…"
        updateClient.cancelPreparation()
    }

    private func updateError(_ message: String) -> Error {
        NSError(domain: "ToskLightLauncher", code: 3, userInfo: [NSLocalizedDescriptionKey: message])
    }

    private func showUpdateResult() -> Bool {
        guard let index = CommandLine.arguments.firstIndex(of: "--update-result"), CommandLine.arguments.count > index + 1 else { return false }
        let result = URL(fileURLWithPath: CommandLine.arguments[index + 1]).resolvingSymlinksInPath()
        let updates = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/ToskLight Launcher/Updates").resolvingSymlinksInPath()
        guard result.path.hasPrefix(updates.path + "/"),
              let data = try? Data(contentsOf: result),
              let report = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return false }
        let message = report["message"] as? String ?? "The update finished."
        let success = report["success"] as? Bool == true
        let backups = report["backups"] as? [String] ?? []
        status.stringValue = message
        let alert = NSAlert()
        alert.messageText = success ? "ToskLight was updated." : "The update could not be completed."
        alert.informativeText = message + (backups.isEmpty ? "" : "\nPrevious copies: \(backups.joined(separator: "\n"))")
            + (success ? "" : "\nDetails: \(result.path)")
        alert.addButton(withTitle: "OK")
        alert.beginSheetModal(for: window) { [weak self] _ in
            if success && backups.isEmpty { try? FileManager.default.removeItem(at: result.deletingLastPathComponent()) }
            self?.checkForUpdates()
        }
        return true
    }

    @objc private func openProduct(_ sender: NSButton) {
        let product = catalog.products[sender.tag]
        guard let application = catalog.application(for: product) else {
            showError("\(product.name) is not installed.", detail: "Keep the launcher beside the applications from the macOS suite, or install them in Applications.")
            return
        }
        buttons.forEach { $0.isEnabled = false }
        status.stringValue = "Opening \(product.name)…"
        let configuration = NSWorkspace.OpenConfiguration()
        configuration.activates = true
        configuration.createsNewApplicationInstance = false
        configuration.allowsRunningApplicationSubstitution = false
        NSWorkspace.shared.openApplication(at: application, configuration: configuration) { [weak self] _, error in
            DispatchQueue.main.async {
                guard let self = self else { return }
                self.buttons.forEach { $0.isEnabled = true }
                if let error = error {
                    self.showError("Could not open \(product.name).", detail: error.localizedDescription)
                } else {
                    self.window.orderOut(nil)
                }
            }
        }
    }

    private func showError(_ message: String, detail: String) {
        status.stringValue = message
        let alert = NSAlert()
        alert.messageText = message
        alert.informativeText = detail
        alert.addButton(withTitle: "OK")
        alert.beginSheetModal(for: window)
    }

    @objc private func openDownloads() {
        let url = URL(string: "https://tosklight.de/downloads/#homebrew")!
        if !NSWorkspace.shared.open(url) { showError("Could not open the downloads page.", detail: url.absoluteString) }
    }
}

do {
    let catalog = try ApplicationCatalog()
    if CommandLine.arguments.contains("--verify") {
        let records = catalog.products.map { product -> [String: String] in
            ["id": product.id, "name": product.name, "application": catalog.application(for: product)?.path ?? ""]
        }
        let data = try JSONSerialization.data(withJSONObject: records, options: [.prettyPrinted, .sortedKeys])
        FileHandle.standardOutput.write(data)
        exit(0)
    }
    let app = NSApplication.shared
    app.setActivationPolicy(.regular)
    let delegate = LauncherDelegate(catalog: catalog)
    app.delegate = delegate
    withExtendedLifetime(delegate) { app.run() }
} catch {
    if CommandLine.arguments.contains("--verify") {
        FileHandle.standardError.write(Data("\(error.localizedDescription)\n".utf8))
        exit(1)
    }
    let app = NSApplication.shared
    app.setActivationPolicy(.regular)
    let alert = NSAlert()
    alert.messageText = "Could not start ToskLight Launcher."
    alert.informativeText = error.localizedDescription
    alert.runModal()
    exit(1)
}
