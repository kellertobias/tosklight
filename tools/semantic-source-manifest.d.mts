import type { Unavailable } from "./semantic-performance-workload.mjs";

export const SOURCE_MANIFEST_VERSION: "tosklight.semantic-source-manifest/1";
export const SOURCE_MANIFEST_POLICY: Readonly<{
	includeRootFiles: boolean;
	includeTrees: readonly string[];
	excludeSegments: readonly string[];
	excludeBasenames: readonly string[];
	keepBasenames: readonly string[];
}>;
export const SOURCE_MANIFEST_POLICY_SHA256: string;

export type SourceManifestEntry =
	| { path: string; state: "tracked" | "untracked" | "present"; kind: "file" | "symlink"; sha256: string; bytes: number }
	| { path: string; state: "deleted" }
	| { path: string; state: "unavailable"; reason: string };

export interface SourceManifestCounts {
	tracked: number;
	untracked: number;
	present: number;
	deleted: number;
	unavailable: number;
}

export interface SourceManifest {
	status: "recorded";
	version: typeof SOURCE_MANIFEST_VERSION;
	policySha256: string;
	mode: "git" | "filesystem";
	gitHead: string | Unavailable;
	deletions: "recorded-against-head-and-index" | "recorded-against-index" | Unavailable;
	entries: SourceManifestEntry[];
	complete: boolean;
	counts: SourceManifestCounts;
	sourceSha256: string;
}

export interface SourceIdentity {
	status: "recorded";
	origin: "collected" | "supplied-snapshot-manifest";
	mode: SourceManifest["mode"];
	sourceSha256: string;
	complete: boolean;
	counts: SourceManifestCounts;
	gitHead: SourceManifest["gitHead"];
	deletions: SourceManifest["deletions"];
}

export function isSourceManifestPath(relativePath: string): boolean;
export function ownGitCheckout(root: string): { owns: true } | { owns: false; reason: string };
export function collectSourceManifest(options: { root: string }): SourceManifest | Unavailable;
export function sourceManifestErrors(manifest: unknown): string[];
export function loadSuppliedSourceManifest(supplied: string | SourceManifest): SourceManifest;
export function sourceIdentity(manifest: SourceManifest | Unavailable, origin: SourceIdentity["origin"]): SourceIdentity | Unavailable;
