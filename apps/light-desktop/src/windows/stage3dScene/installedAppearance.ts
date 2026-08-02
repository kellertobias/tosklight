import * as THREE from "three";
import type {
	GelAssignment,
	InstalledFixtureAppearance,
	PatchedFixture,
} from "../../api/types";

const NEUTRAL = new THREE.Color(1, 1, 1);

function srgbChannelToLinear(channel: number) {
	return channel <= 0.04045
		? channel / 12.92
		: ((channel + 0.055) / 1.055) ** 2.4;
}

/** Decode the persisted canonical sRGB form without consulting a local catalog. */
export function parseCanonicalSrgbHexLinear(value: string) {
	if (!/^#[0-9A-F]{6}$/.test(value)) return null;
	return new THREE.Color(
		srgbChannelToLinear(Number.parseInt(value.slice(1, 3), 16) / 255),
		srgbChannelToLinear(Number.parseInt(value.slice(3, 5), 16) / 255),
		srgbChannelToLinear(Number.parseInt(value.slice(5, 7), 16) / 255),
	);
}

/** The same bounded CCT approximation used by the standalone Viz renderer. */
export function colorTemperatureLinearRgb(kelvin: number) {
	const temperature = Math.min(25_000, Math.max(1_000, kelvin)) / 100;
	const red =
		temperature <= 66 ? 255 : 329.69873 * (temperature - 60) ** -0.13320476;
	const green =
		temperature <= 66
			? 99.4708 * Math.log(temperature) - 161.11957
			: 288.12216 * (temperature - 60) ** -0.075514846;
	const blue =
		temperature >= 66
			? 255
			: temperature <= 19
				? 0
				: 138.51773 * Math.log(temperature - 10) - 305.0448;
	return new THREE.Color(
		srgbChannelToLinear(Math.min(255, Math.max(0, red)) / 255),
		srgbChannelToLinear(Math.min(255, Math.max(0, green)) / 255),
		srgbChannelToLinear(Math.min(255, Math.max(0, blue)) / 255),
	);
}

function gelLinearRgb(gel: GelAssignment | undefined) {
	if (!gel || gel.type === "open_white") return NEUTRAL;
	const encoded =
		gel.type === "built_in"
			? gel.embedded_fallback.visualizer_srgb
			: gel.color_srgb;
	return parseCanonicalSrgbHexLinear(encoded) ?? NEUTRAL;
}

export function installedAppearanceLinearRgb(
	fixture: PatchedFixture,
	appearance: InstalledFixtureAppearance | undefined,
) {
	const profileCct =
		fixture.definition.profile_snapshot?.physical.color_temperature_kelvin;
	const cct = appearance?.color_temperature_kelvin ?? profileCct;
	const source =
		typeof cct === "number" && Number.isFinite(cct)
			? colorTemperatureLinearRgb(Math.round(cct))
			: NEUTRAL;
	return source.clone().multiply(gelLinearRgb(appearance?.gel));
}

/** Compose live fixture colour with installed source and gel in linear space. */
export function applyInstalledAppearance(
	liveLinearRgb: THREE.Color,
	fixture: PatchedFixture,
	appearance: InstalledFixtureAppearance | undefined,
) {
	return liveLinearRgb
		.clone()
		.multiply(installedAppearanceLinearRgb(fixture, appearance));
}
