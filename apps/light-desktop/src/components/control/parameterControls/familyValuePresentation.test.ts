import { describe, expect, it } from "vitest";
import type {
	ProgrammingAttributeValue,
	ProgrammingComponent,
	ProgrammingComponentDescriptor,
} from "../../../api/familyEncoderModels";
import type { ProgrammingPositionIntent } from "../../../api/programmingIntentModels";
import { decodeAttributeValue } from "../../../api/programmerValuesWireProjection";
import { decodeColorProgram, decodePositionIntent, decodeZoomIntent } from "../../../api/programmingIntentWire";
import {
	type ComponentDescriptorLookup,
	MIXED_LABEL,
	presentColorProgram,
	presentFocus,
	presentNumber,
	presentPosition,
	presentPositionSelection,
	presentScalar,
	presentScalarSelection,
	presentSelection,
	presentZoom,
	type TargetReferenceLabels,
} from "./familyValuePresentation";

/**
 * Descriptors exactly as `light_core::ProgrammingComponent::descriptor()` serializes them through the
 * generated wire contract (serde f32 shortest decimals). No desktop decoder exists for descriptors,
 * so these are Rust-shaped JSON, parsed rather than hand-built TypeScript objects.
 */
const DESCRIPTOR_JSON: Record<string, string> = {
	pan: '{"owner":"position","role":"angle","unit":"degrees","domain":{"kind":"finite"},"step":1.0,"fine_step":0.1,"display_scale":1.0,"interpolation":"linear","capability":"semantic_intent","spread":true,"align":true,"dynamics":true}',
	tilt: '{"owner":"position","role":"angle","unit":"degrees","domain":{"kind":"finite"},"step":1.0,"fine_step":0.1,"display_scale":1.0,"interpolation":"linear","capability":"semantic_intent","spread":true,"align":true,"dynamics":true}',
	target: '{"owner":"position","role":"target","unit":"metres","domain":{"kind":"finite"},"step":0.1,"fine_step":0.01,"display_scale":1.0,"interpolation":"linear","capability":"semantic_intent","spread":true,"align":true,"dynamics":true}',
	focus: '{"owner":"focus","role":"focus","unit":"percent","domain":{"kind":"bounded","bounds":{"min":0.0,"max":1.0}},"step":0.01,"fine_step":0.001,"display_scale":100.0,"interpolation":"linear","capability":"focus_parameter","spread":true,"align":true,"dynamics":true}',
	zoom: '{"owner":"zoom","role":"zoom","unit":"degrees","domain":{"kind":"bounded","bounds":{"min":0.0,"max":180.0}},"step":1.0,"fine_step":0.1,"display_scale":1.0,"interpolation":"linear","capability":"semantic_intent","spread":true,"align":true,"dynamics":true}',
	recipe: '{"owner":"color","role":"color_recipe","unit":"percent","domain":{"kind":"bounded","bounds":{"min":0.0,"max":1.0}},"step":0.01,"fine_step":0.001,"display_scale":100.0,"interpolation":"linear","capability":"semantic_intent","spread":true,"align":true,"dynamics":true}',
	hue: '{"owner":"color","role":"color_coordinate","unit":"degrees","domain":{"kind":"cyclic","bounds":{"min":0.0,"max":360.0}},"step":1.0,"fine_step":0.1,"display_scale":1.0,"interpolation":"shortest_arc","capability":"semantic_intent","spread":true,"align":true,"dynamics":true}',
	saturation: '{"owner":"color","role":"color_coordinate","unit":"percent","domain":{"kind":"bounded","bounds":{"min":0.0,"max":1.0}},"step":0.01,"fine_step":0.001,"display_scale":100.0,"interpolation":"linear","capability":"semantic_intent","spread":true,"align":true,"dynamics":true}',
	orthogonalPercent: '{"owner":"color","role":"color_orthogonal","unit":"percent","domain":{"kind":"bounded","bounds":{"min":0.0,"max":1.0}},"step":0.01,"fine_step":0.001,"display_scale":100.0,"interpolation":"linear","capability":"semantic_intent","spread":true,"align":true,"dynamics":true}',
	temperature: '{"owner":"color","role":"color_orthogonal","unit":"kelvin","domain":{"kind":"bounded","bounds":{"min":1000.0,"max":20000.0}},"step":100.0,"fine_step":10.0,"display_scale":1.0,"interpolation":"reciprocal","capability":"semantic_intent","spread":true,"align":true,"dynamics":true}',
	duv: '{"owner":"color","role":"color_orthogonal","unit":"duv","domain":{"kind":"bounded","bounds":{"min":-0.03,"max":0.03}},"step":0.001,"fine_step":0.0001,"display_scale":1.0,"interpolation":"linear","capability":"semantic_intent","spread":true,"align":true,"dynamics":true}',
	relativeOutput: '{"owner":"color","role":"color_orthogonal","unit":"factor","domain":{"kind":"bounded","bounds":{"min":0.0,"max":3.4028235e38}},"step":0.01,"fine_step":0.001,"display_scale":1.0,"interpolation":"linear","capability":"semantic_intent","spread":true,"align":true,"dynamics":true}',
	native: '{"owner":"color","role":"native_color","unit":"native_integer","domain":null,"step":1.0,"fine_step":1.0,"display_scale":1.0,"interpolation":"linear","capability":"verified_native_control","spread":false,"align":false,"dynamics":false}',
};
const descriptor = (key: string) => JSON.parse(DESCRIPTOR_JSON[key]) as ProgrammingComponentDescriptor;

