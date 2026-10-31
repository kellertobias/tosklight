import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { artifactPaths } from "./artifact-paths.mjs";

const mac = { skip: process.platform !== "darwin" };
const scripts = Object.fromEntries(
	["macos-signing-credentials", "seal-macos-app", "notarize-macos-release"].map(
		(name) => [name, path.resolve(`tools/${name}.sh`)],
	),
);
const credentialNames = [
	"APPLE_CERTIFICATE_BASE64",
	"APPLE_CERTIFICATE_PASSWORD",
	"APPLE_SIGNING_IDENTITY",
	"APPLE_ID",
	"APPLE_TEAM_ID",
	"APPLE_APP_SPECIFIC_PASSWORD",
];
function run(command, args, env) {
	return spawnSync(command, args, { encoding: "utf8", env });
}
function checked(command, args, env) {
	const result = run(command, args, env);
	assert.equal(result.status, 0, result.stderr || result.stdout);
	return result;
}
function fixture(t) {
	fs.mkdirSync(artifactPaths.tmp, { recursive: true });
	const root = fs.mkdtempSync(
		path.join(artifactPaths.tmp, "macos-release-signing-"),
	);
	t.after(() => fs.rmSync(root, { recursive: true, force: true }));
	const bin = path.join(root, "bin");
	fs.mkdirSync(bin);
	const env = {
		...process.env,
		PATH: `${bin}:${process.env.PATH}`,
		LIGHT_TMP_DIR: path.join(root, "tmp"),
		GITHUB_ENV: path.join(root, "github-env"),
		MOCK_COMMAND_LOG: path.join(root, "commands"),
	};
	for (const name of [
		...credentialNames,
		"LIGHT_MACOS_SIGNING_MODE",
		"LIGHT_MACOS_SIGNING_IDENTITY",
		"LIGHT_MACOS_KEYCHAIN",
	])
		delete env[name];
	fs.writeFileSync(env.MOCK_COMMAND_LOG, "");
	const mock = (name, body) =>
		fs.writeFileSync(
			path.join(bin, name),
			`#!/bin/bash\nset -eu\nprintf '%s %s\\n' '${name}' "$*" >> "$MOCK_COMMAND_LOG"\n${body}\n`,
			{ mode: 0o755 },
		);
	return {
		root,
		env,
		mock,
		log: () => fs.readFileSync(env.MOCK_COMMAND_LOG, "utf8"),
	};
}
function appFixture(root, name = "ToskLight.app") {
	const app = path.join(root, name);
	fs.mkdirSync(path.join(app, "Contents/MacOS"), { recursive: true });
	fs.mkdirSync(path.join(app, "Contents/Resources/helpers"), {
		recursive: true,
	});
	fs.writeFileSync(
		path.join(app, "Contents/Info.plist"),
		'<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>de.tokenet.light</string><key>CFBundleExecutable</key><string>main</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>',
	);
	const source = path.join(root, "fixture.c");
	fs.writeFileSync(source, "int main(void) { return 0; }\n");
	checked("/usr/bin/clang", [
		source,
		"-o",
		path.join(app, "Contents/MacOS/main"),
	]);
	fs.copyFileSync(
		path.join(app, "Contents/MacOS/main"),
		path.join(app, "Contents/Resources/helpers/renderer"),
	);
	return app;
}
function credentials(env) {
	return {
		...env,
		APPLE_CERTIFICATE_BASE64: Buffer.from("fake fixture certificate").toString(
			"base64",
		),
		APPLE_CERTIFICATE_PASSWORD: "CERTIFICATE_PASSWORD_SENTINEL",
		APPLE_SIGNING_IDENTITY: "Developer ID Application: Fixture (TESTTEAM)",
		APPLE_ID: "fixture@example.invalid",
		APPLE_TEAM_ID: "TESTTEAM",
		APPLE_APP_SPECIFIC_PASSWORD: "NOTARY_PASSWORD_SENTINEL",
	};
}
function assertSecretsAbsent(result, env) {
	for (const name of [
		"APPLE_CERTIFICATE_BASE64",
		"APPLE_CERTIFICATE_PASSWORD",
		"APPLE_APP_SPECIFIC_PASSWORD",
	])
		assert.ok(
			!(result.stdout + result.stderr).includes(env[name]),
			`${name} leaked in output`,
		);
}

