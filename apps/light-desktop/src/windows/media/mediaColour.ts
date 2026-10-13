import { srgbToXyz } from "../../features/presetPreview/colorDisplay";
import type {
	ProgrammerFixtureValue,
	ProgrammerValuesMutation,
} from "../../features/programmerValues/contracts";

/** A picked RGB recipe replaces RGB together, preserving the independent colour envelope. */
export function mediaColourMutation(
	fixtureId: string,
	hex: string,
	values?: readonly ProgrammerFixtureValue[],
): ProgrammerValuesMutation | null {
	const match = /^#([0-9a-f]{6})$/iu.exec(hex);
	if (!match) return null;
	const rgb = [0, 2, 4].map(
		(offset) => Number.parseInt(match[1].slice(offset, offset + 2), 16) / 255,
	) as [number, number, number];
	const current = values?.find(
		(entry) => entry.fixtureId === fixtureId && entry.attribute === "color",
	)?.value;
	const previous =
		current?.kind === "color_program" && current.value.kind === "semantic"
			? current.value.intent
			: null;
	const amber = previous?.recipe.amber ?? 0;
	const xyz = srgbToXyz(...rgb);
	const amberXyz = srgbToXyz(1, 0.5, 0);
	return {
		action: "set_fixture",
		fixtureId,
		attribute: "color",
		value: {
			kind: "color_program",
			value: {
				kind: "semantic",
				intent: {
					white_blend: 0,
					white_target: { kelvin: 6500, duv: 0 },
					uv: { amount: 0 },
					relative_output: 1,
					allocation: "preserve_recipe",
					...previous,
					base_xyz: {
						x: xyz.x + amber * amberXyz.x,
						y: xyz.y + amber * amberXyz.y,
						z: xyz.z + amber * amberXyz.z,
					},
					recipe: { version: 1, rgb, amber, approximate: false },
					spreads: previous?.spreads?.filter(
						(spread) =>
							!["red", "green", "blue", "hue", "saturation"].includes(
								spread.component,
							),
					),
				},
			},
		},
		timing: { fade: false, fadeMillis: null, delayMillis: null },
	};
}
