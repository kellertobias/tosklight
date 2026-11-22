import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { after, before, describe, test } from "node:test";
import { artifactPaths, repositoryRoot } from "./artifact-paths.mjs";

describe("macOS launcher update client", { skip: process.platform !== "darwin" }, () => {
	let root;
	let harness;
	const identifiers = {
		control: "de.tokenet.light",
		architect: "de.tokenet.tosklight.visualizer",
		pixel: "de.tokenet.tosklight.media",
		launcher: "de.tokenet.tosklight.launcher",
	};
	function run(command, args, options = {}) {
		return spawnSync(command, args, { encoding: "utf8", timeout: 120_000, ...options });
	}
	function checked(command, args, options) {
		const result = run(command, args, options);
		assert.equal(result.status, 0, result.stderr || result.stdout);
		return result;
	}
	before(() => {
		fs.mkdirSync(artifactPaths.tmp, { recursive: true });
		root = fs.mkdtempSync(path.join(artifactPaths.tmp, "launcher-update-client-"));
		harness = path.join(root, "client-tests");
		const source = path.join(root, "main.swift");
		fs.writeFileSync(source, `import Foundation
final class MockProtocol: URLProtocol {
    static var payload = Data()
    static var status = 200
    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        let response = HTTPURLResponse(url: request.url!, statusCode: Self.status, httpVersion: "HTTP/1.1", headerFields: nil)!
        client!.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client!.urlProtocol(self, didLoad: Self.payload)
        client!.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
}
func printRelease(_ release: SuiteRelease) throws {
    let data = try JSONSerialization.data(withJSONObject: ["version":release.version,"tag":release.tag,"notesURL":release.notesURL.absoluteString,"assetURL":release.assetURL.absoluteString,"sha256":release.sha256,"size":release.size], options:[.sortedKeys])
    FileHandle.standardOutput.write(data)
}
do {
    let args = CommandLine.arguments
    switch args[1] {
    case "parse": try printRelease(SuiteRelease.parse(data: Data(contentsOf: URL(fileURLWithPath:args[2]))))
    case "zip": try SuiteUpdateClient.validateArchive(URL(fileURLWithPath:args[2])); print("safe")
    case "prepare":
        let release = try SuiteRelease.parse(data: Data(contentsOf: URL(fileURLWithPath:args[2])))
        let update = try SuiteUpdateClient.prepareArchive(URL(fileURLWithPath:args[3]), release:release, directory:URL(fileURLWithPath:args[4]))
        let result = update.applications.mapValues { $0.path }
        FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject:result,options:[.sortedKeys]))
    case "sign":
        let records = try JSONSerialization.jsonObject(with:Data(contentsOf:URL(fileURLWithPath:args[2]))) as! [String:String]
        let prepared = PreparedSuiteUpdate(directory:URL(fileURLWithPath:args[3]), applications:records.mapValues { URL(fileURLWithPath:$0) })
        try SuiteUpdateClient.applyLocalSigning(prepared, helperURL:URL(fileURLWithPath:args[4]), shouldCancel:{ args.count > 5 && args[5] == "cancel" })
        print("signed")
    case "check":
        MockProtocol.payload = try Data(contentsOf: URL(fileURLWithPath:args[2]))
        if args.count > 3 { MockProtocol.status = Int(args[3])! }
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [MockProtocol.self]
        let client = SuiteUpdateClient(configuration:configuration)
        client.check { result in
            guard Thread.isMainThread else { exit(7) }
            do { try printRelease(result.get()); exit(0) }
            catch { FileHandle.standardError.write(Data(error.localizedDescription.utf8)); exit(1) }
        }
        withExtendedLifetime(client) { RunLoop.main.run() }
    default: exit(3)
    }
} catch { FileHandle.standardError.write(Data(error.localizedDescription.utf8)); exit(1) }
`);
		checked("xcrun", ["swiftc", "-swift-version", "5", "-module-cache-path", path.join(artifactPaths.root, "cache/swift-modules"),
			path.join(repositoryRoot, "apps/macos-launcher/SuiteUpdates.swift"), source, "-o", harness]);
	});
	after(() => { if (root) fs.rmSync(root, { recursive: true, force: true }); });
	function release(overrides = {}) {
		return { tag_name: "v1.8.0", html_url: "https://github.com/kellertobias/tosklight/releases/tag/v1.8.0",
			draft: false, prerelease: false, assets: [{ name: "tosklight-bundle-macos_arm64.zip", size: 123,
				browser_download_url: "https://github.com/kellertobias/tosklight/releases/download/v1.8.0/tosklight-bundle-macos_arm64.zip",
				digest: `sha256:${"a".repeat(64)}` }], ...overrides };
	}
	function metadata(data) {
		const file = path.join(root, `release-${Math.random()}.json`);
		fs.writeFileSync(file, JSON.stringify(data));
		return file;
	}
	function fixtureArchive(label, { omit, wrongVersion } = {}) {
		const directory = path.join(root, label);
		const suite = path.join(directory, "Downloaded Suite");
		fs.mkdirSync(suite, { recursive: true });
		for (const [product, identifier] of Object.entries(identifiers)) {
			if (product === omit) continue;
			const app = path.join(suite, `Renamed ${product}.app`);
			fs.mkdirSync(path.join(app, "Contents/MacOS"), { recursive: true });
			fs.copyFileSync("/usr/bin/true", path.join(app, "Contents/MacOS/tool"));
			fs.chmodSync(path.join(app, "Contents/MacOS/tool"), 0o755);
			const version = product === wrongVersion ? "1.7.0" : "1.8.0";
			fs.writeFileSync(path.join(app, "Contents/Info.plist"), `<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>${identifier}</string><key>CFBundleExecutable</key><string>tool</string><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleShortVersionString</key><string>${version}</string></dict></plist>`);
			checked("codesign", ["--force", "--sign", "-", "--timestamp=none", app]);
		}
		const archive = path.join(directory, "suite.zip");
		checked("ditto", ["-c", "-k", "--sequesterRsrc", "--keepParent", suite, archive]);
		return archive;
	}
	function archiveMetadata(archive) {
		const data = release();
		const bytes = fs.readFileSync(archive);
		data.assets[0].digest = `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
		data.assets[0].size = bytes.length;
		return data;
	}
	test("accepts a stable release with exact repository URLs and a pinned digest", () => {
		const data = release();
		const parsed = JSON.parse(checked(harness, ["parse", metadata(data)]).stdout);
		assert.equal(parsed.version, "1.8.0");
		assert.equal(parsed.tag, "v1.8.0");
		assert.equal(parsed.notesURL, data.html_url);
		assert.equal(parsed.assetURL, data.assets[0].browser_download_url);
		assert.equal(parsed.sha256, "a".repeat(64));
		assert.equal(parsed.size, 123);
	});
	test("rejects unpublished, malformed, unpinned and redirected release declarations", () => {
		const candidates = [release({ draft: true }), release({ prerelease: true }), release({ tag_name: "v1.8.0-beta.1" }), release({ tag_name: "v01.8.0" }), release({ html_url: "https://example.com/releases/v1.8.0" })];
		for (const [key, value] of [["digest", null], ["digest", "sha256:bad"], ["browser_download_url", "https://github.com/another/project/releases/download/v1.8.0/tosklight-bundle-macos_arm64.zip"], ["browser_download_url", "https://github.com.evil.example/kellertobias/tosklight/releases/download/v1.8.0/tosklight-bundle-macos_arm64.zip"], ["browser_download_url", "https://github.com/kellertobias/tosklight/releases/download/v1.8.0/tosklight-bundle-macos_arm64.zip?redirect=evil"]]) {
			const candidate = release(); candidate.assets[0][key] = value; candidates.push(candidate);
		}
		for (const candidate of candidates) assert.notEqual(run(harness, ["parse", metadata(candidate)]).status, 0, JSON.stringify(candidate));
	});
	test("delivers successful checks and HTTP failures to the main queue", () => {
		const file = metadata(release());
		assert.equal(JSON.parse(checked(harness, ["check", file]).stdout).version, "1.8.0");
		const failure = run(harness, ["check", file, "403"]);
		assert.equal(failure.status, 1);
		assert.match(failure.stderr, /HTTP 403/);
	});
	test("inspects real ZIP paths and rejects traversal, links and special files before extraction", () => {
		const valid = fixtureArchive("zip-validation");
		assert.match(checked(harness, ["zip", valid]).stdout, /safe/);
		for (const [name, mode] of [["../outside", 0o100644], ["/absolute", 0o100644], ["C:/outside", 0o100644], ["Suite\\outside", 0o100644], ["Suite/link", 0o120777], ["Suite/pipe", 0o010644]]) {
			const archive = path.join(root, `bad-${Math.random()}.zip`);
			checked("python3", ["-c", "import sys,zipfile; z=zipfile.ZipFile(sys.argv[1],'w'); i=zipfile.ZipInfo(sys.argv[2]); i.create_system=3; i.external_attr=int(sys.argv[3])<<16; z.writestr(i,'payload'); z.close()", archive, name, String(mode)]);
			assert.notEqual(run(harness, ["zip", archive]).status, 0, name);
		}
		const conflict = path.join(root, "conflicting-paths.zip");
		checked("python3", ["-c", "import sys,zipfile; p=sys.argv[1]; z=zipfile.ZipFile(p,'w'); z.writestr('safe-name','payload'); z.close(); d=bytearray(open(p,'rb').read()); d[30:39]=b'../escape'; open(p,'wb').write(d)", conflict]);
		assert.match(run(harness, ["zip", conflict]).stderr, /conflicting file paths/);
	});
	test("prepares four signed matching apps and rejects altered downloads or incomplete suites", () => {
		const archive = fixtureArchive("complete");
		const data = archiveMetadata(archive);
		const directory = path.join(root, "prepared"); fs.mkdirSync(directory);
		const prepared = JSON.parse(checked(harness, ["prepare", metadata(data), archive, directory]).stdout);
		assert.deepEqual(Object.keys(prepared).sort(), ["architect", "control", "launcher", "pixel"]);
		assert.ok(Object.values(prepared).every((file) => fs.existsSync(file)));
		const altered = structuredClone(data); altered.assets[0].digest = `sha256:${"0".repeat(64)}`;
		const wrongDigest = run(harness, ["prepare", metadata(altered), archive, path.join(root, "not-extracted")]);
		assert.notEqual(wrongDigest.status, 0);
		assert.match(wrongDigest.stderr, /SHA-256/);
		assert.equal(fs.existsSync(path.join(root, "not-extracted/extracted")), false);
		for (const [label, options, expected] of [["missing-launcher", { omit: "launcher" }, /does not contain/], ["wrong-version", { wrongVersion: "pixel" }, /unexpected version/]]) {
			const zip = fixtureArchive(label, options);
			const result = run(harness, ["prepare", metadata(archiveMetadata(zip)), zip, path.join(root, `${label}-prepared`)]);
			assert.notEqual(result.status, 0);
			assert.match(result.stderr, expected);
		}
	});
	test("locally signs verified apps with the current helper and keeps quarantine intact", () => {
		const archive = fixtureArchive("signing");
		const directory = path.join(root, "signing-prepared");
		const prepared = JSON.parse(checked(harness, ["prepare", metadata(archiveMetadata(archive)), archive, directory]).stdout);
		const records = metadata(prepared);
		const helper = path.join(repositoryRoot, "tools/sign-macos-apps-locally.sh");
		const quarantine = "0081;65000000;ToskLightUpdateClientTest;";
		for (const app of Object.values(prepared)) checked("xattr", ["-w", "com.apple.quarantine", quarantine, app]);
		// An archive-provided script must not participate in update signing.
		fs.writeFileSync(path.join(directory, "extracted/Downloaded Suite/sign-macos-apps-locally.sh"), "#!/bin/bash\nexit 44\n");
		assert.match(checked(harness, ["sign", records, directory, helper]).stdout, /signed/);
		for (const app of Object.values(prepared)) {
			checked("codesign", ["--verify", "--deep", "--strict", app]);
			assert.match(checked("codesign", ["--display", "--verbose=4", app]).stderr, /Signature=adhoc/);
			assert.equal(checked("xattr", ["-p", "com.apple.quarantine", app]).stdout.trim(), quarantine);
		}
		const missing = run(harness, ["sign", records, directory, path.join(root, "missing-helper.sh")]);
		assert.equal(missing.status, 1);
		assert.match(missing.stderr, /missing its local-signing helper.*Reinstall/);
		const canceled = run(harness, ["sign", records, directory, helper, "cancel"]);
		assert.equal(canceled.status, 1);
		assert.match(canceled.stderr, /NSURLErrorDomain.*-999/);
	});
	test("rejects a changed original signature before running any signing helper", () => {
		const archive = fixtureArchive("signing-tampered");
		const directory = path.join(root, "tampered-prepared");
		const prepared = JSON.parse(checked(harness, ["prepare", metadata(archiveMetadata(archive)), archive, directory]).stdout);
		fs.appendFileSync(path.join(prepared.launcher, "Contents/Info.plist"), "\n<!-- changed after verification -->\n");
		const helper = path.join(root, "record-signing.sh");
		const called = `${helper}.called`;
		fs.writeFileSync(helper, '#!/bin/bash\ntouch "$0.called"\n');
		const result = run(harness, ["sign", metadata(prepared), directory, helper]);
		assert.equal(result.status, 1);
		assert.equal(fs.existsSync(called), false);
	});
});
