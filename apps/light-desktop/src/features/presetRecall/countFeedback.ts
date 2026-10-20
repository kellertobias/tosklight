import type { VisualizationSnapshot } from "../../api/types";
import type { ProgrammerValuesProjection } from "../programmerValues/contracts";
import type { ResolvedValueIndex } from "../presetRecording/presetFixtureCounts";
import {
	sameEffectiveValue,
	resolvedValueIndex,
} from "../presetRecording/presetFixtureCounts";
import type { PresetRecallOutcome } from "./contracts";

export interface PresetCountFeedback {
	readonly before: ProgrammerValuesProjection;
	readonly after: ProgrammerValuesProjection;
	readonly baseline: VisualizationSnapshot | null;
}

/** Display-only bridge from an accepted recall to the next causally matching output frame. */
export class PresetCountFeedbackStore {
	private current: PresetCountFeedback | null = null;
	private output: VisualizationSnapshot | null = null;
	private timer: ReturnType<typeof setTimeout> | null = null;
	private readonly listeners = new Set<() => void>();
	readonly getSnapshot = () => this.current;
	readonly subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	};
	accept(
		before: ProgrammerValuesProjection,
		outcome: PresetRecallOutcome,
		live: boolean,
	) {
		if (
			!live ||
			outcome.target !== "programmer" ||
			outcome.disposition !== "recalled" ||
			outcome.status !== "changed" ||
			!outcome.projection
		)
			return;
		const original = this.current?.before ?? before;
		const baseline = this.current?.baseline ?? this.output;
		if (this.timer !== null) clearTimeout(this.timer);
		this.current = { before: original, after: outcome.projection, baseline };
		// No wait before displaying. Expiry only bounds optimistic feedback if output stalls or
		// a higher-priority source prevents the accepted Programmer intent reaching the stage.
		this.timer = setTimeout(() => this.clear(), 5000);
		this.emit();
	}
	observe(
		snapshot: VisualizationSnapshot | null,
		expected: ResolvedValueIndex,
	) {
		this.output = snapshot;
		if (!this.current || !snapshot || !newer(snapshot, this.current.baseline))
			return;
		const resolved = resolvedValueIndex(snapshot);
		if (
			[...expected].every(([fixture, attributes]) =>
				[...attributes].every(([attribute, value]) => {
					const actual = resolved.get(fixture)?.get(attribute);
					return actual && sameEffectiveValue(value, actual);
				}),
			)
		)
			this.clear();
	}
	clear() {
		if (this.timer !== null) clearTimeout(this.timer);
		this.timer = null;
		if (!this.current) return;
		this.current = null;
		this.emit();
	}
	stop() {
		this.clear();
	}
	private emit() {
		for (const listener of this.listeners) listener();
	}
}
function newer(
	snapshot: VisualizationSnapshot,
	baseline: VisualizationSnapshot | null,
) {
	if (!baseline) return true;
	const now = Date.parse(snapshot.generated_at),
		before = Date.parse(baseline.generated_at);
	return Number.isFinite(now) && Number.isFinite(before)
		? now > before
		: snapshot.revision > baseline.revision;
}
