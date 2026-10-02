/**
 * Semantic programming values for the planned demo generator (`76-demo-show-generation`).
 *
 * TL-552: the generator authors programming contract 1 intent only: whole-family semantic Color,
 * Position Angles in degrees, and typed Dynamic lanes. The legacy normalized `pan`/`tilt` and
 * `color.red/green/blue/white` branch (TL-560's `LIGHT_DEMO_SEMANTIC` switch) is gone, because a
 * contract-1 desk rejects that programming. The committed `assets/demo.show` is generated from
 * this path.
 *
 * The Color values are the exact contract-1 serialization (recipe, authoritative XYZ derived
 * from the recipe by `VirtualColorAuthoringV1::recipe_xyz`, default White Blend/target, UV 0,
 * relative output 1), produced by the Rust core so validation accepts them bit for bit.
 * Normalized Position maps to degrees over the demo movers' nominal 540° pan and 270° tilt
 * travel, centred: degrees = (normalized − 0.5) × travel.
 */

export const PAN_TRAVEL_DEGREES = 540;
export const TILT_TRAVEL_DEGREES = 270;

export const SEMANTIC_COLORS: Readonly<Record<string, object>> = {
	"Red": {"kind": "color_program", "value": {"intent": {"allocation": "preserve_recipe", "base_xyz": {"x": 0.4124563932418823, "y": 0.2126729041337967, "z": 0.01933390088379383}, "recipe": {"amber": 0.0, "approximate": false, "rgb": [1.0, 0.0, 0.0], "version": 1}, "relative_output": 1.0, "uv": {"amount": 0.0}, "white_blend": 0.0, "white_target": {"duv": 0.0, "kelvin": 6500.0}}, "kind": "semantic"}},
	"Orange": {"kind": "color_program", "value": {"intent": {"allocation": "preserve_recipe", "base_xyz": {"x": 0.44838619232177734, "y": 0.28453248739242554, "z": 0.03131049498915672}, "recipe": {"amber": 0.0, "approximate": false, "rgb": [1.0, 0.3499999940395355, 0.0], "version": 1}, "relative_output": 1.0, "uv": {"amount": 0.0}, "white_blend": 0.0, "white_target": {"duv": 0.0, "kelvin": 6500.0}}, "kind": "semantic"}},
	"Yellow": {"kind": "color_program", "value": {"intent": {"allocation": "preserve_recipe", "base_xyz": {"x": 0.770032525062561, "y": 0.9278250932693481, "z": 0.13852590322494507}, "recipe": {"amber": 0.0, "approximate": false, "rgb": [1.0, 1.0, 0.0], "version": 1}, "relative_output": 1.0, "uv": {"amount": 0.0}, "white_blend": 0.0, "white_target": {"duv": 0.0, "kelvin": 6500.0}}, "kind": "semantic"}},
	"Lime": {"kind": "color_program", "value": {"intent": {"allocation": "preserve_recipe", "base_xyz": {"x": 0.4661649167537689, "y": 0.7711433172225952, "z": 0.1242820993065834}, "recipe": {"amber": 0.0, "approximate": false, "rgb": [0.550000011920929, 1.0, 0.0], "version": 1}, "relative_output": 1.0, "uv": {"amount": 0.0}, "white_blend": 0.0, "white_target": {"duv": 0.0, "kelvin": 6500.0}}, "kind": "semantic"}},
	"Green": {"kind": "color_program", "value": {"intent": {"allocation": "preserve_recipe", "base_xyz": {"x": 0.3575761020183563, "y": 0.7151522040367126, "z": 0.11919199675321579}, "recipe": {"amber": 0.0, "approximate": false, "rgb": [0.0, 1.0, 0.0], "version": 1}, "relative_output": 1.0, "uv": {"amount": 0.0}, "white_blend": 0.0, "white_target": {"duv": 0.0, "kelvin": 6500.0}}, "kind": "semantic"}},
	"Teal": {"kind": "color_program", "value": {"intent": {"allocation": "preserve_recipe", "base_xyz": {"x": 0.12493102997541428, "y": 0.2005968689918518, "z": 0.1935446709394455}, "recipe": {"amber": 0.0, "approximate": false, "rgb": [0.0, 0.550000011920929, 0.44999998807907104], "version": 1}, "relative_output": 1.0, "uv": {"amount": 0.0}, "white_blend": 0.0, "white_target": {"duv": 0.0, "kelvin": 6500.0}}, "kind": "semantic"}},
	"Cyan": {"kind": "color_program", "value": {"intent": {"allocation": "preserve_recipe", "base_xyz": {"x": 0.5380135774612427, "y": 0.787327229976654, "z": 1.0694960355758667}, "recipe": {"amber": 0.0, "approximate": false, "rgb": [0.0, 1.0, 1.0], "version": 1}, "relative_output": 1.0, "uv": {"amount": 0.0}, "white_blend": 0.0, "white_target": {"duv": 0.0, "kelvin": 6500.0}}, "kind": "semantic"}},
	"Light Blue": {"kind": "color_program", "value": {"intent": {"allocation": "preserve_recipe", "base_xyz": {"x": 0.3373207449913025, "y": 0.35479307174682617, "z": 0.9965873956680298}, "recipe": {"amber": 0.0, "approximate": false, "rgb": [0.25, 0.6499999761581421, 1.0], "version": 1}, "relative_output": 1.0, "uv": {"amount": 0.0}, "white_blend": 0.0, "white_target": {"duv": 0.0, "kelvin": 6500.0}}, "kind": "semantic"}},
	"Dark Blue": {"kind": "color_program", "value": {"intent": {"allocation": "preserve_recipe", "base_xyz": {"x": 0.18043750524520874, "y": 0.07217500358819962, "z": 0.9503040909767151}, "recipe": {"amber": 0.0, "approximate": false, "rgb": [0.0, 0.0, 1.0], "version": 1}, "relative_output": 1.0, "uv": {"amount": 0.0}, "white_blend": 0.0, "white_target": {"duv": 0.0, "kelvin": 6500.0}}, "kind": "semantic"}},
	"Purple": {"kind": "color_program", "value": {"intent": {"allocation": "preserve_recipe", "base_xyz": {"x": 0.17933669686317444, "y": 0.07987280189990997, "z": 0.5771188735961914}, "recipe": {"amber": 0.0, "approximate": false, "rgb": [0.44999998807907104, 0.0, 0.800000011920929], "version": 1}, "relative_output": 1.0, "uv": {"amount": 0.0}, "white_blend": 0.0, "white_target": {"duv": 0.0, "kelvin": 6500.0}}, "kind": "semantic"}},
	"Magenta": {"kind": "color_program", "value": {"intent": {"allocation": "preserve_recipe", "base_xyz": {"x": 0.5928938984870911, "y": 0.2848479151725769, "z": 0.9696379899978638}, "recipe": {"amber": 0.0, "approximate": false, "rgb": [1.0, 0.0, 1.0], "version": 1}, "relative_output": 1.0, "uv": {"amount": 0.0}, "white_blend": 0.0, "white_target": {"duv": 0.0, "kelvin": 6500.0}}, "kind": "semantic"}},
	"White": {"kind": "color_program", "value": {"intent": {"allocation": "preserve_recipe", "base_xyz": {"x": 0.9504700303077698, "y": 1.0000001192092896, "z": 1.0888299942016602}, "recipe": {"amber": 0.0, "approximate": false, "rgb": [1.0, 1.0, 1.0], "version": 1}, "relative_output": 1.0, "uv": {"amount": 0.0}, "white_blend": 0.0, "white_target": {"duv": 0.0, "kelvin": 6500.0}}, "kind": "semantic"}},
	"Tungsten White": {"kind": "color_program", "value": {"intent": {"allocation": "preserve_recipe", "base_xyz": {"x": 0.5499603152275085, "y": 0.4635641872882843, "z": 0.13952800631523132}, "recipe": {"amber": 0.0, "approximate": false, "rgb": [1.0, 0.6200000047683716, 0.3199999928474426], "version": 1}, "relative_output": 1.0, "uv": {"amount": 0.0}, "white_blend": 0.0, "white_target": {"duv": 0.0, "kelvin": 6500.0}}, "kind": "semantic"}},
};

