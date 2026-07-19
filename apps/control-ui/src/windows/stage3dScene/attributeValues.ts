import * as THREE from "three";
import type {
	AttributeValue,
	FixtureMode,
	PatchedFixture,
	VisualizationSnapshot,
} from "../../api/types";
import type {
	FixtureAttributeValues,
	FixtureValuesById,
} from "./types";

export function normalized(
	value: AttributeValue | undefined,
	fallback: number,
) {
	return value?.kind === "normalized" ? value.value : fallback;
}

export function parameterDefault(
	fixture: PatchedFixture,
	attribute: string,
	fallback: number,
) {
	return (
		fixture.definition.heads
			?.flatMap((head) => head.parameters)
			.find((parameter) => parameter.attribute === attribute)?.default ??
		fallback
	);
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

function xyzChannelToSrgb(channel: number) {
	return channel <= 0.0031308
		? 12.92 * channel
		: 1.055 * channel ** (1 / 2.4) - 0.055;
}

function xyzColor(value: Extract<AttributeValue, { kind: "color_xyz" }>) {
	const { x, y, z } = value.value;
	return new THREE.Color(
		xyzChannelToSrgb(3.2406 * x - 1.5372 * y - 0.4986 * z),
		xyzChannelToSrgb(-0.9689 * x + 1.8758 * y + 0.0415 * z),
		xyzChannelToSrgb(0.0557 * x - 0.204 * y + 1.057 * z),
	);
}

export function resolvedColor(
	value: AttributeValue | undefined,
	attributes: FixtureAttributeValues,
) {
	if (value?.kind === "color_xyz") return xyzColor(value);
	return new THREE.Color(
		normalized(attributes.get("color.red"), 1),
		normalized(attributes.get("color.green"), 1),
		normalized(attributes.get("color.blue"), 1),
	);
}

export function valuesByFixture(
	snapshot: VisualizationSnapshot | null,
): FixtureValuesById {
	const result: FixtureValuesById = new Map();
	for (const entry of [
		...(snapshot?.values ?? []),
		...(snapshot?.profile_output_values ?? []),
	]) {
		const attributes = result.get(entry.fixture_id) ?? new Map();
		attributes.set(entry.attribute, entry.value);
		result.set(entry.fixture_id, attributes);
	}
	return result;
}

export function profileMode(fixture: PatchedFixture) {
	const profile = fixture.definition.profile_snapshot;
	return (
		profile?.modes.find((mode) => mode.id === fixture.definition.mode_id) ??
		profile?.modes.find((mode) => mode.name === fixture.definition.mode) ??
		null
	);
}

export function headOwnerId(
	fixture: PatchedFixture,
	mode: FixtureMode,
	headId: string,
) {
	const index = mode.heads.findIndex((head) => head.id === headId);
	const head = mode.heads[index];
	if (!head || head.master_shared) return fixture.fixture_id;
	return (
		fixture.logical_heads.find((candidate) => candidate.head_index === index)
			?.fixture_id ??
		fixture.logical_heads.find(
			(candidate) => candidate.head_index === index + 1,
		)?.fixture_id ??
		fixture.fixture_id
	);
}

export function attributesForHead(
	fixture: PatchedFixture,
	mode: FixtureMode,
	headId: string,
	byFixture: FixtureValuesById,
) {
	const attributes = new Map(byFixture.get(fixture.fixture_id) ?? []);
	const owner = headOwnerId(fixture, mode, headId);
	if (owner !== fixture.fixture_id) {
		for (const [attribute, value] of byFixture.get(owner) ?? []) {
			attributes.set(attribute, value);
		}
	}
	return attributes;
}

export function channelDefault(
	mode: FixtureMode,
	headId: string,
	attribute: string,
	fallback: number,
) {
	const channel = mode.channels.find(
		(candidate) =>
			candidate.head_id === headId && candidate.attribute === attribute,
	);
	if (!channel) return fallback;
	const maximum = { u8: 0xff, u16: 0xffff, u24: 0xffffff, u32: 0xffffffff }[
		channel.resolution
	];
	return channel.default_raw / maximum;
}
