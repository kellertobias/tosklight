import * as THREE from "three";
import type {
	AttributeValue,
	FixtureAttributeValues,
	FixtureMode,
	FixtureValuesById,
	StageProfileFixture,
} from "./types";

export function normalized(
	value: AttributeValue | undefined,
	fallback: number,
) {
	return value?.kind === "normalized" ? value.value : fallback;
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

export function headOwnerId(
	fixture: StageProfileFixture,
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
	fixture: StageProfileFixture,
	mode: FixtureMode,
	headId: string,
	byFixture: FixtureValuesById,
) {
	const owner = headOwnerId(fixture, mode, headId);
	const fixtureAttributes = byFixture.get(fixture.fixture_id);
	if (owner === fixture.fixture_id) return fixtureAttributes ?? new Map();
	const attributes = new Map(fixtureAttributes ?? []);
	for (const [attribute, value] of byFixture.get(owner) ?? [])
		attributes.set(attribute, value);
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
