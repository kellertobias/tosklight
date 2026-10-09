import type { Cue, StoredPreset, PatchedFixture } from "../../api/types";
import {
	usePresets,
	useShowObjectCollectionsReady,
} from "../../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../../features/showObjects/ShowObjectsView";

import {
	usePatchedFixturesView,
	usePatchStatus,
} from "../../features/patch/PatchState";

const PRESET_KINDS = ["preset"] as const;

export function missingCuePresetSources(
	cues: readonly Cue[],
	presets: readonly StoredPreset[],
	fixtures?: readonly PatchedFixture[],
): string[] {
	const sources = new Map(
		presets.map((preset) => [preset.instance_id, preset]),
	);
	const warnings = new Set<string>();
	for (const cue of cues) {
		for (const change of [...cue.changes, ...(cue.group_changes ?? [])]) {
			const reference = change.preset_reference;
			if (!reference) continue;
			const preset = sources.get(reference.preset_instance_id);
			const owner = reference.source_owner;
			const values =
				owner.type === "universal"
					? preset?.universal_values
					: owner.type === "fixture"
						? preset?.values[owner.fixture_id]
						: preset?.group_values?.[owner.group_id];
			const derived =
				owner.type === "universal" &&
				reference.source_attribute === "position" &&
				preset?.aim_at_fixture_number != null;
			if (derived) {
				// Patch loading is not evidence of a deleted target. Keep notices retired until
				// this view has an authoritative current-show fixture collection.
				if (!fixtures) continue;
				const target = fixtures.find(
					(fixture) => fixture.fixture_number === preset.aim_at_fixture_number,
				);
				const isPoint = (fixture: PatchedFixture) =>
					fixture.definition.heads.some((head) =>
						head.parameters.some(
							(parameter) => parameter.attribute === "point.position.x",
						),
					);
				const validPlacement = (fixture: PatchedFixture) =>
					[fixture.location, fixture.rotation].every(
						(vector) => !vector || Object.values(vector).every(Number.isFinite),
					);
				if (
					target &&
					(isPoint(target) ||
						(validPlacement(target) &&
							(!target.position_master ||
								fixtures.some(
									(fixture) =>
										fixture.fixture_id === target.position_master &&
										isPoint(fixture),
								))))
				)
					continue;
			} else if (values?.[reference.source_attribute] != null) continue;
			const destination =
				"fixture_id" in change
					? `fixture ${change.fixture_id}`
					: `Group ${change.group_id}`;
			warnings.add(
				`Cue ${cue.number} · ${destination} · ${change.attribute}: ${derived ? "Aim target is missing or invalid" : preset ? "Preset source value is missing" : "Source Preset is missing"}. The recorded fallback is used. Restore the source or re-record this Cue value.`,
			);
		}
	}
	return [...warnings];
}

export function CuePresetWarnings({
	cues,
	active,
}: {
	cues: readonly Cue[];
	active: boolean;
}) {
	const hasReferences = cues.some((cue) =>
		[...cue.changes, ...(cue.group_changes ?? [])].some(
			(change) => change.preset_reference,
		),
	);
	const enabled = active && hasReferences;
	useShowObjectView("preset", enabled);
	const presets = usePresets(enabled);
	const ready = useShowObjectCollectionsReady(PRESET_KINDS, enabled);
	const derivedIds = new Set(
		presets
			.filter((preset) => preset.body.aim_at_fixture_number != null)
			.map((preset) => preset.body.instance_id),
	);
	const needsPatch =
		enabled &&
		cues.some((cue) =>
			[...cue.changes, ...(cue.group_changes ?? [])].some(
				(change) =>
					change.preset_reference?.source_owner.type === "universal" &&
					change.preset_reference.source_attribute === "position" &&
					derivedIds.has(change.preset_reference.preset_instance_id),
			),
		);
	const fixtures = usePatchedFixturesView(needsPatch);
	const patch = usePatchStatus(needsPatch);
	const warnings =
		enabled && ready
			? missingCuePresetSources(
					cues,
					presets.map((preset) => preset.body),
					needsPatch && patch.status === "ready" ? fixtures : undefined,
				)
			: [];
	if (!warnings.length) return null;
	return (
		<aside
			role="status"
			aria-label="Cue Preset source warnings"
			className="cue-preset-warnings"
		>
			<ul>
				{warnings.map((warning) => (
					<li key={warning}>{warning}</li>
				))}
			</ul>
		</aside>
	);
}