test(
	"release sealing produces real valid ad-hoc signatures for app and nonstandard helper",
	mac,
	(t) => {
		const { root, env } = fixture(t);
		const app = appFixture(root);
		checked("bash", [scripts["seal-macos-app"], app], env);
		for (const item of [
			app,
			path.join(app, "Contents/MacOS/main"),
			path.join(app, "Contents/Resources/helpers/renderer"),
		]) {
			checked("/usr/bin/codesign", ["--verify", "--deep", "--strict", item]);
			assert.match(
				checked("/usr/bin/codesign", ["--display", "--verbose=4", item]).stderr,
				/Signature=adhoc/,
			);
		}
	},
);

test(
	"Developer ID sealing signs helpers first with runtime, timestamp, entitlements and isolated keychain",
	mac,
	(t) => {
		const { root, env, mock, log } = fixture(t);
		const app = appFixture(root);
		mock("codesign", "exit 0");
		checked("bash", [scripts["seal-macos-app"], app], {
			...env,
			LIGHT_MACOS_SIGNING_IDENTITY:
				"Developer ID Application: Fixture (TESTTEAM)",
			LIGHT_MACOS_KEYCHAIN: path.join(root, "release.keychain-db"),
		});
		const lines = log().trim().split("\n");
		const signs = lines.filter((line) => line.includes("--force"));
		assert.equal(signs.length, 3);
		for (const line of signs) {
			assert.match(
				line,
				/--sign Developer ID Application: Fixture \(TESTTEAM\)/,
			);
			assert.match(
				line,
				/--timestamp --options runtime --entitlements .*macos-audio\.entitlements\.plist/,
			);
			assert.match(line, /--keychain .*release\.keychain-db/);
			assert.doesNotMatch(line, /--deep|timestamp=none/);
		}
		assert.equal(signs.at(-1).endsWith(app), true);
		assert.match(lines.at(-1), /--verify --deep --strict/);
		fs.writeFileSync(env.MOCK_COMMAND_LOG, "");
		assert.notEqual(
			run("bash", [scripts["seal-macos-app"], app], {
				...env,
				LIGHT_MACOS_SIGNING_IDENTITY: "Apple Development: Fixture",
			}).status,
			0,
		);
		assert.equal(log(), "", "invalid identity cannot invoke signing");
	},
);

test(
	"credentials absent select ad-hoc; partial credentials fail before keychain import",
	mac,
	(t) => {
		const { env, mock, log } = fixture(t);
		mock("security", "exit 88");
		checked("bash", [scripts["macos-signing-credentials"], "setup"], env);
		assert.equal(
			fs.readFileSync(env.GITHUB_ENV, "utf8"),
			"LIGHT_MACOS_SIGNING_MODE=adhoc\n",
		);
		assert.equal(log(), "");
		fs.unlinkSync(env.GITHUB_ENV);
		const result = run(
			"bash",
			[scripts["macos-signing-credentials"], "setup"],
			{ ...env, APPLE_ID: "fixture@example.invalid" },
		);
		assert.notEqual(result.status, 0);
		assert.match(result.stderr, /incomplete Apple release credentials/);
		assert.equal(log(), "");
		assert.equal(fs.existsSync(env.GITHUB_ENV), false);
	},
);

