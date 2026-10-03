import type {
	ProgrammingAttributeValue,
	ProgrammingColorComponent,
	ProgrammingComponent,
	ProgrammingComponentDescriptor,
	ProgrammingScalarIntent,
	ProgrammingTargetReference,
} from "../../../api/familyEncoderModels";
import type {
	ProgrammingColorIntent,
	ProgrammingColorProgram,
	ProgrammingColorXyz,
	ProgrammingNativeColorIdentity,
	ProgrammingNativeColorRecipe,
	ProgrammingOpeningConvention,
	ProgrammingPhysicalDataQuality,
	ProgrammingPortableColorEstimate,
	ProgrammingPositionIntent,
	ProgrammingZoomIntent,
} from "../../../api/programmingIntentModels";

/**
 * Operator readouts of *requested* Position, Color, Focus and Zoom family values.
 *
 * Everything here describes what the Programmer, a Preset or a Cue asks for, read in the units its
 * published `ProgrammingComponentDescriptor` declares. It is deliberately not a readout of what a
 * lamp achieves: a Target has no authored Pan/Tilt, and resolved angles, fitted colour and reachable
 * zoom come only from the coherent published frame. Nothing here solves, normalizes against a patch,
 * averages a selection or warns about a "default white" — a black or UV-only request is a request.
 */

/** Shown wherever the selection holds more than one requested value; matches the desk's wording. */
export const MIXED_LABEL = "Mixed";
/** Joins ordered spread control points; mirrors the `[THRU]` entry that authored them. */
export const SPREAD_SEPARATOR = " thru ";
/** The fixed-coordinate Target reference. */
export const ORIGIN_LABEL = "Origin";

/**
 * Supplies the published descriptor of one component. Returning nothing leaves that component
 * undescribed: its requested number is shown plainly, without a scale or unit the desk would have to
 * invent.
 */
export type ComponentDescriptorLookup = (
	component: ProgrammingComponent,
) => ProgrammingComponentDescriptor | null | undefined;

export interface PresentedNumber {
	/** The requested value exactly as programmed. */
	requested: number;
	/** `requested × display_scale`, rounded to the descriptor's fine step. */
	display: number;
	text: string;
}

export type ScalarPresentation =
	| {
			kind: "value";
			descriptor: ProgrammingComponentDescriptor | null;
			value: PresentedNumber;
			text: string;
	  }
	| {
			kind: "spread";
			descriptor: ProgrammingComponentDescriptor | null;
			/** Every control point, in authored selection order; never sorted or averaged. */
			points: PresentedNumber[];
			text: string;
	  };

export interface MixedPresentation<T> {
	kind: "mixed";
	/** How many requested values were compared. */
	count: number;
	/** Each distinct requested value, in first-seen order. */
	distinct: T[];
	text: string;
}

export type SelectionPresentation<T> = T | MixedPresentation<T>;

// ---------------------------------------------------------------------------------------------
// Numbers and scalar intents
// ---------------------------------------------------------------------------------------------

const MAX_DECIMALS = 6;

function decimalsFor(descriptor: ProgrammingComponentDescriptor): number {
	const resolution = Math.abs(descriptor.fine_step * descriptor.display_scale);
	if (!Number.isFinite(resolution) || resolution <= 0) return MAX_DECIMALS;
	// f32 steps such as 0.001 arrive as 0.0010000000474974513; the epsilon keeps them at 3.
	return Math.min(MAX_DECIMALS, Math.max(0, Math.ceil(-Math.log10(resolution) - 1e-6)));
}

function trimmed(fixed: string): string {
	return fixed.includes(".") ? fixed.replace(/\.?0+$/, "") : fixed;
}

function withoutNegativeZero(fixed: string): string {
	return Number(fixed) === 0 ? fixed.replace(/^-/, "") : fixed;
}

const UNIT_SUFFIX: Record<ProgrammingComponentDescriptor["unit"], string> = {
	percent: "%",
	degrees: "°",
	metres: " m",
	kelvin: " K",
	duv: " Duv",
	factor: "×",
	native_integer: "",
	selection: "",
};