const descriptors: ComponentDescriptorLookup = (component: ProgrammingComponent) => {
	switch (component.kind) {
		case "pan":
		case "tilt":
		case "focus":
		case "zoom":
			return descriptor(component.kind);
		case "target_x":
		case "target_y":
		case "target_z":
			return descriptor("target");
		case "native_color":
			return descriptor("native");
		case "color":
			switch (component.component) {
				case "red":
				case "green":
				case "blue":
				case "amber":
					return descriptor("recipe");
				case "hue":
				case "saturation":
				case "temperature":
				case "duv":
					return descriptor(component.component);
				case "white_blend":
				case "uv":
					return descriptor("orthogonalPercent");
				case "relative_output":
					return descriptor("relativeOutput");
			}
			return null;
		default:
			return null;
	}
};

const id = (part: number) => `00000000-0000-0000-0000-${part.toString().padStart(12, "0")}`;
const STAGE_CENTER = id(9);
const DELETED_POINT = id(10);
const labels: TargetReferenceLabels = {
	pointLabel: (pointId) => (pointId === STAGE_CENTER ? "Stage center" : undefined),
	missingPointLabel: "Missing point",
};

/** Every requested value below goes through the production wire decoders first. */
function decoded(json: unknown): ProgrammingAttributeValue {
	return decodeAttributeValue(JSON.parse(JSON.stringify(json)), "$");
}
function position(json: unknown): ProgrammingPositionIntent {
	const value = decoded({ kind: "position", value: json });
	if (value.kind !== "position") throw new Error("expected position");
	return value.value;
}
function semanticColor(intent: Record<string, unknown>) {
	return decodeColorProgram(
		JSON.parse(
			JSON.stringify({
				kind: "semantic",
				intent: {
					base_xyz: { x: 0, y: 0, z: 0 },
					recipe: { version: 1, rgb: [0, 0, 0], amber: 0, approximate: false },
					white_blend: 0,
					white_target: { kelvin: 6500, duv: 0 },
					uv: { amount: 0 },
					relative_output: 1,
					allocation: "preserve_recipe",
					...intent,
				},
			}),
		),
		"$",
	);
}
const source = {
	profile_id: id(1),
	profile_revision: 2,
	profile_digest: "digest",
	mode_id: id(3),
	head_id: id(4),
	path_id: id(5),
	model_revision: 6,
	native_layout_signature: "layout",
};

