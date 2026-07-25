import type { PresetFamily as ProductPresetFamily } from "../../../apps/light-desktop/src/presetFamilies";
import type { PresetFamily } from "./presetScenario";

export function productPresetFamily(
	family: PresetFamily,
): ProductPresetFamily {
	return family;
}

export function validPresetNumber(number: number) {
	if (!Number.isSafeInteger(number) || number < 1)
		throw new Error("Preset numbers start at 1");
	return number;
}

export function presetCommandAddress(
	family: PresetFamily,
	number: number,
) {
	return `${presetFamilyCode(family)}.${number}`;
}

export function presetAddressKeys(
	family: PresetFamily,
	number: number,
) {
	return [
		`digit-${presetFamilyCode(family)}`,
		"dot",
		...String(number)
			.split("")
			.map((digit) => `digit-${digit}`),
	];
}

export function stablePresetRouteIndex(seed: string, modulo: number) {
	let hash = 0x811c9dc5;
	for (const byte of new TextEncoder().encode(seed)) {
		hash ^= byte;
		hash = Math.imul(hash, 0x01000193);
	}
	return (hash >>> 0) % modulo;
}

function presetFamilyCode(family: PresetFamily) {
	return {
		Mixed: 0,
		Intensity: 1,
		Color: 2,
		Position: 3,
		Beam: 4,
	}[family];
}