/** One requested number in its descriptor's display unit. */
export function presentNumber(
	requested: number,
	descriptor: ProgrammingComponentDescriptor | null | undefined,
): PresentedNumber {
	if (!descriptor) {
		const text = withoutNegativeZero(trimmed(requested.toFixed(MAX_DECIMALS)));
		return { requested, display: Number(text), text };
	}
	if (descriptor.unit === "native_integer" || descriptor.unit === "selection") {
		const text = withoutNegativeZero(Math.round(requested * descriptor.display_scale).toFixed(0));
		return { requested, display: Number(text), text };
	}
	const decimals = decimalsFor(descriptor);
	const scaled = requested * descriptor.display_scale;
	const fixed = withoutNegativeZero(scaled.toFixed(decimals));
	const display = Number(fixed);
	if (descriptor.unit === "duv") {
		// Duv is signed: positive toward green, negative toward magenta. Keep its full resolution.
		const signed = display > 0 ? `+${fixed}` : fixed;
		return { requested, display, text: `${signed}${UNIT_SUFFIX.duv}` };
	}
	return { requested, display, text: `${trimmed(fixed)}${UNIT_SUFFIX[descriptor.unit]}` };
}

/** A typed scalar or ordered spread, kept distinct even when a spread's endpoints coincide. */
export function presentScalar(
	intent: ProgrammingScalarIntent,
	descriptor: ProgrammingComponentDescriptor | null | undefined,
): ScalarPresentation {
	const described = descriptor ?? null;
	if (intent.kind === "value") {
		const value = presentNumber(intent.value, described);
		return { kind: "value", descriptor: described, value, text: value.text };
	}
	const points = intent.value.map((point) => presentNumber(point, described));
	return {
		kind: "spread",
		descriptor: described,
		points,
		text: points.map((point) => point.text).join(SPREAD_SEPARATOR),
	};
}

// ---------------------------------------------------------------------------------------------
// Selection comparison
// ---------------------------------------------------------------------------------------------

function sameValue(left: unknown, right: unknown): boolean {
	if (Object.is(left, right)) return true;
	if (typeof left === "number" && typeof right === "number") return left === right;
	if (typeof left !== "object" || typeof right !== "object" || !left || !right) return false;
	if (Array.isArray(left) !== Array.isArray(right)) return false;
	if (Array.isArray(left) && Array.isArray(right)) {
		return left.length === right.length && left.every((item, index) => sameValue(item, right[index]));
	}
	const leftRecord = left as Record<string, unknown>;
	const rightRecord = right as Record<string, unknown>;
	const keys = Object.keys(leftRecord).filter((key) => leftRecord[key] !== undefined);
	const otherKeys = Object.keys(rightRecord).filter((key) => rightRecord[key] !== undefined);
	return (
		keys.length === otherKeys.length &&
		keys.every((key) => key in rightRecord && sameValue(leftRecord[key], rightRecord[key]))
	);
}

/**
 * Presents the requested values of a selection: one shared request reads as that request; any
 * difference in the requested wire values — not merely in their rounded text — reads as Mixed with
 * each distinct request retained. Nothing is averaged. An empty selection presents nothing.
 */
export function presentSelection<V, P>(
	values: readonly V[],
	present: (value: V) => P,
): SelectionPresentation<P> | null {
	if (!values.length) return null;
	const distinct: V[] = [];
	for (const value of values) {
		if (!distinct.some((seen) => sameValue(seen, value))) distinct.push(value);
	}
	if (distinct.length === 1) return present(distinct[0]);
	return { kind: "mixed", count: values.length, distinct: distinct.map(present), text: MIXED_LABEL };
}

export function presentScalarSelection(
	intents: readonly ProgrammingScalarIntent[],
	descriptor: ProgrammingComponentDescriptor | null | undefined,
): SelectionPresentation<ScalarPresentation> | null {
	return presentSelection(intents, (intent) => presentScalar(intent, descriptor));
}

// ---------------------------------------------------------------------------------------------
// Position
// ---------------------------------------------------------------------------------------------

export interface TargetReferenceLabels {
	/** The show's name for a 3D Point, or nothing when the Point is absent from the show. */
	pointLabel: (pointId: string) => string | null | undefined;
	/** Caller-supplied wording for a Point the show cannot resolve, e.g. "Missing point". */
	missingPointLabel: string;
}

export type TargetReferencePresentation =
	| { kind: "origin"; text: string }
	| {
			kind: "point";
			/** The stored UUID, retained whether or not the show resolves it. */
			pointId: string;
			resolved: boolean;
			text: string;
	  };

export type PositionPresentation =
	| { kind: "angles"; text: "Angles"; pan: ScalarPresentation; tilt: ScalarPresentation }
	| {
			kind: "target";
			text: "Target";
			reference: TargetReferencePresentation;
			/** X across stage, Y upstage, Z upward; absolute at Origin, Point-local otherwise. */
			offsets: [ScalarPresentation, ScalarPresentation, ScalarPresentation];
	  };