describe("descriptor units and display scale", () => {
	it("keeps ±900° Pan unwrapped and reads Tilt in signed degrees", () => {
		const presented = presentPosition(
			position({ kind: "angles", pan_degrees: { kind: "value", value: 900 }, tilt_degrees: { kind: "value", value: -135.5 } }),
			descriptors,
			labels,
		);
		expect(presented).toMatchObject({
			kind: "angles",
			pan: { kind: "value", value: { requested: 900, display: 900, text: "900°" } },
			tilt: { kind: "value", text: "-135.5°" },
		});
		const negative = presentPosition(
			position({ kind: "angles", pan_degrees: { kind: "value", value: -900 }, tilt_degrees: { kind: "value", value: 0 } }),
			descriptors,
			labels,
		);
		expect(negative.kind === "angles" && negative.pan.text).toBe("-900°");
		expect(negative.kind === "angles" && negative.tilt.text).toBe("0°");
	});

	it("reads Target offsets in metres with no authored Pan/Tilt", () => {
		const presented = presentPosition(
			position({
				kind: "target",
				reference: { kind: "origin" },
				offset_metres: [{ kind: "value", value: -3 }, { kind: "value", value: 2.25 }, { kind: "value", value: 0 }],
			}),
			descriptors,
			labels,
		);
		expect(presented.kind).toBe("target");
		if (presented.kind !== "target") return;
		expect(presented.reference).toEqual({ kind: "origin", text: "Origin" });
		expect(presented.offsets.map((axis) => axis.text)).toEqual(["-3 m", "2.25 m", "0 m"]);
		expect(presented).not.toHaveProperty("pan");
		expect(presented).not.toHaveProperty("tilt");
		expect(JSON.stringify(presented)).not.toMatch(/°/);
	});

	it("reads Focus as descriptor percent, keeping the normalized request", () => {
		const value = decoded({ kind: "normalized", value: 0.425 });
		if (value.kind !== "normalized") throw new Error("expected normalized");
		expect(presentFocus(value, descriptors)).toMatchObject({ kind: "value", value: { requested: 0.425, display: 42.5, text: "42.5%" } });
		const full = decoded({ kind: "normalized", value: 1 });
		if (full.kind !== "normalized") throw new Error("expected normalized");
		expect(presentFocus(full, descriptors).text).toBe("100%");
	});

	it("reads Zoom as physical degrees with its Beam/Field convention", () => {
		const beam = decoded({ kind: "zoom", value: { opening_degrees: { kind: "value", value: 24 }, convention: "beam" } });
		const field = decodeZoomIntent({ opening_degrees: { kind: "value", value: 37.5 }, convention: "field" }, "$");
		if (beam.kind !== "zoom") throw new Error("expected zoom");
		expect(presentZoom(beam.value, descriptors)).toMatchObject({ convention: "beam", conventionText: "Beam", text: "24° Beam" });
		expect(presentZoom(field, descriptors)).toMatchObject({ convention: "field", conventionText: "Field", text: "37.5° Field" });
	});

	it("reads Kelvin and signed Duv faithfully", () => {
		const warm = presentColorProgram(
			semanticColor({ white_target: { kelvin: 3200, duv: -0.005 } }),
			descriptors,
			{ unknownAppearanceLabel: "Unknown appearance" },
		);
		const green = presentColorProgram(
			semanticColor({ white_target: { kelvin: 5600, duv: 0.012 } }),
			descriptors,
			{ unknownAppearanceLabel: "Unknown appearance" },
		);
		const neutral = presentColorProgram(semanticColor({}), descriptors, { unknownAppearanceLabel: "Unknown appearance" });
		if (warm.kind !== "semantic" || green.kind !== "semantic" || neutral.kind !== "semantic") throw new Error("expected semantic");
		expect(warm.temperature.text).toBe("3200 K");
		expect(warm.duv.text).toBe("-0.0050 Duv");
		expect(green.duv.text).toBe("+0.0120 Duv");
		expect(neutral.duv.text).toBe("0.0000 Duv");
		expect(neutral.temperature.text).toBe("6500 K");
	});

	it("shows an undescribed component plainly instead of inventing a unit or scale", () => {
		expect(presentNumber(0.5, null)).toEqual({ requested: 0.5, display: 0.5, text: "0.5" });
		const none: ComponentDescriptorLookup = () => undefined;
		const value = decoded({ kind: "normalized", value: 0.5 });
		if (value.kind !== "normalized") throw new Error("expected normalized");
		expect(presentFocus(value, none)).toMatchObject({ descriptor: null, text: "0.5" });
	});
});

