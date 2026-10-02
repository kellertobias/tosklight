import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import {
	SOURCE_MANIFEST_POLICY,
	collectSourceManifest,
	isSourceManifestPath,
	loadSuppliedSourceManifest,
	ownGitCheckout,
	sourceManifestErrors,
} from "./semantic-source-manifest.mjs";

const { artifactPaths } = await import("./artifact-paths.mjs");

// Isolated from the developer's global Git configuration (hooks, signing, templates).
const gitEnv = { ...process.env, GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_NOSYSTEM: "1" };
function git(cwd, ...args) {
	const result = spawnSync("git", ["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false", ...args], { cwd, env: gitEnv, encoding: "utf8" });
	assert.equal(result.status, 0, `git ${args.join(" ")}: ${result.stderr}`);
	return result.stdout.trim();
}

function write(root, file, content) {
	fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
	fs.writeFileSync(path.join(root, file), content);
}

async function withFixture(name, body) {
	const directory = fs.mkdtempSync(path.join(artifactPaths.tmp, `semantic-source-${name}-`));
	try {
		return await body(directory);
	} finally {
		fs.chmodSync(directory, 0o755);
		fs.rmSync(directory, { recursive: true, force: true });
	}
}

function gitRepository(root) {
	git(root, "init", "-q", "-b", "main");
	write(root, "Cargo.toml", "[workspace]\n");
	write(root, ".gitignore", "/ignored-output/\n*.local\n");
	write(root, "tools/a.mjs", "export const a = 1;\n");
	write(root, "crates/x/src/lib.rs", "pub fn x() {}\n");
	write(root, "docs/notes.md", "not implementation source\n");
	git(root, "add", "-A");
	git(root, "commit", "-q", "-m", "fixture");
	return git(root, "rev-parse", "HEAD");
}

const byPath = (manifest) => Object.fromEntries(manifest.entries.map((entry) => [entry.path, entry]));

test("policy keeps implementation sources and drops artifacts, dependencies, Git internals and secrets", () => {
	for (const included of ["Cargo.toml", "package.json", "tools/a.mjs", "apps/light-desktop/src/x.tsx", "crates/a/src/lib.rs", "tests/a.spec.ts", "assets/fixture-library/x.toskfixture", ".cargo/config.toml", ".env.example"])
		assert.ok(isSourceManifestPath(included), included);
	for (const excluded of [
		".artifacts/tmp/x.mjs",
		"apps/x/node_modules/y/index.js",
		".git/HEAD",
		"crates/target/debug/x",
		"apps/x/dist/index.js",
		"tools/.env",
		".env.production",
		"tools/server.pem",
		"tools/id_ed25519",
		"tests/run.log",
		"apps/x/tsconfig.tsbuildinfo",
		"assets/media/clip.mp4",
		"docs/help/x.md",
		"../escape.mjs",
		"/absolute.mjs",
	])
		assert.ok(!isSourceManifestPath(excluded), excluded);
});

test("tracked edits change identity at an equal HEAD; contents are never stored", async () => {
	await withFixture("tracked", (root) => {
		const head = gitRepository(root);
		const clean = collectSourceManifest({ root });
		assert.equal(clean.mode, "git");
		assert.equal(clean.gitHead, head);
		assert.equal(clean.deletions, "recorded-against-head-and-index");
		assert.deepEqual(Object.keys(byPath(clean)), [".gitignore", "Cargo.toml", "crates/x/src/lib.rs", "tools/a.mjs"]);
		assert.equal(byPath(clean)["tools/a.mjs"].state, "tracked");

		write(root, "tools/a.mjs", "export const a = 2;\n");
		const editA = collectSourceManifest({ root });
		write(root, "tools/a.mjs", "export const a = 3;\n");
		const editB = collectSourceManifest({ root });
		assert.equal(git(root, "rev-parse", "HEAD"), head);
		assert.notEqual(editA.sourceSha256, clean.sourceSha256);
		assert.notEqual(editA.sourceSha256, editB.sourceSha256, "same HEAD, same dirty flag, different bytes");
		assert.ok(!JSON.stringify(editB).includes("export const a"), "manifests store digests, not contents");
	});
});