export function semanticColor(name: string): object {
	const value = SEMANTIC_COLORS[name];
	if (!value) throw new Error(`No semantic demo Color named ${name}`);
	return value;
}

export const panDegrees = (normalized: number) =>
	(normalized - 0.5) * PAN_TRAVEL_DEGREES;
export const tiltDegrees = (normalized: number) =>
	(normalized - 0.5) * TILT_TRAVEL_DEGREES;

/** Position Angles from the legacy normalized pair. */
export function semanticAngles(pan: number, tilt: number) {
	return {
		kind: "position",
		value: {
			kind: "angles",
			pan_degrees: { kind: "value", value: panDegrees(pan) },
			tilt_degrees: { kind: "value", value: tiltDegrees(tilt) },
		},
	};
}

const scalar = (value: number) => ({
	kind: "value",
	value: { kind: "scalar", value },
});

/** Typed lane address and the scalar domain conversion for one legacy lane attribute. */
function typedAddress(attribute: string) {
	switch (attribute) {
		case "pan":
			return {
				address: { representation: { kind: "angles" }, component: { kind: "pan" } },
				convert: panDegrees,
				span: PAN_TRAVEL_DEGREES,
			};
		case "tilt":
			return {
				address: { representation: { kind: "angles" }, component: { kind: "tilt" } },
				convert: tiltDegrees,
				span: TILT_TRAVEL_DEGREES,
			};
		case "color.red":
		case "color.green":
		case "color.blue":
			return {
				address: {
					representation: { kind: "semantic_color", basis: "recipe" },
					component: { kind: "color", component: attribute.slice("color.".length) },
				},
				convert: (value: number) => value,
				span: 1,
			};
		case "color.white":
			return {
				address: {
					representation: { kind: "semantic_color", basis: "recipe" },
					component: { kind: "color", component: "white_blend" },
				},
				convert: (value: number) => value,
				span: 1,
			};
		default:
			return undefined;
	}
}