describe("scalar, spread and mixed presentation", () => {
	it("retains ordered spread endpoints and every control point, never an average", () => {
		const presented = presentPosition(
			position({ kind: "angles", pan_degrees: { kind: "spread", value: [900, 0, -900] }, tilt_degrees: { kind: "value", value: 45 } }),
			descriptors,
			labels,
		);
		if (presented.kind !== "angles" || presented.pan.kind !== "spread") throw new Error("expected Pan spread");
		expect(presented.pan.points.map((point) => point.requested)).toEqual([900, 0, -900]);
		expect(presented.pan.text).toBe("900° thru 0° thru -900°");
	});

	it("keeps a spread with coincident endpoints distinct from a single value", () => {
		const focus = decoded({ kind: "spread", value: [0.5, 0.5] });
		if (focus.kind !== "spread") throw new Error("expected spread");
		const spread = presentFocus(focus, descriptors);
		expect(spread.kind).toBe("spread");
		expect(spread.text).toBe("50% thru 50%");
		const zoom = decoded({ kind: "zoom", value: { opening_degrees: { kind: "spread", value: [60, 4] }, convention: "beam" } });
		if (zoom.kind !== "zoom") throw new Error("expected zoom");
		expect(presentZoom(zoom.value, descriptors).text).toBe("60° thru 4° Beam");
	});

	it("reads a differing selection as Mixed with each distinct request retained", () => {
		const intents = [
			position({ kind: "angles", pan_degrees: { kind: "value", value: -720 }, tilt_degrees: { kind: "value", value: 10 } }),
			position({ kind: "angles", pan_degrees: { kind: "value", value: 720 }, tilt_degrees: { kind: "value", value: 10 } }),
			position({ kind: "angles", pan_degrees: { kind: "value", value: -720 }, tilt_degrees: { kind: "value", value: 10 } }),
		];
		const selection = presentPositionSelection(intents, descriptors, labels);
		if (selection?.kind !== "angles") throw new Error("expected Angles selection");
		expect(selection.pan).toMatchObject({ kind: "mixed", count: 3, text: MIXED_LABEL });
		if (selection.pan.kind !== "mixed") return;
		expect(selection.pan.distinct.map((pan) => pan.text)).toEqual(["-720°", "720°"]);
		expect(JSON.stringify(selection)).not.toMatch(/"0°"/);
		expect(selection.tilt).toMatchObject({ kind: "value", text: "10°" });
	});

	it("compares requested wire values, not rounded text", () => {
		const opening = (value: number) =>
			decodeZoomIntent({ opening_degrees: { kind: "value", value }, convention: "beam" }, "$").opening_degrees;
		const selection = presentScalarSelection([opening(24), opening(24.00001)], descriptor("zoom"));
		expect(selection).toMatchObject({ kind: "mixed", text: "Mixed" });
		if (selection?.kind !== "mixed") return;
		expect(selection.distinct.map((item) => item.text)).toEqual(["24°", "24°"]);
		expect(presentScalarSelection([], descriptor("zoom"))).toBeNull();
		expect(presentSelection([opening(12), opening(12)], (intent) => presentScalar(intent, descriptor("zoom")))).toMatchObject({ kind: "value", text: "12°" });
	});

	it("never merges Angles and Target in one selection", () => {
		const selection = presentPositionSelection(
			[
				position({ kind: "angles", pan_degrees: { kind: "value", value: 0 }, tilt_degrees: { kind: "value", value: 0 } }),
				position({ kind: "target", reference: { kind: "origin" }, offset_metres: [{ kind: "value", value: 0 }, { kind: "value", value: 0 }, { kind: "value", value: 0 }] }),
			],
			descriptors,
			labels,
		);
		expect(selection).toEqual({ kind: "mixed_variant", count: 2, text: "Mixed" });
	});
});

