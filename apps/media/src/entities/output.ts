// Typed models over an output projection.

import type { LayerView, OutputView, SourceStatusView } from "../shared/api/generated/media-wire";

export type SourceTone = "neutral" | "busy" | "good" | "bad";

export interface SourceBadge {
	label: string;
	tone: SourceTone;
	/** Operator-safe detail, when there is any. Never a path or a decoder message. */
	detail: string | undefined;
}

const BADGES: Record<string, { label: string; tone: SourceTone }> = {
	unselected: { label: "Empty", tone: "neutral" },
	loading: { label: "Loading", tone: "busy" },
	ready: { label: "Ready", tone: "good" },
	completed: { label: "Finished", tone: "neutral" },
	failed: { label: "Failed", tone: "bad" },
};

export function sourceBadge(status: SourceStatusView): SourceBadge {
	const badge = BADGES[status.state] ?? { label: status.state, tone: "neutral" as SourceTone };
	return { ...badge, detail: status.failure ?? undefined };
}

export function layerName(layer: LayerView): string {
	return `Layer ${layer.index + 1}`;
}

/** Percent, rounded, for a value the server keeps as 0–1. */
export function percent(value: number): string {
	return `${Math.round(value * 100)}%`;
}

/**
 * Why the web interface may not write to this output right now, or nothing.
 *
 * A desk owning an output is normal operation, not an error, so the wording explains rather than
 * warns — and every control this covers is disabled rather than silently ignored.
 */
export function readOnlyReason(output: OutputView | undefined): string | undefined {
	if (!output?.dmxActive) return undefined;
	return "A lighting desk is driving this output. Its values change here when the desk stops sending.";
}

export function layersDrawing(output: OutputView | undefined): number {
	return output?.layers.filter((layer) => layer.drawing).length ?? 0;
}
