import Foundation
import CryptoKit

enum SuiteUpdateError: LocalizedError {
    case release(String)
    case download(String)
    case archive(String)
    case verification(String)
    case busy

    var errorDescription: String? {
        switch self {
        case .release(let message), .download(let message), .archive(let message), .verification(let message): return message
        case .busy: return "An update is already being prepared."
        }
    }
}

struct SuiteRelease: Equatable {
    let version: String
    let tag: String
    let notesURL: URL
    let assetURL: URL
    let sha256: String
    let size: Int64

    static let assetName = "tosklight-bundle-macos_arm64.zip"
    static let repositoryPath = "/kellertobias/tosklight"

    static func parse(data: Data) throws -> SuiteRelease {
        struct Asset: Decodable { let name: String; let browser_download_url: String; let digest: String?; let size: Int64 }
        struct Release: Decodable { let tag_name: String; let html_url: String; let draft: Bool; let prerelease: Bool; let assets: [Asset] }
        let release: Release
        do { release = try JSONDecoder().decode(Release.self, from: data) }
        catch { throw SuiteUpdateError.release("GitHub returned an unreadable release. Try checking for updates again.") }
        guard !release.draft && !release.prerelease,
              release.tag_name.range(of: #"^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$"#, options: .regularExpression) != nil else {
            throw SuiteUpdateError.release("GitHub did not return a published stable ToskLight release.")
        }
        let tag = release.tag_name
        let version = String(tag.dropFirst())
        let notes = try githubURL(release.html_url, path: "\(repositoryPath)/releases/tag/\(tag)")
        let assets = release.assets.filter { $0.name == assetName }
        guard assets.count == 1, let asset = assets.first, asset.size > 0,
              let digest = asset.digest,
              digest.range(of: #"^sha256:[a-f0-9]{64}$"#, options: .regularExpression) != nil else {
            throw SuiteUpdateError.release("The GitHub release is missing the macOS suite or its SHA-256 checksum. Try again after release publication finishes.")
        }
        let download = try githubURL(asset.browser_download_url, path: "\(repositoryPath)/releases/download/\(tag)/\(assetName)")
        return SuiteRelease(version: version, tag: tag, notesURL: notes, assetURL: download,
                            sha256: String(digest.dropFirst(7)), size: asset.size)
    }

    private static func githubURL(_ text: String, path: String) throws -> URL {
        guard let url = URL(string: text), let parts = URLComponents(url: url, resolvingAgainstBaseURL: false),
              parts.scheme == "https", parts.host == "github.com", parts.port == nil,
              parts.user == nil, parts.password == nil, parts.query == nil, parts.fragment == nil,
              parts.path == path else {
            throw SuiteUpdateError.release("The release contains an unexpected download or change-log address. No files were downloaded.")
        }
        return url
    }
}

struct PreparedSuiteUpdate {
    let directory: URL
    let applications: [String: URL]

    func cleanup() throws { try FileManager.default.removeItem(at: directory) }
}

/// Downloads and verifies a complete suite; application replacement belongs to the installer helper.
final class SuiteUpdateClient: NSObject, URLSessionDownloadDelegate, @unchecked Sendable {
    static let releaseEndpoint = URL(string: "https://api.github.com/repos/kellertobias/tosklight/releases/latest")!
    static let applicationIdentifiers = [
        "de.tokenet.light": "control",
        "de.tokenet.tosklight.visualizer": "architect",
        "de.tokenet.tosklight.media": "pixel",
        "de.tokenet.tosklight.launcher": "launcher"
    ]

    private struct Download {
        let release: SuiteRelease
        let directory: URL
        let progress: (String, Double?) -> Void
        let completion: (Result<PreparedSuiteUpdate, Error>) -> Void
    }

    private let scratchDirectory: URL
    private let endpoint: URL
    private let signingHelperURL: URL?
    private var session: URLSession!
    private let lock = NSLock()
    private var active: Download?
    private var taskIdentifier: Int?
    private var verifying = false
    private var lastProgressTime: TimeInterval = 0
    private var cancellationRequested = false