/**
 * Converts one legacy scalar lane (as built by `plannedDemoDynamics`) into a typed Programming
 * lane on the owning family, or returns it unchanged for non-family attributes (intensity).
 */
export function semanticLane(legacy: any): any {
	const typed = typedAddress(legacy.attribute);
	if (!typed) return legacy;
	const { attribute, mode, keyframes, max_min, middle_amplitude, ...shared } = legacy;
	const source = (value: any) =>
		value?.type === "value" ? scalar(typed.convert(value.value)) : { kind: "current" };
	const configuration =
		mode === "keyframes"
			? {
					...keyframes,
					points: keyframes.points.map((point: any) => ({
						...point,
						source: source(point.source),
					})),
				}
			: mode === "max_min"
				? {
						...max_min,
						minimum: source(max_min.minimum),
						maximum: source(max_min.maximum),
					}
				: mode === "middle_amplitude"
					? {
							...middle_amplitude,
							middle: source(middle_amplitude.middle),
							amplitude: {
								kind: "scalar",
								value: middle_amplitude.amplitude * typed.span,
							},
						}
					: undefined;
	return {
		...shared,
		programming: {
			address: typed.address,
			configuration:
				mode === "random" ? { mode: "random" } : { mode, configuration },
		},
	};
}

/** A random group shared by typed lanes uses the typed range over the 0–1 recipe domain. */
export function semanticRandomGroup(legacy: any): any {
	const { low, high, ...shared } = legacy;
	return {
		...shared,
		programming_range: { low: scalar(low.value), high: scalar(high.value) },
	};
}