export function presentTargetReference(
	reference: ProgrammingTargetReference,
	labels: TargetReferenceLabels,
): TargetReferencePresentation {
	if (reference.kind === "origin") return { kind: "origin", text: ORIGIN_LABEL };
	const label = labels.pointLabel(reference.point_id);
	const resolved = typeof label === "string" && label.length > 0;
	return {
		kind: "point",
		pointId: reference.point_id,
		resolved,
		text: resolved ? label : labels.missingPointLabel,
	};
}

const TARGET_AXES = [{ kind: "target_x" }, { kind: "target_y" }, { kind: "target_z" }] as const;

/**
 * Requested Position. Angles stay unwrapped (a −900° request reads −900°, never its modulo). A
 * Target carries only its reference and metre offsets; it has no Pan/Tilt of its own.
 */
export function presentPosition(
	intent: ProgrammingPositionIntent,
	descriptors: ComponentDescriptorLookup,
	labels: TargetReferenceLabels,
): PositionPresentation {
	if (intent.kind === "angles") {
		return {
			kind: "angles",
			text: "Angles",
			pan: presentScalar(intent.pan_degrees, descriptors({ kind: "pan" })),
			tilt: presentScalar(intent.tilt_degrees, descriptors({ kind: "tilt" })),
		};
	}
	const [x, y, z] = intent.offset_metres.map((axis, index) =>
		presentScalar(axis, descriptors(TARGET_AXES[index])),
	);
	return {
		kind: "target",
		text: "Target",
		reference: presentTargetReference(intent.reference, labels),
		offsets: [x, y, z],
	};
}

export type PositionSelectionPresentation =
	| { kind: "mixed_variant"; count: number; text: string }
	| {
			kind: "angles";
			text: "Angles";
			pan: SelectionPresentation<ScalarPresentation>;
			tilt: SelectionPresentation<ScalarPresentation>;
	  }
	| {
			kind: "target";
			text: "Target";
			reference: SelectionPresentation<TargetReferencePresentation>;
			offsets: [
				SelectionPresentation<ScalarPresentation>,
				SelectionPresentation<ScalarPresentation>,
				SelectionPresentation<ScalarPresentation>,
			];
	  };

/**
 * A selection's requested Position, component by component. Angles and Targets are never merged:
 * a selection holding both reads as a mixed variant rather than an averaged or solved pose.
 */
export function presentPositionSelection(
	intents: readonly ProgrammingPositionIntent[],
	descriptors: ComponentDescriptorLookup,
	labels: TargetReferenceLabels,
): PositionSelectionPresentation | null {
	if (!intents.length) return null;
	const angles = intents.filter((intent) => intent.kind === "angles");
	const targets = intents.filter((intent) => intent.kind === "target");
	if (angles.length && targets.length) {
		return { kind: "mixed_variant", count: intents.length, text: MIXED_LABEL };
	}
	if (angles.length) {
		return {
			kind: "angles",
			text: "Angles",
			pan: required(presentScalarSelection(angles.map((intent) => intent.pan_degrees), descriptors({ kind: "pan" }))),
			tilt: required(presentScalarSelection(angles.map((intent) => intent.tilt_degrees), descriptors({ kind: "tilt" }))),
		};
	}
	const axis = (index: 0 | 1 | 2) =>
		required(presentScalarSelection(targets.map((intent) => intent.offset_metres[index]), descriptors(TARGET_AXES[index])));
	return {
		kind: "target",
		text: "Target",
		reference: required(presentSelection(targets.map((intent) => intent.reference), (reference) => presentTargetReference(reference, labels))),
		offsets: [axis(0), axis(1), axis(2)],
	};
}

function required<T>(value: T | null): T {
	if (value === null) throw new Error("A non-empty selection always presents a value");
	return value;
}

// ---------------------------------------------------------------------------------------------
// Focus and Zoom
// ---------------------------------------------------------------------------------------------

/** Focus is requested as a normalized value or spread and reads in its descriptor's percent. */
export function presentFocus(
	value: Extract<ProgrammingAttributeValue, { kind: "normalized" | "spread" }>,
	descriptors: ComponentDescriptorLookup,
): ScalarPresentation {
	const intent: ProgrammingScalarIntent =
		value.kind === "normalized" ? { kind: "value", value: value.value } : { kind: "spread", value: value.value };
	return presentScalar(intent, descriptors({ kind: "focus" }));
}

