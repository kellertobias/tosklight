import { modeWithBoundGeometry } from "@tosklight/patch";
import type {
	AttributeValue,
	FixtureMode,
	PatchedFixture,
	VisualizationSnapshot,
} from "../../api/types";
import type { FixtureValuesById } from "./types";

// What drawing a profile's geometry reads lives with that geometry, which the fixture-profile
// editor previews in the Architect as well as on the desk.
export {
	attributesForHead,
	channelDefault,
	headOwnerId,
	normalized,
	resolvedColor,
} from "@tosklight/patch/stage-geometry";

const parameterDefaults = new WeakMap<
	PatchedFixture["definition"],
	Map<string, number>
>();
const profileModes = new WeakMap<PatchedFixture, FixtureMode | null>();

export function parameterDefault(
	fixture: PatchedFixture,
	attribute: string,
	fallback: number,
) {
	let defaults = parameterDefaults.get(fixture.definition);
	if (!defaults) {
		defaults = new Map();
		for (const head of fixture.definition.heads ?? [])
			for (const parameter of head.parameters)
				if (!defaults.has(parameter.attribute))
					defaults.set(parameter.attribute, parameter.default);
		parameterDefaults.set(fixture.definition, defaults);
	}
	return defaults.get(attribute) ?? fallback;
}

export function capabilityName(
	fixture: PatchedFixture,
	attribute: string,
	value: AttributeValue | undefined,
) {
	if (value?.kind === "discrete") return value.value;
	if (value?.kind !== "normalized") return null;
	const raw = Math.round(value.value * 255);
	return (
		fixture.definition.heads
			?.flatMap((head) => head.parameters)
			.find((parameter) => parameter.attribute === attribute)
			?.capabilities?.find(
				(capability) => raw >= capability.dmx_from && raw <= capability.dmx_to,
			)?.name ?? null
	);
}

export function valuesByFixture(
	snapshot: VisualizationSnapshot | null,
): FixtureValuesById {
	const result: FixtureValuesById = new Map();
	const install = (entries: NonNullable<VisualizationSnapshot>["values"]) => {
		for (const entry of entries) {
			const attributes = result.get(entry.fixture_id) ?? new Map();
			attributes.set(entry.attribute, entry.value);
			result.set(entry.fixture_id, attributes);
		}
	};
	install(snapshot?.values ?? []);
	install(snapshot?.profile_output_values ?? []);
	return result;
}

export function profileMode(fixture: PatchedFixture) {
	const retained = profileModes.get(fixture);
	if (retained !== undefined) return retained;
	const profile = fixture.definition.profile_snapshot;
	const selected =
		profile?.modes.find((mode) => mode.id === fixture.definition.mode_id) ??
		profile?.modes.find((mode) => mode.name === fixture.definition.mode) ??
		null;
	// Geometry belongs to the fixture; this mode says which of its heads owns which emitter. Bind
	// them here, once, so everything downstream reads a mode that carries its own graph as before.
	const mode =
		selected && profile ? modeWithBoundGeometry(profile, selected) : selected;
	profileModes.set(fixture, mode);
	return mode;
}
