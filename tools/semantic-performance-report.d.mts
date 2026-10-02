import type { SemanticWorkloadManifest, Unavailable } from "./semantic-performance-workload.mjs";
import type { SourceIdentity, SourceManifest } from "./semantic-source-manifest.mjs";

export const SEMANTIC_REPORT_VERSION: "tosklight.semantic-performance-report/1";
export const REQUIRED_EVIDENCE: Readonly<Record<"output" | "nativeStage" | "semantic", readonly string[]>>;

export interface Measured {
	status: "measured";
	value: number;
	unit: string;
	provenance: { source: string; counter: string; [key: string]: unknown };
}
export type Evidence = Measured | Unavailable;
export type ClaimState = "not-evaluated" | "incomplete" | "evidence-complete";

export function unavailable(reason: string): Unavailable;
export function measured(value: number, unit: string, provenance: Measured["provenance"]): Measured;
export interface BuildIdentity {
	gitHead: string | Unavailable;
	gitTrackedChanges: boolean | Unavailable;
	source: SourceIdentity | Unavailable;
	binary: { path: string; sha256: string; bytes: number } | Unavailable;
	buildProfile: string | Unavailable;
	/** Full manifest; moved out of the report JSON and retained beside it by the writer. */
	sourceManifest?: SourceManifest;
	[key: string]: unknown;
}
export function collectBuildIdentity(options?: {
	repositoryRoot?: string;
	binaryPath?: string;
	buildProfile?: string;
	/** Explicit immutable snapshot manifest (object or JSON file path); its digest is validated. */
	sourceManifest?: string | SourceManifest;
}): BuildIdentity;
export function collectHostIdentity(): Record<string, unknown>;

export interface SemanticPerformanceReport {
	reportVersion: string;
	createdAt: string;
	evidenceSource: "synthetic" | "measured-run";
	identity: Record<string, unknown>;
	rates: { configured: Record<string, unknown>; observed: Record<string, Evidence> };
	metrics: Record<"output" | "nativeStage" | "semantic", Record<string, Evidence>>;
	claims: Record<"output" | "nativeStage", { state: ClaimState; reason: string; missing: string[] }>;
	acceptance: { granted: false; reason: string };
	reportSha256: string;
	/** Non-enumerable: excluded from the report JSON and digest, written once per digest. */
	readonly sourceManifest?: SourceManifest;
	[key: string]: unknown;
}

export function createSemanticPerformanceReport(options: {
	manifest: SemanticWorkloadManifest;
	build?: Record<string, unknown>;
	host?: Record<string, unknown>;
	evidence?: {
		source?: "synthetic" | "measured-run";
		productionSemanticSupport?: { status: string; [key: string]: unknown };
		nativeStageTargetHz?: number;
		output?: Record<string, Evidence>;
		nativeStage?: Record<string, Evidence>;
		semantic?: Record<string, Evidence>;
	};
	createdAt?: string;
}): SemanticPerformanceReport;
export function sourceManifestFileName(sourceSha256: string): string;
export function writeSemanticPerformanceReport(
	report: SemanticPerformanceReport,
	directory?: string,
	options?: { sourceManifest?: SourceManifest },
): Promise<string>;
