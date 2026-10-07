import type {
	AttributeDescriptor,
	DynamicValueAddressProjection,
} from "../../api/types";

type ProgrammingColorComponent = Extract<
	NonNullable<DynamicValueAddressProjection["component"]>,
	{ kind: "color" }
>["component"];

/**
 * What one Dynamic lane animates and in which units the editor shows it (TL-648).
 *
 * Programming contract 1 stores Position, Color and Zoom as fixture-independent families, so a
 * lane on one of them is a typed Programming lane: Pan and Tilt are Angles in degrees, Zoom an
 * opening in degrees, Color a semantic recipe/hue/orthogonal component. Every other continuous
 * attribute (Intensity, Focus, Beam, …) keeps a scalar 0–1 lane. Graphs, source encoders,
 * amplitude and preview all read the same descriptor, so a degree is never shown as a percentage.
 */
export interface DynamicLaneDomain {
	/** Stable lane-chooser identity; for a scalar lane it is the attribute itself. */
	key: string;
	label: string;
	/** Registry family the lane chooser groups it under. */
	family: string;
	unit: "percent" | "degrees" | "kelvin";
	/** Encoder bounds and the graph's vertical extent, in descriptor units. */
	minimum: number;
	maximum: number;
	/** Value-modal factor: 100 for a percentage typed as 0–100. */
	inputScale: number;
	fineStep: number;
	coarseStep: number;
	/** The typed Programming address, or null for a scalar lane. */
	address: DynamicValueAddressProjection | null;
	/** Programming key a Preset stores this lane's family under. */
	presetKey: string;
	defaults: {
		method: "max_min" | "middle_amplitude";
		minimum: number;
		maximum: number;
		middle: number | "current";
		amplitude: number;
	};
}

export type DynamicLaneChoice = Pick<
	DynamicLaneDomain,
	"key" | "label" | "family"
> & { id: string };

const PERCENT = {
	unit: "percent",
	minimum: 0,
	maximum: 1,
	inputScale: 100,
	fineStep: 0.01,
	coarseStep: 0.1,
} as const;

const angle = (
	component: "pan" | "tilt",
	label: string,
	travel: number,
	amplitude: number,
): DynamicLaneDomain => ({
	key: `position.${component}`,
	label,
	family: "position",
	unit: "degrees",
	minimum: -travel,
	maximum: travel,
	inputScale: 1,
	fineStep: 1,
	coarseStep: 10,
	address: { representation: { kind: "angles" }, component: { kind: component } },
	presetKey: "position",
	defaults: {
		method: "middle_amplitude",
		minimum: -amplitude,
		maximum: amplitude,
		middle: "current",
		amplitude,
	},
});

const color = (
	component: ProgrammingColorComponent,
	label: string,
	basis: "recipe" | "hue_saturation" | "retain",
	shape: Pick<
		DynamicLaneDomain,
		"unit" | "minimum" | "maximum" | "inputScale" | "fineStep" | "coarseStep"
	> = PERCENT,
	defaults: Partial<DynamicLaneDomain["defaults"]> = {},
): DynamicLaneDomain => ({
	key: `color.${component}`,
	label,
	family: "color",
	...shape,
	address: {
		representation: { kind: "semantic_color", basis },
		component: { kind: "color", component },
	},
	presetKey: "color",
	defaults: {
		method: "max_min",
		minimum: shape.minimum,
		maximum: shape.maximum,
		middle: (shape.minimum + shape.maximum) / 2,
		amplitude: (shape.maximum - shape.minimum) / 2,
		...defaults,
	},
});

/** Pan and Tilt span the nominal ±270° / ±180° travel of a moving head. */
const POSITION_DOMAINS = [
	angle("pan", "Pan", 270, 45),
	angle("tilt", "Tilt", 180, 30),
];
const COLOR_DOMAINS = [
	color("red", "Red", "recipe"),
	color("green", "Green", "recipe"),
	color("blue", "Blue", "recipe"),
	color("amber", "Amber", "recipe"),
	color(
		"hue",
		"Hue",
		"hue_saturation",
		{
			unit: "degrees",
			minimum: 0,
			maximum: 359,
			inputScale: 1,
			fineStep: 1,
			coarseStep: 10,
		},
		{ amplitude: 60 },
	),
	color("saturation", "Saturation", "hue_saturation"),
	color("white_blend", "White Blend", "retain"),
	color(
		"temperature",
		"Temperature",
		"retain",
		{
			unit: "kelvin",
			minimum: 1800,
			maximum: 10_000,
			inputScale: 1,
			fineStep: 10,
			coarseStep: 100,
		},
		{ minimum: 2700, maximum: 6500, middle: 4600, amplitude: 1900 },
	),
	color("uv", "UV", "retain"),
];
const ZOOM_DOMAIN: DynamicLaneDomain = {
	key: "zoom",
	label: "Zoom",
	family: "focus",
	unit: "degrees",
	minimum: 0,
	maximum: 180,
	inputScale: 1,
	fineStep: 1,
	coarseStep: 10,
	address: {
		representation: { kind: "zoom", convention: "beam" },
		component: { kind: "zoom" },
	},
	presetKey: "zoom",
	defaults: {
		method: "max_min",
		minimum: 10,
		maximum: 40,
		middle: "current",
		amplitude: 10,
	},
};
const TYPED_DOMAINS = new Map(
	[...POSITION_DOMAINS, ...COLOR_DOMAINS, ZOOM_DOMAIN].map((domain) => [
		domain.key,
		domain,
	]),
);