    init(scratchDirectory: URL? = nil, releaseEndpoint: URL = SuiteUpdateClient.releaseEndpoint,
         configuration: URLSessionConfiguration? = nil,
         signingHelperURL: URL? = Bundle.main.url(forResource: "sign-macos-apps-locally", withExtension: "sh")) {
        self.scratchDirectory = scratchDirectory ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("ToskLight Launcher/Updates", isDirectory: true)
        self.endpoint = releaseEndpoint
        self.signingHelperURL = signingHelperURL
        super.init()
        let settings = configuration ?? URLSessionConfiguration.ephemeral
        settings.timeoutIntervalForRequest = 60
        settings.timeoutIntervalForResource = 30 * 60
        session = URLSession(configuration: settings, delegate: self, delegateQueue: nil)
    }

    func check(completion: @escaping (Result<SuiteRelease, Error>) -> Void) {
        var request = URLRequest(url: endpoint, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 30)
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        request.setValue("ToskLight-Launcher", forHTTPHeaderField: "User-Agent")
        session.dataTask(with: request) { data, response, error in
            let result: Result<SuiteRelease, Error>
            if let error = error { result = .failure(error) }
            else if let response = response as? HTTPURLResponse, response.statusCode != 200 {
                result = .failure(SuiteUpdateError.release("Could not check GitHub Releases (HTTP \(response.statusCode)). Try again later."))
            } else if let data = data {
                result = Result { try SuiteRelease.parse(data: data) }
            } else { result = .failure(SuiteUpdateError.release("GitHub returned no release information. Try again.")) }
            DispatchQueue.main.async { completion(result) }
        }.resume()
    }

    func prepare(release: SuiteRelease, progress: @escaping (String, Double?) -> Void,
                 completion: @escaping (Result<PreparedSuiteUpdate, Error>) -> Void) {
        #if !arch(arm64)
        DispatchQueue.main.async {
            completion(.failure(SuiteUpdateError.release("Published macOS suite updates require Apple Silicon. Keep using your source-built applications on this Mac.")))
        }
        return
        #endif
        lock.lock()
        guard active == nil else {
            lock.unlock()
            DispatchQueue.main.async { completion(.failure(SuiteUpdateError.busy)) }
            return
        }
        let directory = scratchDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        active = Download(release: release, directory: directory, progress: progress, completion: completion)
        verifying = false
        lastProgressTime = 0
        cancellationRequested = false
        lock.unlock()
        do {
            guard let signingHelperURL = signingHelperURL,
                  FileManager.default.isReadableFile(atPath: signingHelperURL.path) else {
                throw SuiteUpdateError.verification("ToskLight Launcher is missing its local-signing helper. Reinstall the launcher before updating the suite.")
            }
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            var request = URLRequest(url: release.assetURL)
            request.setValue("ToskLight-Launcher", forHTTPHeaderField: "User-Agent")
            let task = session.downloadTask(with: request)
            lock.lock(); taskIdentifier = task.taskIdentifier; lock.unlock()
            DispatchQueue.main.async { progress("Downloading ToskLight \(release.version)…", 0) }
            task.resume()
        } catch { finish(.failure(error)) }
    }

