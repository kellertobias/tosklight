import type { Cue, StoredPreset } from "../../api/types";
import {
	usePresets,
	useShowObjectCollectionsReady,
} from "../../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../../features/showObjects/ShowObjectsView";

const PRESET_KINDS = ["preset"] as const;

export function missingCuePresetSources(
	cues: readonly Cue[],
	presets: readonly StoredPreset[],
): string[] {
	const sources = new Map(presets.map((preset) => [preset.instance_id, preset]));
	const warnings = new Set<string>();
	for (const cue of cues) {
		for (const change of [...cue.changes, ...(cue.group_changes ?? [])]) {
			const reference = change.preset_reference;
			if (!reference) continue;
			const preset = sources.get(reference.preset_instance_id);
			const owner = reference.source_owner;
			const values = owner.type === "universal"
				? preset?.universal_values
				: owner.type === "fixture"
					? preset?.values[owner.fixture_id]
					: preset?.group_values?.[owner.group_id];
			if (values?.[reference.source_attribute] != null) continue;
			const destination = "fixture_id" in change
				? `fixture ${change.fixture_id}`
				: `Group ${change.group_id}`;
			warnings.add(`Cue ${cue.number} · ${destination} · ${change.attribute}: ${preset ? "Preset source value is missing" : "Source Preset is missing"}. The recorded fallback is used. Restore the source or re-record this Cue value.`);
		}
	}
	return [...warnings];
}

export function CuePresetWarnings({ cues, active }: { cues: readonly Cue[]; active: boolean }) {
	const hasReferences = cues.some((cue) =>
		[...cue.changes, ...(cue.group_changes ?? [])].some((change) => change.preset_reference),
	);
	const enabled = active && hasReferences;
	useShowObjectView("preset", enabled);
	const presets = usePresets(enabled);
	const ready = useShowObjectCollectionsReady(PRESET_KINDS, enabled);
	const warnings = enabled && ready
		? missingCuePresetSources(cues, presets.map((preset) => preset.body))
		: [];
	if (!warnings.length) return null;
	return (
		<aside role="status" aria-label="Cue Preset source warnings" className="cue-preset-warnings">
			<ul>{warnings.map((warning) => <li key={warning}>{warning}</li>)}</ul>
		</aside>
	);
}