describe("unresolved Points and unknown Direct appearance", () => {
	it("keeps the Point UUID with a resolved name or the caller's missing label", () => {
		const offsets = [{ kind: "value", value: 0 }, { kind: "value", value: 1.5 }, { kind: "spread", value: [-1, 1] }];
		const resolved = presentPosition(
			decodePositionIntent({ kind: "target", reference: { kind: "point", point_id: STAGE_CENTER }, offset_metres: offsets }, "$"),
			descriptors,
			labels,
		);
		const missing = presentPosition(
			decodePositionIntent({ kind: "target", reference: { kind: "point", point_id: DELETED_POINT }, offset_metres: offsets }, "$"),
			descriptors,
			labels,
		);
		expect(resolved).toMatchObject({ kind: "target", reference: { kind: "point", pointId: STAGE_CENTER, resolved: true, text: "Stage center" } });
		expect(missing).toMatchObject({ kind: "target", reference: { kind: "point", pointId: DELETED_POINT, resolved: false, text: "Missing point" } });
		if (missing.kind !== "target") return;
		expect(missing.offsets.map((axis) => axis.text)).toEqual(["0 m", "1.5 m", "-1 m thru 1 m"]);
		expect(missing).not.toHaveProperty("pan");
	});

	it("reads a Target selection reference as Mixed without interpolating Point identities", () => {
		const target = (pointId: string) =>
			position({ kind: "target", reference: { kind: "point", point_id: pointId }, offset_metres: [{ kind: "value", value: 0 }, { kind: "value", value: 0 }, { kind: "value", value: 0 }] });
		const selection = presentPositionSelection([target(STAGE_CENTER), target(DELETED_POINT)], descriptors, labels);
		if (selection?.kind !== "target") throw new Error("expected Target selection");
		expect(selection.reference).toMatchObject({ kind: "mixed", text: "Mixed" });
		if (selection.reference.kind !== "mixed") return;
		expect(selection.reference.distinct).toEqual([
			{ kind: "point", pointId: STAGE_CENTER, resolved: true, text: "Stage center" },
			{ kind: "point", pointId: DELETED_POINT, resolved: false, text: "Missing point" },
		]);
		expect(selection.offsets[0]).toMatchObject({ kind: "value", text: "0 m" });
	});

	it("keeps Direct source identity, exact native integers and an explicit unknown appearance", () => {
		const value = decoded({
			kind: "color_program",
			value: {
				kind: "direct",
				recipe: {
					source,
					channels: [
						{ channel_id: id(7), function_id: id(8), raw: 4294967294 },
						{ channel_id: id(11), function_id: id(12), raw: 0 },
					],
					spreads: [{ binding: { channel_id: id(7), function_id: id(8) }, points: [4294967293, 4294967294] }],
				},
				portable: { model_revision: 6, visible: null, uv: { amount: 0.8, quality: "estimated" }, quality: "unknown", limitations: ["Visible appearance unavailable"] },
			},
		});
		if (value.kind !== "color_program") throw new Error("expected color program");
		const presented = presentColorProgram(value.value, descriptors, { unknownAppearanceLabel: "Unknown appearance" });
		if (presented.kind !== "direct") throw new Error("expected Direct");
		expect(presented.source).toEqual(source);
		expect(presented.channels).toMatchObject([
			{ channelId: id(7), functionId: id(8), value: { kind: "spread", text: "4294967293 thru 4294967294" } },
			{ channelId: id(11), functionId: id(12), value: { kind: "value", text: "0" } },
		]);
		expect(presented.portable.visible).toEqual({ kind: "unknown", text: "Unknown appearance" });
		expect(presented.portable.uv).toMatchObject({ kind: "known", amount: { text: "80%" }, quality: "estimated" });
		expect(presented.portable.quality).toBe("unknown");
		expect(presented.portable.limitations).toEqual(["Visible appearance unavailable"]);
	});

	it("distinguishes a known black Direct estimate from an unknown one", () => {
		const value = decoded({
			kind: "color_program",
			value: {
				kind: "direct",
				recipe: { source, channels: [{ channel_id: id(7), function_id: id(8), raw: 0 }] },
				portable: { model_revision: 6, visible: { xyz: { x: 0, y: 0, z: 0 }, relative_output: 0 }, uv: null, quality: "measured", limitations: [] },
			},
		});
		if (value.kind !== "color_program") throw new Error("expected color program");
		const presented = presentColorProgram(value.value, descriptors, { unknownAppearanceLabel: "Unknown appearance" });
		if (presented.kind !== "direct") throw new Error("expected Direct");
		expect(presented.portable.visible).toEqual({ kind: "known", xyz: { x: 0, y: 0, z: 0 }, relativeOutput: { requested: 0, display: 0, text: "0×" }, black: true });
		expect(presented.portable.uv).toEqual({ kind: "unknown", text: "Unknown appearance" });
	});
});