    func cancelPreparation() {
        lock.lock()
        if active != nil { cancellationRequested = true }
        lock.unlock()
        session.getAllTasks { tasks in tasks.filter { $0 is URLSessionDownloadTask }.forEach { $0.cancel() } }
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask,
                    didWriteData bytesWritten: Int64, totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        lock.lock()
        guard taskIdentifier == downloadTask.taskIdentifier, !cancellationRequested, let download = active else { lock.unlock(); return }
        let expected = totalBytesExpectedToWrite > 0 ? totalBytesExpectedToWrite : download.release.size
        let fraction = min(1, Double(totalBytesWritten) / Double(expected))
        let now = ProcessInfo.processInfo.systemUptime
        guard now - lastProgressTime >= 0.1 || fraction == 1 else { lock.unlock(); return }
        lastProgressTime = now
        lock.unlock()
        DispatchQueue.main.async { download.progress("Downloading ToskLight \(download.release.version) (\(Int(fraction * 100))%)…", fraction) }
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        lock.lock()
        let download = active
        let matches = taskIdentifier == downloadTask.taskIdentifier
        if matches { verifying = true }
        lock.unlock()
        guard matches, let download = download else { return }
        do {
            guard let response = downloadTask.response as? HTTPURLResponse, response.statusCode == 200 else {
                throw SuiteUpdateError.download("The macOS suite download failed. No installed applications were changed.")
            }
            let archive = download.directory.appendingPathComponent(SuiteRelease.assetName)
            try FileManager.default.moveItem(at: location, to: archive)
            // The URLSession delegate queue is not the UI queue. Hashing, ZIP inspection, extraction
            // and signature checks all finish here before the main-queue completion is delivered.
            DispatchQueue.main.async { download.progress("Verifying the downloaded suite…", nil) }
            let prepared = try Self.prepareArchive(archive, release: download.release, directory: download.directory)
            guard let signingHelperURL = signingHelperURL else {
                throw SuiteUpdateError.verification("ToskLight Launcher is missing its local-signing helper. Reinstall the launcher before updating the suite.")
            }
            try checkCancellation()
            DispatchQueue.main.async { download.progress("Signing and verifying ToskLight…", nil) }
            try Self.applyLocalSigning(prepared, helperURL: signingHelperURL, shouldCancel: { self.isCanceled })
            finish(.success(prepared))
        } catch { finish(.failure(error)) }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        lock.lock(); let matches = taskIdentifier == task.taskIdentifier; let isVerifying = verifying; lock.unlock()
        if matches, !isVerifying, let error = error { finish(.failure(error)) }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        let allowed = ["api.github.com", "github.com", "release-assets.githubusercontent.com", "objects.githubusercontent.com"]
        guard request.url?.scheme == "https", let host = request.url?.host, allowed.contains(host) else {
            completionHandler(nil)
            return
        }
        completionHandler(request)
    }

    private func finish(_ result: Result<PreparedSuiteUpdate, Error>) {
        lock.lock()
        let download = active
        let resolved: Result<PreparedSuiteUpdate, Error> = cancellationRequested ? .failure(URLError(.cancelled)) : result
        active = nil; taskIdentifier = nil; verifying = false
        cancellationRequested = false
        lock.unlock()
        guard let download = download else { return }
        if case .failure = resolved { try? FileManager.default.removeItem(at: download.directory) }
        DispatchQueue.main.async { download.completion(resolved) }
    }

    private var isCanceled: Bool {
        lock.lock(); defer { lock.unlock() }
        return cancellationRequested
    }

    private func checkCancellation() throws {
        if isCanceled { throw URLError(.cancelled) }
    }

    static func applyLocalSigning(_ prepared: PreparedSuiteUpdate, helperURL: URL,
                                  shouldCancel: () -> Bool = { false }) throws {
        guard FileManager.default.isReadableFile(atPath: helperURL.path) else {
            throw SuiteUpdateError.verification("ToskLight Launcher is missing its local-signing helper. Reinstall the launcher before updating the suite.")
        }
        let products = ["control", "architect", "pixel", "launcher"]
        guard Set(prepared.applications.keys) == Set(products) else {
            throw SuiteUpdateError.verification("The prepared update does not contain the complete ToskLight suite.")
        }
        // Never use local signing to repair an invalid downloaded signature. Recheck every
        // original bundle before the first signing operation, then use the current launcher's helper.
        for product in products {
            if shouldCancel() { throw URLError(.cancelled) }
            try run("/usr/bin/codesign", ["--verify", "--deep", "--strict", prepared.applications[product]!.path])
        }
        for product in products {
            if shouldCancel() { throw URLError(.cancelled) }
            let application = prepared.applications[product]!
            try run("/bin/bash", [helperURL.path, application.path])
            try run("/usr/bin/codesign", ["--verify", "--deep", "--strict", application.path])
        }
        if shouldCancel() { throw URLError(.cancelled) }
    }