export const ZOOM_CONVENTION_LABEL: Record<ProgrammingOpeningConvention, string> = {
	beam: "Beam",
	field: "Field",
};

export interface ZoomPresentation {
	/** Full physical opening, in the descriptor's degrees. */
	opening: ScalarPresentation;
	convention: ProgrammingOpeningConvention;
	conventionText: string;
	text: string;
}

export function presentZoom(
	intent: ProgrammingZoomIntent,
	descriptors: ComponentDescriptorLookup,
): ZoomPresentation {
	const opening = presentScalar(intent.opening_degrees, descriptors({ kind: "zoom" }));
	const conventionText = ZOOM_CONVENTION_LABEL[intent.convention];
	return { opening, convention: intent.convention, conventionText, text: `${opening.text} ${conventionText}` };
}

// ---------------------------------------------------------------------------------------------
// Color
// ---------------------------------------------------------------------------------------------

const color = (component: ProgrammingColorComponent): ProgrammingComponent => ({ kind: "color", component });

export interface SemanticColorPresentation {
	kind: "semantic";
	/** Authoritative requested base XYZ, retained exactly. Zero is black, not D65 white. */
	baseXyz: ProgrammingColorXyz;
	recipe: {
		red: ScalarPresentation;
		green: ScalarPresentation;
		blue: ScalarPresentation;
		amber: ScalarPresentation;
		/** Advanced coordinates are retained while Easy controls show an approximation. */
		approximate: boolean;
	};
	/** Present only when the request spreads hue or saturation across the selection. */
	hue: ScalarPresentation | null;
	saturation: ScalarPresentation | null;
	whiteBlend: ScalarPresentation;
	temperature: ScalarPresentation;
	duv: ScalarPresentation;
	/** Explicit UV request, independent of visible colour. */
	uv: ScalarPresentation;
	relativeOutput: ScalarPresentation;
	allocation: ProgrammingColorIntent["allocation"];
	wheelConstraints: NonNullable<ProgrammingColorIntent["wheel_constraints"]>;
	/**
	 * The request itself asks for no visible light: zero relative output, or a black base with no
	 * White Blend, with no visible-colour spread. A requested property, not a published result.
	 */
	requestedVisibleBlack: boolean;
	/** Visible black with a non-zero UV request: a valid UV-only look. */
	uvOnly: boolean;
}

export type PortableAppearancePresentation =
	| {
			kind: "known";
			xyz: ProgrammingColorXyz;
			relativeOutput: PresentedNumber;
			/** A supported measured/estimated zero is black, distinct from unknown. */
			black: boolean;
	  }
	| { kind: "unknown"; text: string };

export type PortableUvPresentation =
	| { kind: "known"; amount: PresentedNumber; quality: ProgrammingPhysicalDataQuality }
	| { kind: "unknown"; text: string };

export interface DirectChannelPresentation {
	channelId: string;
	functionId: string;
	/** The exact native integer, or its ordered spread endpoints. */
	value: ScalarPresentation;
}

export interface DirectColorPresentation {
	kind: "direct";
	/** Pinned source identity; a Direct recipe is only meaningful against this exact source. */
	source: ProgrammingNativeColorIdentity;
	channels: DirectChannelPresentation[];
	portable: {
		modelRevision: number;
		visible: PortableAppearancePresentation;
		uv: PortableUvPresentation;
		quality: ProgrammingPhysicalDataQuality;
		limitations: string[];
	};
}

export type ColorPresentation = SemanticColorPresentation | DirectColorPresentation;

export interface ColorPresentationOptions {
	/** Caller-supplied wording for an unknown portable estimate, e.g. "Unknown appearance". */
	unknownAppearanceLabel: string;
}

const VISIBLE_SPREAD_COMPONENTS: readonly ProgrammingColorComponent[] = [
	"red",
	"green",
	"blue",
	"amber",
	"hue",
	"saturation",
	"white_blend",
	"temperature",
	"duv",
	"relative_output",
];