/** Fixture-facing addresses contract 1 refuses as scalar lanes; their family replaces them. */
function legacyFamily(id: string): "position" | "color" | "zoom" | null {
	if (["pan", "tilt", "pan.continuous", "tilt.continuous", "position"].includes(id))
		return "position";
	if (id === "zoom") return "zoom";
	if (id === "color") return "color";
	if (id.startsWith("color.") && id !== "color.tint" && !id.startsWith("color.wheel"))
		return "color";
	return null;
}

function scalarDomain(key: string, label = key, family = ""): DynamicLaneDomain {
	return {
		key,
		label,
		family,
		...PERCENT,
		address: null,
		presetKey: key,
		defaults: {
			method: "max_min",
			minimum: 0,
			maximum: 1,
			middle: 0.5,
			amplitude: 0.5,
		},
	};
}

/** The descriptor for a lane-chooser key: typed family lanes, else a scalar 0–1 attribute. */
export function laneDomainForKey(key: string): DynamicLaneDomain {
	return TYPED_DOMAINS.get(key) ?? scalarDomain(key);
}

/** The descriptor for a typed lane address, or null when the editor cannot compose it. */
export function laneDomainForAddress(
	address: DynamicValueAddressProjection,
): DynamicLaneDomain | null {
	const component = address.component;
	if (!component) return null;
	switch (component.kind) {
		case "pan":
		case "tilt":
			return address.representation.kind === "angles"
				? laneDomainForKey(`position.${component.kind}`)
				: null;
		case "zoom":
			return address.representation.kind === "zoom"
				? { ...ZOOM_DOMAIN, address }
				: null;
		case "color":
			return address.representation.kind === "semantic_color"
				? (TYPED_DOMAINS.get(`color.${component.component}`) ?? null)
				: null;
		default:
			return null;
	}
}

function isScalarLaneAttribute(attribute: AttributeDescriptor) {
	return (
		attribute.recordable &&
		attribute.value_type === "continuous" &&
		attribute.normalized_min != null &&
		attribute.normalized_max != null
	);
}

/**
 * The lane chooser in registry order. Position, Color and Zoom appear once, where their first
 * fixture-facing attribute sits, as their fixture-independent components.
 */
export function dynamicLaneChoices(
	registry: readonly AttributeDescriptor[],
): DynamicLaneChoice[] {
	const choices: DynamicLaneChoice[] = [];
	const emitted = new Set<string>();
	const emit = (domains: readonly DynamicLaneDomain[], family: string) => {
		for (const domain of domains)
			choices.push({ id: domain.key, key: domain.key, label: domain.label, family });
	};
	for (const attribute of registry) {
		const family = legacyFamily(attribute.id);
		if (family) {
			if (emitted.has(family)) continue;
			emitted.add(family);
			if (family === "position") emit(POSITION_DOMAINS, attribute.family);
			else if (family === "color") emit(COLOR_DOMAINS, attribute.family);
			else emit([ZOOM_DOMAIN], attribute.family);
			continue;
		}
		if (isScalarLaneAttribute(attribute))
			choices.push({
				id: attribute.id,
				key: attribute.id,
				label: attribute.label,
				family: attribute.family,
			});
	}
	return choices;
}

/** A value in descriptor units, as the encoders and keyframe chips show it. */
export function formatLaneValue(domain: DynamicLaneDomain, value: number) {
	switch (domain.unit) {
		case "percent":
			return `${Math.round(value * 100)}%`;
		case "degrees":
			return `${Math.round(value * 10) / 10}°`;
		case "kelvin":
			return `${Math.round(value)} K`;
	}
}

/** Position on the graph's vertical axis, 0 at the domain minimum and 1 at its maximum. */
export function laneGraphFraction(domain: DynamicLaneDomain, value: number) {
	return (value - domain.minimum) / Math.max(1e-9, domain.maximum - domain.minimum);
}