    static func prepareArchive(_ archive: URL, release: SuiteRelease, directory: URL) throws -> PreparedSuiteUpdate {
        let attributes = try FileManager.default.attributesOfItem(atPath: archive.path)
        guard (attributes[.size] as? NSNumber)?.int64Value == release.size else {
            throw SuiteUpdateError.verification("The downloaded suite size does not match GitHub. Try downloading the update again.")
        }
        let file = try FileHandle(forReadingFrom: archive)
        defer { try? file.close() }
        var hash = SHA256()
        while true {
            let data = try file.read(upToCount: 1024 * 1024) ?? Data()
            if data.isEmpty { break }
            hash.update(data: data)
        }
        let digest = hash.finalize().map { String(format: "%02x", $0) }.joined()
        guard digest == release.sha256 else {
            throw SuiteUpdateError.verification("The downloaded suite failed its SHA-256 check. No installed applications were changed.")
        }
        try validateArchive(archive)
        let extracted = directory.appendingPathComponent("extracted", isDirectory: true)
        guard !FileManager.default.fileExists(atPath: extracted.path) else {
            throw SuiteUpdateError.archive("The update staging folder is already in use. Try preparing the update again.")
        }
        try FileManager.default.createDirectory(at: extracted, withIntermediateDirectories: true)
        try run("/usr/bin/ditto", ["-x", "-k", archive.path, extracted.path])
        guard let enumerator = FileManager.default.enumerator(at: extracted, includingPropertiesForKeys: [.isSymbolicLinkKey], options: [.skipsHiddenFiles]) else {
            throw SuiteUpdateError.archive("Could not inspect the extracted macOS suite.")
        }
        var applications: [String: URL] = [:]
        var parent: URL?
        for case let candidate as URL in enumerator {
            if try candidate.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink == true {
                throw SuiteUpdateError.archive("The suite contains a symbolic link and cannot be installed safely.")
            }
            guard candidate.pathExtension == "app" else { continue }
            enumerator.skipDescendants()
            guard let bundle = Bundle(url: candidate), let identifier = bundle.bundleIdentifier,
                  let product = applicationIdentifiers[identifier], applications[product] == nil else {
                throw SuiteUpdateError.archive("The suite contains an unexpected or duplicate application.")
            }
            let folder = candidate.deletingLastPathComponent().standardizedFileURL
            if let expected = parent, expected != folder {
                throw SuiteUpdateError.archive("The suite applications are not in one installation folder.")
            }
            parent = folder
            guard bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String == release.version,
                  let executable = bundle.executableURL, FileManager.default.isExecutableFile(atPath: executable.path) else {
                throw SuiteUpdateError.verification("\(candidate.lastPathComponent) has an unexpected version or is missing its executable.")
            }
            try run("/usr/bin/codesign", ["--verify", "--deep", "--strict", candidate.path])
            applications[product] = candidate
        }
        guard applications.count == applicationIdentifiers.count else {
            throw SuiteUpdateError.archive("This release does not contain Control, Architect, Pixel, and ToskLight Launcher. Try again when a complete suite release is available.")
        }
        return PreparedSuiteUpdate(directory: directory, applications: applications)
    }

