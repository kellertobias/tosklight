// Source identity for semantic performance evidence (TL-604). Git HEAD and a dirty flag do not
// identify a large uncommitted or untracked implementation: two materially different trees can
// share both. This module records a deterministic, sorted relative-path/content-hash manifest of
// the implementation sources, or validates an explicitly supplied immutable snapshot manifest.
//
// - Only paths, byte counts and SHA-256 digests are stored, never file contents.
// - Identity never comes from timestamps, inode numbers or the absolute checkout location.
// - Generated/runtime artifacts, dependencies, Git internals and likely secrets are excluded.
// - A checkout nested inside another repository without its own Git metadata (a snapshot under
//   `.artifacts`) is hashed from the filesystem; it never borrows the enclosing HEAD.
// - Deletions are recorded relative to the checkout's own HEAD/index. Without Git metadata there is
//   no base to compare against, so deletions are explicitly unavailable, not "none".
// - Unreadable files and missing roots stay explicitly unavailable.
// This is cold tooling: it walks and hashes the tree once per report, never per frame.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { canonicalJson, sha256 } from "./semantic-performance-contract.mjs";

export const SOURCE_MANIFEST_VERSION = "tosklight.semantic-source-manifest/1";

/**
 * What counts as implementation source. Regular files at the repository root are included; below
 * the root only these trees are. `assets/fixture-library` is included because workloads read it.
 * The policy itself is part of the digest, so changing it changes every source identity.
 */
export const SOURCE_MANIFEST_POLICY = Object.freeze({
	includeRootFiles: true,
	includeTrees: Object.freeze([".cargo", "apps", "assets/fixture-library", "crates", "tests", "tools"]),
	excludeSegments: Object.freeze([
		".artifacts",
		".git",
		".next",
		".pnpm-store",
		".show",
		".turbo",
		".vite",
		"__pycache__",
		"coverage",
		"dist",
		"light-data",
		"node_modules",
		"playwright-report",
		"storybook-static",
		"target",
		"test-results",
	]),
	// Runtime output, build caches and likely secrets, by basename.
	excludeBasenames: Object.freeze([
		"^\\.DS_Store$",
		"^\\.env(?:\\..*)?$",
		"^\\.npmrc$",
		"^id_(?:rsa|dsa|ecdsa|ed25519)(?:\\..*)?$",
		"\\.(?:key|pem|p12|pfx|keystore|jks)$",
		"\\.(?:log|pid|pyc|profraw|profdata|tsbuildinfo)$",
		"\\.(?:sqlite|show)(?:-shm|-wal)?$",
	]),
	keepBasenames: Object.freeze(["^\\.env\\.example$"]),
});

const excludedSegments = new Set(SOURCE_MANIFEST_POLICY.excludeSegments);
const excludedBasenames = SOURCE_MANIFEST_POLICY.excludeBasenames.map((pattern) => new RegExp(pattern, "u"));
const keptBasenames = SOURCE_MANIFEST_POLICY.keepBasenames.map((pattern) => new RegExp(pattern, "u"));
const ENTRY_STATES = new Set(["tracked", "untracked", "present", "deleted", "unavailable"]);
const SHA256 = /^[0-9a-f]{64}$/u;

function unavailable(reason) {
	return { status: "unavailable", reason };
}

/** True when a repository-relative POSIX path is implementation source under the policy. */
export function isSourceManifestPath(relativePath) {
	const segments = relativePath.split("/");
	if (segments.some((segment) => segment === "" || segment === "." || segment === ".." || excludedSegments.has(segment)))
		return false;
	const basename = segments.at(-1);
	if (!keptBasenames.some((pattern) => pattern.test(basename)) && excludedBasenames.some((pattern) => pattern.test(basename)))
		return false;
	if (segments.length === 1) return SOURCE_MANIFEST_POLICY.includeRootFiles;
	return SOURCE_MANIFEST_POLICY.includeTrees.some((tree) => relativePath.startsWith(`${tree}/`));
}

function git(args, cwd) {
	const result = spawnSync("git", args, { cwd, encoding: "buffer", timeout: 30_000, maxBuffer: 256 * 1024 * 1024 });
	return result.status === 0 ? result.stdout : null;
}

function nulList(buffer) {
	return buffer.toString("utf8").split("\0").filter(Boolean);
}

/**
 * Whether `root` is the top level of its own Git checkout. A directory nested in an enclosing
 * repository (for example a snapshot copied below `.artifacts`) is not, even though Git would
 * happily answer for the enclosing repository.
 */
export function ownGitCheckout(root) {
	const topLevel = git(["rev-parse", "--show-toplevel"], root);
	if (topLevel === null) return { owns: false, reason: "not inside a readable git checkout" };
	const top = topLevel.toString("utf8").trim();
	try {
		if (fs.realpathSync(top) === fs.realpathSync(root)) return { owns: true };
	} catch {
		return { owns: false, reason: "git top level could not be resolved" };
	}
	return { owns: false, reason: "nested inside an enclosing git checkout without git metadata of its own" };
}