test("untracked additions are recorded; ignored output and secrets are not", async () => {
	await withFixture("untracked", (root) => {
		gitRepository(root);
		const before = collectSourceManifest({ root });
		write(root, "tools/new-helper.mjs", "export {};\n");
		write(root, "ignored-output/tools/x.mjs", "ignored\n");
		write(root, "tools/cache.local", "ignored\n");
		write(root, "tools/.env", "TOKEN=secret\n");
		write(root, "tools/node_modules/dep/index.js", "dependency\n");
		write(root, ".artifacts/tmp/x.mjs", "artifact\n");
		const after = collectSourceManifest({ root });
		assert.notEqual(after.sourceSha256, before.sourceSha256);
		assert.equal(byPath(after)["tools/new-helper.mjs"].state, "untracked");
		assert.equal(after.counts.untracked, 1);
		for (const excluded of ["ignored-output/tools/x.mjs", "tools/cache.local", "tools/.env", "tools/node_modules/dep/index.js", ".artifacts/tmp/x.mjs"])
			assert.equal(byPath(after)[excluded], undefined, excluded);
		assert.ok(!JSON.stringify(after).includes("secret"));
	});
});

test("worktree and staged deletions are distinguished from absent files", async () => {
	await withFixture("deleted", (root) => {
		gitRepository(root);
		fs.rmSync(path.join(root, "tools/a.mjs"));
		git(root, "rm", "-q", "crates/x/src/lib.rs");
		const manifest = collectSourceManifest({ root });
		assert.deepEqual(byPath(manifest)["tools/a.mjs"], { path: "tools/a.mjs", state: "deleted" });
		assert.deepEqual(byPath(manifest)["crates/x/src/lib.rs"], { path: "crates/x/src/lib.rs", state: "deleted" });
		assert.equal(manifest.counts.deleted, 2);
		assert.deepEqual(sourceManifestErrors(manifest), []);
	});
});

test("ordering is stable regardless of creation order and is byte-sorted", async () => {
	await withFixture("order", (root) => {
		const files = ["tools/b.mjs", "tools/A.mjs", "crates/z/lib.rs", "tools/a/b.mjs", "tools/a-b.mjs", "package.json"];
		for (const [name, order] of [["one", files], ["two", [...files].reverse()]]) {
			fs.mkdirSync(path.join(root, name));
			for (const file of order) write(path.join(root, name), file, `${file}\n`);
		}
		const one = collectSourceManifest({ root: path.join(root, "one") });
		const two = collectSourceManifest({ root: path.join(root, "two") });
		assert.equal(one.sourceSha256, two.sourceSha256);
		const paths = one.entries.map((entry) => entry.path);
		assert.deepEqual(paths, [...paths].sort());
		assert.deepEqual(paths, ["crates/z/lib.rs", "package.json", "tools/A.mjs", "tools/a-b.mjs", "tools/a/b.mjs", "tools/b.mjs"]);
		assert.equal(collectSourceManifest({ root: path.join(root, "one") }).sourceSha256, one.sourceSha256, "repeatable");
	});
});

test("a nested snapshot without its own Git metadata never borrows the enclosing HEAD", async () => {
	await withFixture("nested", (root) => {
		const outerHead = gitRepository(root);
		const snapshot = path.join(root, "tools", "snapshot");
		write(snapshot, "tools/a.mjs", "export const a = 1;\n");
		write(snapshot, "Cargo.toml", "[workspace]\n");
		assert.equal(ownGitCheckout(snapshot).owns, false);
		const manifest = collectSourceManifest({ root: snapshot });
		assert.equal(manifest.mode, "filesystem");
		assert.equal(manifest.gitHead.status, "unavailable");
		assert.match(manifest.gitHead.reason, /enclosing git checkout/u);
		assert.equal(manifest.deletions.status, "unavailable");
		assert.ok(!JSON.stringify(manifest).includes(outerHead));
		assert.deepEqual(manifest.entries.map((entry) => [entry.path, entry.state]), [["Cargo.toml", "present"], ["tools/a.mjs", "present"]]);
		write(snapshot, "tools/a.mjs", "export const a = 2;\n");
		assert.notEqual(collectSourceManifest({ root: snapshot }).sourceSha256, manifest.sourceSha256);
		// The outer checkout lists the nested files as its own untracked sources; the snapshot does not.
		assert.equal(byPath(collectSourceManifest({ root }))["tools/snapshot/tools/a.mjs"].state, "untracked");
		// A nested directory with its own repository is its own checkout.
		git(snapshot, "init", "-q", "-b", "main");
		assert.equal(ownGitCheckout(snapshot).owns, true);
		const own = collectSourceManifest({ root: snapshot });
		assert.equal(own.mode, "git");
		assert.equal(own.gitHead.status, "unavailable", "no commit yet");
		assert.equal(own.deletions, "recorded-against-index");
		assert.equal(own.counts.untracked, 2);
	});
});