    /// Read ZIP metadata before extraction, including UNIX mode bits which identify symbolic links.
    static func validateArchive(_ archive: URL) throws {
        let file = try FileHandle(forReadingFrom: archive)
        defer { try? file.close() }
        let length = try file.seekToEnd()
        guard length >= 22 else { throw SuiteUpdateError.archive("The update is not a complete ZIP archive.") }
        let tailLength = Int(min(length, 65_557))
        try file.seek(toOffset: length - UInt64(tailLength))
        let tail = try file.read(upToCount: tailLength) ?? Data()
        guard tail.count >= 22 else { throw SuiteUpdateError.archive("The update ZIP directory is incomplete.") }
        func u16(_ data: Data, _ offset: Int) -> UInt16 { UInt16(data[offset]) | UInt16(data[offset + 1]) << 8 }
        func u32(_ data: Data, _ offset: Int) -> UInt32 { UInt32(u16(data, offset)) | UInt32(u16(data, offset + 2)) << 16 }
        var end: Int?
        for offset in stride(from: tail.count - 22, through: 0, by: -1) {
            if u32(tail, offset) == 0x06054b50 && offset + 22 + Int(u16(tail, offset + 20)) == tail.count { end = offset; break }
        }
        guard let end = end, u16(tail, end + 4) == 0, u16(tail, end + 6) == 0,
              u16(tail, end + 8) == u16(tail, end + 10), u16(tail, end + 10) != UInt16.max else {
            throw SuiteUpdateError.archive("The update ZIP format is unsupported or incomplete.")
        }
        let count = Int(u16(tail, end + 10))
        let size = u32(tail, end + 12), start = u32(tail, end + 16)
        let endPosition = length - UInt64(tailLength) + UInt64(end)
        guard start != UInt32.max, size != UInt32.max, UInt64(start) + UInt64(size) <= endPosition else {
            throw SuiteUpdateError.archive("The update ZIP directory is invalid.")
        }
        try file.seek(toOffset: UInt64(start))
        let directory = try file.read(upToCount: Int(size)) ?? Data()
        var offset = 0
        for _ in 0..<count {
            guard offset + 46 <= directory.count, u32(directory, offset) == 0x02014b50 else {
                throw SuiteUpdateError.archive("The update ZIP directory is incomplete.")
            }
            let nameLength = Int(u16(directory, offset + 28))
            let next = offset + 46 + nameLength + Int(u16(directory, offset + 30)) + Int(u16(directory, offset + 32))
            guard next <= directory.count,
                  let name = String(data: directory.subdata(in: offset + 46..<offset + 46 + nameLength), encoding: .utf8),
                  !name.isEmpty, !name.hasPrefix("/"), !name.contains("\\"), !name.contains("\0"), !name.contains(":"),
                  !name.split(separator: "/", omittingEmptySubsequences: false).contains("..") else {
                throw SuiteUpdateError.archive("The update contains an unsafe ZIP path.")
            }
            let mode = (u32(directory, offset + 38) >> 16) & 0xf000
            guard mode == 0 || mode == 0x8000 || mode == 0x4000,
                  u16(directory, offset + 8) & 1 == 0, u16(directory, offset + 34) == 0 else {
                throw SuiteUpdateError.archive("The update contains a symbolic link, special file, or unsupported ZIP entry.")
            }
            let localOffset = u32(directory, offset + 42)
            guard localOffset != UInt32.max, UInt64(localOffset) + 30 <= UInt64(start) else {
                throw SuiteUpdateError.archive("The update ZIP entry points outside its data area.")
            }
            try file.seek(toOffset: UInt64(localOffset))
            let header = try file.read(upToCount: 30) ?? Data()
            guard header.count == 30, u32(header, 0) == 0x04034b50,
                  u16(header, 6) == u16(directory, offset + 8), u16(header, 8) == u16(directory, offset + 10),
                  UInt64(localOffset) + 30 + UInt64(u16(header, 26)) + UInt64(u16(header, 28)) + UInt64(u32(directory, offset + 20)) <= UInt64(start) else {
                throw SuiteUpdateError.archive("The update ZIP entry has an invalid local header.")
            }
            let localName = try file.read(upToCount: Int(u16(header, 26))) ?? Data()
            guard localName == directory.subdata(in: offset + 46..<offset + 46 + nameLength) else {
                throw SuiteUpdateError.archive("The update ZIP contains conflicting file paths.")
            }
            offset = next
        }
        guard offset == directory.count, count > 0 else { throw SuiteUpdateError.archive("The update ZIP directory is invalid or empty.") }
    }

    private static func run(_ command: String, _ arguments: [String]) throws {
        let process = Process()
        let output = Pipe()
        process.executableURL = URL(fileURLWithPath: command)
        process.arguments = arguments
        process.standardOutput = output
        process.standardError = output
        try process.run()
        let data = output.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            let detail = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            throw SuiteUpdateError.verification("The downloaded suite could not be verified. \(detail)")
        }
    }
}