/** Hash one path without following symlinks. Returns an entry body or an unavailable marker. */
function hashPath(absolute) {
	let stat;
	try {
		stat = fs.lstatSync(absolute);
	} catch (error) {
		if (error.code === "ENOENT") return null;
		return { state: "unavailable", reason: `stat failed: ${error.code ?? error.message}` };
	}
	try {
		if (stat.isSymbolicLink()) {
			const target = fs.readlinkSync(absolute);
			return { kind: "symlink", sha256: createHash("sha256").update(target).digest("hex"), bytes: Buffer.byteLength(target) };
		}
		if (!stat.isFile()) return { state: "unavailable", reason: "not a regular file or symlink" };
		const bytes = fs.readFileSync(absolute);
		return { kind: "file", sha256: createHash("sha256").update(bytes).digest("hex"), bytes: bytes.length };
	} catch (error) {
		return { state: "unavailable", reason: `read failed: ${error.code ?? error.message}` };
	}
}

function compareEntries(a, b) {
	return a.path < b.path ? -1 : a.path > b.path ? 1 : 0;
}

function entryFor(root, relativePath, presentState) {
	const hashed = hashPath(path.join(root, ...relativePath.split("/")));
	if (hashed === null) return { path: relativePath, state: "deleted" };
	if (hashed.state === "unavailable") return { path: relativePath, ...hashed };
	return { path: relativePath, state: presentState, ...hashed };
}

function gitEntries(root) {
	const head = git(["rev-parse", "--verify", "HEAD^{commit}"], root)?.toString("utf8").trim() ?? null;
	const index = git(["ls-files", "-z", "--cached"], root);
	const untracked = git(["ls-files", "-z", "--others", "--exclude-standard"], root);
	const headTree = head === null ? [] : git(["ls-tree", "-r", "-z", "--name-only", head], root);
	if (index === null || untracked === null || headTree === null) return null;
	const tracked = new Set([...nulList(index), ...(Array.isArray(headTree) ? headTree : nulList(headTree))]);
	const entries = [];
	for (const file of tracked) if (isSourceManifestPath(file)) entries.push(entryFor(root, file, "tracked"));
	for (const file of nulList(untracked))
		if (!tracked.has(file) && isSourceManifestPath(file)) entries.push(entryFor(root, file, "untracked"));
	return { head, entries };
}

function mayContainSource(relativeDirectory) {
	return SOURCE_MANIFEST_POLICY.includeTrees.some(
		(tree) => tree === relativeDirectory || relativeDirectory.startsWith(`${tree}/`) || tree.startsWith(`${relativeDirectory}/`),
	);
}

/** Filesystem walk for checkouts without Git metadata of their own. Symlinks are never followed. */
function walk(root, relativeDirectory, entries) {
	let names;
	try {
		names = fs.readdirSync(path.join(root, ...relativeDirectory.split("/")), { withFileTypes: true });
	} catch (error) {
		entries.push({ path: `${relativeDirectory}/`, state: "unavailable", reason: `directory unreadable: ${error.code ?? error.message}` });
		return;
	}
	for (const dirent of names) {
		const relative = relativeDirectory ? `${relativeDirectory}/${dirent.name}` : dirent.name;
		if (dirent.isDirectory()) {
			if (!excludedSegments.has(dirent.name) && mayContainSource(relative)) walk(root, relative, entries);
		} else if (isSourceManifestPath(relative)) {
			entries.push(entryFor(root, relative, "present"));
		}
	}
}

function counts(entries) {
	const result = { tracked: 0, untracked: 0, present: 0, deleted: 0, unavailable: 0 };
	for (const entry of entries) result[entry.state] += 1;
	return result;
}

/** The digest input: everything that identifies the source set, nothing about where or when. */
function digestInput(manifest) {
	return {
		version: manifest.version,
		policySha256: manifest.policySha256,
		mode: manifest.mode,
		gitHead: manifest.gitHead,
		deletions: manifest.deletions,
		entries: manifest.entries,
	};
}

export const SOURCE_MANIFEST_POLICY_SHA256 = sha256(SOURCE_MANIFEST_POLICY);

function seal(manifest) {
	return { ...manifest, sourceSha256: sha256(digestInput(manifest)) };
}

/**
 * Hash the implementation sources below `root`. Returns a manifest with `status: "recorded"`
 * (possibly `complete: false` when some files are unreadable), or `status: "unavailable"` when the
 * root cannot be read at all.
 */