test("missing and unreadable evidence stays explicitly unavailable", async () => {
	await withFixture("unavailable", (root) => {
		assert.deepEqual(collectSourceManifest({ root: path.join(root, "missing") }).status, "unavailable");
		write(root, "plain-file", "x");
		assert.equal(collectSourceManifest({ root: path.join(root, "plain-file") }).status, "unavailable");
		const snapshot = path.join(root, "snapshot");
		write(snapshot, "tools/a.mjs", "a\n");
		write(snapshot, "tools/secret-ish.mjs", "b\n");
		const readable = collectSourceManifest({ root: snapshot });
		assert.equal(readable.complete, true);
		if (process.getuid?.() === 0) return; // root reads mode-000 files; nothing further to prove
		fs.chmodSync(path.join(snapshot, "tools/secret-ish.mjs"), 0o000);
		try {
			const manifest = collectSourceManifest({ root: snapshot });
			const entry = byPath(manifest)["tools/secret-ish.mjs"];
			assert.equal(entry.state, "unavailable");
			assert.match(entry.reason, /read failed/u);
			assert.equal(entry.sha256, undefined, "no digest is invented");
			assert.equal(manifest.complete, false);
			assert.equal(manifest.counts.unavailable, 1);
			assert.notEqual(manifest.sourceSha256, readable.sourceSha256);
			assert.deepEqual(sourceManifestErrors(manifest), []);
			fs.chmodSync(path.join(snapshot, "tools"), 0o000);
			const unlistable = collectSourceManifest({ root: snapshot });
			assert.deepEqual(unlistable.entries, [{ path: "tools/", state: "unavailable", reason: "directory unreadable: EACCES" }]);
		} finally {
			fs.chmodSync(path.join(snapshot, "tools"), 0o755);
			fs.chmodSync(path.join(snapshot, "tools/secret-ish.mjs"), 0o644);
		}
	});
});

test("a supplied snapshot manifest is accepted only with a valid digest", async () => {
	await withFixture("supplied", (root) => {
		write(root, "tools/a.mjs", "a\n");
		write(root, "tools/b.mjs", "b\n");
		const manifest = collectSourceManifest({ root });
		const file = path.join(root, "snapshot-manifest.json");
		fs.writeFileSync(file, JSON.stringify(manifest));
		assert.deepEqual(loadSuppliedSourceManifest(file), manifest);
		const clone = () => JSON.parse(JSON.stringify(manifest));
		const tampered = clone();
		tampered.entries[0].sha256 = "0".repeat(64);
		assert.throws(() => loadSuppliedSourceManifest(tampered), /digest does not match/u);
		const reordered = clone();
		reordered.entries.reverse();
		assert.throws(() => loadSuppliedSourceManifest(reordered), /sorted and unique/u);
		const withContent = clone();
		withContent.entries[0].content = "a\n";
		assert.throws(() => loadSuppliedSourceManifest(withContent), /must not carry file contents/u);
		const outside = clone();
		outside.entries[0].path = "node_modules/x.js";
		assert.throws(() => loadSuppliedSourceManifest(outside), /excluded by the source policy/u);
		const policy = clone();
		policy.policySha256 = "f".repeat(64);
		assert.throws(() => loadSuppliedSourceManifest(policy), /different source policy/u);
		assert.throws(() => loadSuppliedSourceManifest({ status: "unavailable", reason: "x" }), /not a recorded manifest/u);
		assert.ok(SOURCE_MANIFEST_POLICY.includeTrees.includes("tools"));
	});
});