test(
	"complete mocked credentials import and clean an isolated keychain without printing secrets",
	mac,
	(t) => {
		const { root, env, mock, log } = fixture(t);
		mock(
			"security",
			// biome-ignore lint/suspicious/noTemplateCurlyInString: These are literal Bash expansions in the mocked command.
			'case "$1" in\ncreate-keychain) touch "${!#}" ;;\ndelete-keychain) rm -f -- "$2" ;;\nfind-identity) printf \'1) FIXTURE "%s"\\n\' "$APPLE_SIGNING_IDENTITY" ;;\nesac',
		);
		const configured = credentials(env);
		const setup = checked(
			"bash",
			[scripts["macos-signing-credentials"], "setup"],
			configured,
		);
		assertSecretsAbsent(setup, configured);
		const exported = Object.fromEntries(
			fs
				.readFileSync(env.GITHUB_ENV, "utf8")
				.trim()
				.split("\n")
				.map((line) => {
					const index = line.indexOf("=");
					return [line.slice(0, index), line.slice(index + 1)];
				}),
		);
		assert.equal(exported.LIGHT_MACOS_SIGNING_MODE, "developer-id");
		assert.equal(
			exported.LIGHT_MACOS_SIGNING_IDENTITY,
			configured.APPLE_SIGNING_IDENTITY,
		);
		assert.ok(
			exported.LIGHT_MACOS_KEYCHAIN.startsWith(
				path.join(root, "tmp/apple-signing."),
			),
		);
		assert.equal(fs.existsSync(exported.LIGHT_MACOS_KEYCHAIN), true);
		assert.equal(
			fs.existsSync(
				path.join(
					path.dirname(exported.LIGHT_MACOS_KEYCHAIN),
					"certificate.p12",
				),
			),
			false,
		);
		assert.match(
			log(),
			/security import .* -k .* -P CERTIFICATE_PASSWORD_SENTINEL -T \/usr\/bin\/codesign/,
		);
		assert.match(
			log(),
			/security set-key-partition-list -S apple-tool:,apple:,codesign:/,
		);
		const cleanup = checked(
			"bash",
			[scripts["macos-signing-credentials"], "cleanup"],
			{ ...configured, ...exported },
		);
		assertSecretsAbsent(cleanup, configured);
		assert.equal(
			fs.existsSync(path.dirname(exported.LIGHT_MACOS_KEYCHAIN)),
			false,
		);
	},
);

function notarizationFixture(t) {
	const setup = fixture(t);
	const { root, env, mock } = setup;
	const directory = path.join(root, "release");
	fs.mkdirSync(directory);
	const app = appFixture(root);
	const archives = [
		"tosklight-1.2.3-macos-arm64.zip",
		"tosklight-architect-1.2.3-macos-arm64.zip",
		"tosklight-viz-1.2.3-macos-arm64.zip",
		"tosklight-media-1.2.3-macos-arm64.zip",
		"tosklight-bundle-macos_arm64.zip",
	];
	const bundle = path.join(root, "tosklight-bundle-macos_arm64");
	fs.mkdirSync(bundle);
	const combinedNames = [
		"tosklight-desk-macos_arm64.app",
		"tosklight-architect-macos_arm64.app",
		"tosklight-media-macos_arm64.app",
	];
	for (const name of combinedNames)
		fs.cpSync(app, path.join(bundle, name), { recursive: true });
	const appRoots = Object.fromEntries(
		archives.map((archive) => [
			archive,
			archive.includes("bundle-")
				? combinedNames.map((name) => `tosklight-bundle-macos_arm64/${name}`)
				: ["ToskLight.app"],
		]),
	);
	for (const archive of archives)
		checked("/usr/bin/ditto", [
			"-c",
			"-k",
			"--sequesterRsrc",
			"--keepParent",
			archive.includes("bundle-") ? bundle : app,
			path.join(directory, archive),
		]);
	const headless = path.join(directory, "light-headless-1.2.3-macos-arm64.zip");
	const data = path.join(root, "headless");
	fs.mkdirSync(data);
	fs.writeFileSync(path.join(data, "headless-fixture"), "unchanged companion");
	checked("/usr/bin/ditto", ["-c", "-k", "--keepParent", data, headless]);
	mock(
		"xcrun",
		// biome-ignore lint/suspicious/noTemplateCurlyInString: These are literal Bash expansions in the mocked command.
		'case "$1 $2" in\n"notarytool submit") printf \'{"id":"fixture-submission","status":"%s"}\\n\' "${MOCK_NOTARY_STATUS:-Accepted}" ;;\n"notarytool log") printf \'{"fixture":"rejected"}\\n\' > "${!#}" ;;\n"stapler staple") touch "$3/Contents/Resources/mock-notary-ticket" ;;\n"stapler validate") test -f "$3/Contents/Resources/mock-notary-ticket" ;;\n*) exit 44 ;;\nesac',
	);
	mock("codesign", "exit 0");
	mock("spctl", "exit 0");
	return {
		...setup,
		directory,
		archives,
		appRoots,
		headless,
		env: { ...credentials(env), LIGHT_MACOS_SIGNING_MODE: "developer-id" },
	};
}