export function collectSourceManifest({ root }) {
	let stat;
	try {
		stat = fs.statSync(root);
	} catch (error) {
		return unavailable(`source root is not readable: ${error.code ?? error.message}`);
	}
	if (!stat.isDirectory()) return unavailable("source root is not a directory");
	try {
		fs.readdirSync(root);
	} catch (error) {
		return unavailable(`source root is not listable: ${error.code ?? error.message}`);
	}
	const checkout = ownGitCheckout(root);
	const fromGit = checkout.owns ? gitEntries(root) : null;
	let manifest;
	if (fromGit) {
		manifest = {
			version: SOURCE_MANIFEST_VERSION,
			policySha256: SOURCE_MANIFEST_POLICY_SHA256,
			mode: "git",
			gitHead: fromGit.head ?? unavailable("the checkout has no commit yet"),
			deletions: fromGit.head === null ? "recorded-against-index" : "recorded-against-head-and-index",
			entries: fromGit.entries,
		};
	} else {
		const entries = [];
		walk(root, "", entries);
		manifest = {
			version: SOURCE_MANIFEST_VERSION,
			policySha256: SOURCE_MANIFEST_POLICY_SHA256,
			mode: "filesystem",
			gitHead: unavailable(checkout.owns ? "git metadata present but unreadable" : `git HEAD not used: ${checkout.reason}`),
			deletions: unavailable("no base revision of its own; deleted files cannot be distinguished from files that never existed"),
			entries,
		};
	}
	manifest.entries.sort(compareEntries);
	const tally = counts(manifest.entries);
	return seal({ status: "recorded", ...manifest, complete: tally.unavailable === 0, counts: tally });
}

function entryErrors(entry, index, entries) {
	const at = `entries[${index}]`;
	const errors = [];
	if (typeof entry?.path !== "string" || !isSourceManifestPath(entry.path.replace(/\/$/u, "/x")))
		errors.push(`${at}: path is missing, absolute, escaping or excluded by the source policy`);
	if (!ENTRY_STATES.has(entry?.state)) errors.push(`${at}: unknown state ${entry?.state}`);
	else if (["tracked", "untracked", "present"].includes(entry.state)) {
		if (!["file", "symlink"].includes(entry.kind) || !SHA256.test(entry.sha256 ?? "") || !Number.isSafeInteger(entry.bytes) || entry.bytes < 0)
			errors.push(`${at}: present entries need kind, sha256 and bytes`);
	} else if (entry.state === "unavailable" && typeof entry.reason !== "string") errors.push(`${at}: unavailable entries need a reason`);
	if ("content" in (entry ?? {}) || "contents" in (entry ?? {})) errors.push(`${at}: manifests must not carry file contents`);
	if (index > 0 && compareEntries(entries[index - 1], entry) >= 0) errors.push(`${at}: entries must be sorted and unique by path`);
	return errors;
}

/**
 * Validate an explicitly supplied immutable snapshot manifest: structure, sort order, policy and a
 * recomputed digest. Returns the list of problems; an empty list means the manifest is usable.
 * The bytes it names are not re-read here; the digest proves only that the manifest is intact.
 */
export function sourceManifestErrors(manifest) {
	if (manifest?.status !== "recorded") return ["supplied source manifest is not a recorded manifest"];
	const errors = [];
	if (manifest.version !== SOURCE_MANIFEST_VERSION) errors.push(`unsupported source manifest version ${manifest.version}`);
	if (manifest.policySha256 !== SOURCE_MANIFEST_POLICY_SHA256) errors.push("source manifest was made under a different source policy");
	if (!["git", "filesystem"].includes(manifest.mode)) errors.push(`unknown source manifest mode ${manifest.mode}`);
	if (!Array.isArray(manifest.entries)) return [...errors, "source manifest has no entries array"];
	manifest.entries.forEach((entry, index, entries) => errors.push(...entryErrors(entry, index, entries)));
	const tally = counts(manifest.entries.filter((entry) => ENTRY_STATES.has(entry?.state)));
	if (canonicalJson(manifest.counts) !== canonicalJson(tally)) errors.push("source manifest counts do not match its entries");
	if (manifest.complete !== (tally.unavailable === 0)) errors.push("source manifest completeness does not match its entries");
	if (!SHA256.test(manifest.sourceSha256 ?? "") || manifest.sourceSha256 !== sha256(digestInput(manifest)))
		errors.push("source manifest digest does not match its content");
	return errors;
}

/** Read and validate a supplied snapshot manifest (object or JSON file path); throws on any problem. */
export function loadSuppliedSourceManifest(supplied) {
	const manifest = typeof supplied === "string" ? JSON.parse(fs.readFileSync(supplied, "utf8")) : supplied;
	const errors = sourceManifestErrors(manifest);
	if (errors.length > 0) throw new Error(`invalid supplied source manifest: ${errors.join("; ")}`);
	return manifest;
}

/** The compact identity that a report's build block carries next to the full manifest. */
export function sourceIdentity(manifest, origin) {
	if (manifest.status !== "recorded") return manifest;
	return {
		status: "recorded",
		origin,
		mode: manifest.mode,
		sourceSha256: manifest.sourceSha256,
		complete: manifest.complete,
		counts: manifest.counts,
		gitHead: manifest.gitHead,
		deletions: manifest.deletions,
	};
}