describe("UV-only black and zero output", () => {
	it("retains UV-only black as a meaningful request without a default-white warning", () => {
		const presented = presentColorProgram(
			semanticColor({ uv: { amount: 0.7 }, relative_output: 0 }),
			descriptors,
			{ unknownAppearanceLabel: "Unknown appearance" },
		);
		if (presented.kind !== "semantic") throw new Error("expected semantic");
		expect(presented.baseXyz).toEqual({ x: 0, y: 0, z: 0 });
		expect(presented.uv.text).toBe("70%");
		expect(presented.relativeOutput.text).toBe("0×");
		expect(presented.recipe.red.text).toBe("0%");
		expect(presented.temperature.text).toBe("6500 K");
		expect(presented.requestedVisibleBlack).toBe(true);
		expect(presented.uvOnly).toBe(true);
		expect(Object.keys(presented)).not.toContain("warning");
		expect(JSON.stringify(presented)).not.toMatch(/warn|default white/i);
	});

	it("keeps a black base at full relative output as black, and White Blend as visible", () => {
		const black = presentColorProgram(semanticColor({}), descriptors, { unknownAppearanceLabel: "Unknown appearance" });
		const whiteBlend = presentColorProgram(semanticColor({ white_blend: 0.25 }), descriptors, { unknownAppearanceLabel: "Unknown appearance" });
		if (black.kind !== "semantic" || whiteBlend.kind !== "semantic") throw new Error("expected semantic");
		expect(black).toMatchObject({ requestedVisibleBlack: true, uvOnly: false, relativeOutput: { text: "1×" } });
		expect(whiteBlend).toMatchObject({ requestedVisibleBlack: false, uvOnly: false, whiteBlend: { text: "25%" } });
	});

	it("reads semantic recipe, approximation and per-component spreads by descriptor", () => {
		const presented = presentColorProgram(
			semanticColor({
				base_xyz: { x: 0.4124564, y: 0.2126729, z: 0.0193339 },
				recipe: { version: 1, rgb: [1, 0, 0], amber: 0, approximate: false },
				spreads: [
					{ component: "hue", points: [350, 10] },
					{ component: "temperature", points: [2700, 6500] },
				],
			}),
			descriptors,
			{ unknownAppearanceLabel: "Unknown appearance" },
		);
		if (presented.kind !== "semantic") throw new Error("expected semantic");
		expect(presented.baseXyz).toEqual({ x: 0.4124564, y: 0.2126729, z: 0.0193339 });
		expect(presented.recipe).toMatchObject({ red: { text: "100%" }, green: { text: "0%" }, approximate: false });
		expect(presented.hue).toMatchObject({ kind: "spread", text: "350° thru 10°" });
		expect(presented.saturation).toBeNull();
		expect(presented.temperature).toMatchObject({ kind: "spread", text: "2700 K thru 6500 K" });
		expect(presented.requestedVisibleBlack).toBe(false);
	});
});