function presentSemanticColor(
	intent: ProgrammingColorIntent,
	descriptors: ComponentDescriptorLookup,
): SemanticColorPresentation {
	const spreads = new Map((intent.spreads ?? []).map((spread) => [spread.component, spread.points]));
	const component = (name: ProgrammingColorComponent, value: number | null): ScalarPresentation | null => {
		const points = spreads.get(name);
		const intentValue: ProgrammingScalarIntent | null = points
			? { kind: "spread", value: points }
			: value === null
				? null
				: { kind: "value", value };
		return intentValue && presentScalar(intentValue, descriptors(color(name)));
	};
	const scalar = (name: ProgrammingColorComponent, value: number) => component(name, value) as ScalarPresentation;
	const [red, green, blue] = intent.recipe.rgb;
	const { x, y, z } = intent.base_xyz;
	const visibleSpread = VISIBLE_SPREAD_COMPONENTS.some((name) => spreads.has(name));
	const requestedVisibleBlack =
		!visibleSpread &&
		(intent.relative_output === 0 || (x === 0 && y === 0 && z === 0 && intent.white_blend === 0));
	const uvRequested = spreads.has("uv") ? (spreads.get("uv") ?? []).some((point) => point > 0) : intent.uv.amount > 0;
	return {
		kind: "semantic",
		baseXyz: { ...intent.base_xyz },
		recipe: {
			red: scalar("red", red),
			green: scalar("green", green),
			blue: scalar("blue", blue),
			amber: scalar("amber", intent.recipe.amber),
			approximate: intent.recipe.approximate,
		},
		hue: component("hue", null),
		saturation: component("saturation", null),
		whiteBlend: scalar("white_blend", intent.white_blend),
		temperature: scalar("temperature", intent.white_target.kelvin),
		duv: scalar("duv", intent.white_target.duv),
		uv: scalar("uv", intent.uv.amount),
		relativeOutput: scalar("relative_output", intent.relative_output),
		allocation: intent.allocation,
		wheelConstraints: [...(intent.wheel_constraints ?? [])],
		requestedVisibleBlack,
		uvOnly: requestedVisibleBlack && uvRequested,
	};
}

function presentDirectChannels(
	recipe: ProgrammingNativeColorRecipe,
	descriptors: ComponentDescriptorLookup,
): DirectChannelPresentation[] {
	return recipe.channels.map((channel) => {
		const spread = recipe.spreads?.find(
			(item) => item.binding.channel_id === channel.channel_id && item.binding.function_id === channel.function_id,
		);
		const descriptor = descriptors({
			kind: "native_color",
			component: { channel_id: channel.channel_id, function_id: channel.function_id },
		});
		return {
			channelId: channel.channel_id,
			functionId: channel.function_id,
			value: presentScalar(
				spread ? { kind: "spread", value: spread.points } : { kind: "value", value: channel.raw },
				// Native values are exact integers; never let a lookup turn them into percentages.
				descriptor && descriptor.unit === "native_integer" ? descriptor : null,
			),
		};
	});
}

function presentPortable(
	portable: ProgrammingPortableColorEstimate,
	descriptors: ComponentDescriptorLookup,
	options: ColorPresentationOptions,
): DirectColorPresentation["portable"] {
	const visible: PortableAppearancePresentation = portable.visible
		? {
				kind: "known",
				xyz: { ...portable.visible.xyz },
				relativeOutput: presentNumber(portable.visible.relative_output, descriptors(color("relative_output"))),
				black:
					portable.visible.relative_output === 0 ||
					(portable.visible.xyz.x === 0 && portable.visible.xyz.y === 0 && portable.visible.xyz.z === 0),
			}
		: { kind: "unknown", text: options.unknownAppearanceLabel };
	const uv: PortableUvPresentation = portable.uv
		? { kind: "known", amount: presentNumber(portable.uv.amount, descriptors(color("uv"))), quality: portable.uv.quality }
		: { kind: "unknown", text: options.unknownAppearanceLabel };
	return {
		modelRevision: portable.model_revision,
		visible,
		uv,
		quality: portable.quality,
		limitations: [...portable.limitations],
	};
}

/**
 * Requested Color. Semantic requests read their recipe, white target (Kelvin and signed Duv), White
 * Blend, explicit UV and relative output; Direct requests keep their pinned source, exact native
 * integers and an explicitly unknown portable estimate where the model has none.
 */
export function presentColorProgram(
	program: ProgrammingColorProgram,
	descriptors: ComponentDescriptorLookup,
	options: ColorPresentationOptions,
): ColorPresentation {
	if (program.kind === "semantic") return presentSemanticColor(program.intent, descriptors);
	return {
		kind: "direct",
		source: { ...program.recipe.source },
		channels: presentDirectChannels(program.recipe, descriptors),
		portable: presentPortable(program.portable, descriptors, options),
	};
}