test(
	"accepted notarization staples and repacks every app archive, retaining companion-only archive",
	mac,
	(t) => {
		const { root, directory, archives, appRoots, headless, env, log } =
			notarizationFixture(t);
		const beforeHeadless = fs.readFileSync(headless);
		const result = checked(
			"bash",
			[scripts["notarize-macos-release"], directory],
			env,
		);
		assertSecretsAbsent(result, env);
		for (const [index, archive] of archives.entries()) {
			const unpacked = path.join(root, `inspection-${index}`);
			checked("/usr/bin/ditto", [
				"-x",
				"-k",
				path.join(directory, archive),
				unpacked,
			]);
			for (const relative of appRoots[archive])
				assert.equal(
					fs.existsSync(
						path.join(
							unpacked,
							relative,
							"Contents/Resources/mock-notary-ticket",
						),
					),
					true,
					`${archive}: ${relative}`,
				);
		}
		assert.deepEqual(fs.readFileSync(headless), beforeHeadless);
		assert.equal(
			log()
				.split("\n")
				.filter((line) => line.startsWith("xcrun stapler staple ")).length,
			Object.values(appRoots).flat().length,
		);
		assert.equal(
			log()
				.split("\n")
				.filter((line) => line.startsWith("spctl --assess ")).length,
			Object.values(appRoots).flat().length,
		);
		assert.equal(
			JSON.parse(
				fs.readFileSync(
					path.join(directory, "macos-notarization.json"),
					"utf8",
				),
			).status,
			"Accepted",
		);
	},
);

test(
	"rejected notarization fails closed and preserves every original archive",
	mac,
	(t) => {
		const { directory, archives, headless, env, log } = notarizationFixture(t);
		const before = new Map(
			[...archives.map((name) => path.join(directory, name)), headless].map(
				(name) => [name, fs.readFileSync(name)],
			),
		);
		const result = run("bash", [scripts["notarize-macos-release"], directory], {
			...env,
			MOCK_NOTARY_STATUS: "Invalid",
		});
		assert.notEqual(result.status, 0);
		assertSecretsAbsent(result, env);
		assert.match(result.stderr, /notarization was not accepted/);
		for (const [name, bytes] of before)
			assert.deepEqual(fs.readFileSync(name), bytes);
		assert.doesNotMatch(log(), /stapler|spctl/);
		assert.equal(
			fs.existsSync(path.join(directory, "macos-notarization-log.json")),
			true,
		);
	},
);

test(
	"failed certificate import removes temporary certificate and keychain without enabling signing",
	mac,
	(t) => {
		const { root, env, mock, log } = fixture(t);
		mock(
			"security",
			// biome-ignore lint/suspicious/noTemplateCurlyInString: These are literal Bash expansions in the mocked command.
			'case "$1" in\ncreate-keychain) touch "${!#}" ;;\nimport) exit 17 ;;\ndelete-keychain) rm -f -- "$2" ;;\nesac',
		);
		const configured = credentials(env);
		const result = run(
			"bash",
			[scripts["macos-signing-credentials"], "setup"],
			configured,
		);
		assert.notEqual(result.status, 0);
		assertSecretsAbsent(result, configured);
		assert.equal(fs.existsSync(env.GITHUB_ENV), false);
		assert.deepEqual(fs.readdirSync(path.join(root, "tmp")), []);
		assert.match(log(), /security delete-keychain/);
	},
);

test(
	"ad-hoc release notarization exits without credentials or Apple tool invocation",
	mac,
	(t) => {
		const { root, env, mock, log } = fixture(t);
		mock("xcrun", "exit 99");
		const result = checked(
			"bash",
			[scripts["notarize-macos-release"], root],
			env,
		);
		assert.match(result.stdout, /notarization is not configured/);
		assert.equal(log(), "");
	},
);
