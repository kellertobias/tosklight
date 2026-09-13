const PERCENT_ATTRIBUTES = new Set([
	"media.layer.dimmer",
	"media.layer.volume",
	"media.layer.grayscale",
	"media.layer.mask.opacity",
	"media.layer.effect.1",
	"media.layer.effect.2",
	"media.layer.effect.3",
	"media.layer.effect.4",
	"intensity",
	"volume",
	"media.grayscale",
	"media.mask.opacity",
	"media.effect.1",
	"media.effect.2",
	"media.effect.3",
	"media.effect.4",
	"media.effect.bank.1.strength",
	"media.effect.bank.2.strength",
	"media.layer.playback.blur",
	"media.playback.blur",
]);

const LAYER_SCALE_ATTRIBUTES = new Set([
	"media.layer.scale.x",
	"media.layer.scale.y",
	"media.scale.x",
	"media.scale.y",
]);

const MASK_SCALE_ATTRIBUTES = new Set([
	"media.layer.mask.scale.x",
	"media.layer.mask.scale.y",
	"media.mask.scale.x",
	"media.mask.scale.y",
]);

const POSITION_ATTRIBUTES = new Set([
	"media.layer.position.x",
	"media.layer.position.y",
	"media.layer.mask.position.x",
	"media.layer.mask.position.y",
	"media.position.x",
	"media.position.y",
	"media.mask.position.x",
	"media.mask.position.y",
]);

const SHAPER_POSITION_ATTRIBUTES = new Set([
	"shaper.blade.1.position",
	"shaper.blade.2.position",
	"shaper.blade.3.position",
	"shaper.blade.4.position",
]);

const SHAPER_ANGLE_ATTRIBUTES = new Set([
	"shaper.blade.1.angle",
	"shaper.blade.2.angle",
	"shaper.blade.3.angle",
	"shaper.blade.4.angle",
]);

/** 16-bit frame counts: the In point from the clip's start, the Out point back from its end. */
const FRAME_ATTRIBUTES = new Set(["media.in_point", "media.out_point"]);

/**
 * A layer's 3D model pan and tilt: 16-bit, centred, −360° to 360°. They are media attributes, not
 * the moving-light `pan`/`tilt`, so Aim and position presets never turn a media layer.
 */
const MODEL_ANGLE_ATTRIBUTES = new Set(["media.model.pan", "media.model.tilt"]);

const SIXTEEN_BIT_MAXIMUM = 65535;
/** The signed master scale's raw zero (0×) and its step per 1×. */
const SIGNED_SCALE_ZERO = 32768;
const SIGNED_SCALE_UNIT = 8192;
/** The signed master scale's 1× home: raw 40960. */
export const SIGNED_MASTER_SCALE_HOME_NORMALIZED = 40960 / SIXTEEN_BIT_MAXIMUM;

export function isMediaPercentAttribute(attribute: string) {
	return PERCENT_ATTRIBUTES.has(attribute);
}

export function isMediaFrameAttribute(attribute: string) {
	return FRAME_ATTRIBUTES.has(attribute);
}

export function isMediaModelAngleAttribute(attribute: string) {
	return MODEL_ANGLE_ATTRIBUTES.has(attribute);
}

/**
 * The mapping Master drops Flip/mirror and makes its scale signed (negative mirrors), so a Master
 * that reports its attributes without Flip/mirror uses the signed scale. A Master that reports no
 * attributes keeps the older unsigned scale.
 */
export function mediaMasterScaleIsSigned(
	masterAttributes: readonly string[] | undefined,
) {
	return Boolean(
		masterAttributes?.length && !masterAttributes.includes("media.flip_mirror"),
	);
}

export function mediaControlDefaultNormalized(
	attribute: string,
	signedMasterScale = false,
) {
	if (signedMasterScale && LAYER_SCALE_ATTRIBUTES.has(attribute))
		return SIGNED_MASTER_SCALE_HOME_NORMALIZED;
	if (
		MODEL_ANGLE_ATTRIBUTES.has(attribute) ||
		LAYER_SCALE_ATTRIBUTES.has(attribute) ||
		MASK_SCALE_ATTRIBUTES.has(attribute) ||
		POSITION_ATTRIBUTES.has(attribute) ||
		attribute === "media.layer.rotation" ||
		attribute === "position.rotation" ||
		attribute === "shaper.rotation" ||
		SHAPER_ANGLE_ATTRIBUTES.has(attribute)
	)
		return 0.5;
	if (
		attribute === "media.layer.dimmer" ||
		attribute === "media.layer.volume" ||
		attribute === "volume"
	)
		return 1;
	if (
		attribute === "media.layer.speed.multiplier" ||
		attribute === "media.playback_speed"
	)
		return 127 / 255;
	return 0;
}

export function mediaControlOperatorValue(
	attribute: string,
	normalized: number,
	master = false,
	signedMasterScale = false,
) {
	const value = Math.max(0, Math.min(1, normalized));
	if (isMediaPercentAttribute(attribute)) return value * 100;
	if (FRAME_ATTRIBUTES.has(attribute))
		return Math.round(value * SIXTEEN_BIT_MAXIMUM);
	if (MODEL_ANGLE_ATTRIBUTES.has(attribute)) return value * 720 - 360;
	if (master && signedMasterScale && LAYER_SCALE_ATTRIBUTES.has(attribute))
		return (value * SIXTEEN_BIT_MAXIMUM - SIGNED_SCALE_ZERO) / SIGNED_SCALE_UNIT;
	if (LAYER_SCALE_ATTRIBUTES.has(attribute))
		return value <= 0.5 ? value * 2 : 1 + (value - 0.5) * (master ? 6 : 18);
	if (MASK_SCALE_ATTRIBUTES.has(attribute))
		return value <= 0.5 ? value * 2 : 1 + (value - 0.5) * 2;
	if (POSITION_ATTRIBUTES.has(attribute)) return value * 4 - 2;
	if (attribute === "media.layer.rotation" || attribute === "position.rotation")
		return master ? value * 360 - 180 : value * 720 - 360;
	if (SHAPER_POSITION_ATTRIBUTES.has(attribute)) return value * 100;
	if (SHAPER_ANGLE_ATTRIBUTES.has(attribute)) return value * 90 - 45;
	if (attribute === "shaper.rotation") return value * 360 - 180;
	return Math.round(value * 255);
}

export function mediaControlNormalizedValue(
	attribute: string,
	value: number,
	master = false,
	signedMasterScale = false,
) {
	if (isMediaPercentAttribute(attribute))
		return Math.max(0, Math.min(100, value)) / 100;
	if (FRAME_ATTRIBUTES.has(attribute))
		return (
			Math.max(0, Math.min(SIXTEEN_BIT_MAXIMUM, Math.round(value))) /
			SIXTEEN_BIT_MAXIMUM
		);
	if (MODEL_ANGLE_ATTRIBUTES.has(attribute))
		return (Math.max(-360, Math.min(360, value)) + 360) / 720;
	if (master && signedMasterScale && LAYER_SCALE_ATTRIBUTES.has(attribute)) {
		const raw = value * SIGNED_SCALE_UNIT + SIGNED_SCALE_ZERO;
		return Math.max(0, Math.min(SIXTEEN_BIT_MAXIMUM, raw)) / SIXTEEN_BIT_MAXIMUM;
	}
	if (LAYER_SCALE_ATTRIBUTES.has(attribute)) {
		const maximum = master ? 4 : 10;
		const scale = Math.max(0, Math.min(maximum, value));
		return scale <= 1 ? scale / 2 : 0.5 + (scale - 1) / (master ? 6 : 18);
	}
	if (MASK_SCALE_ATTRIBUTES.has(attribute)) {
		const scale = Math.max(0, Math.min(2, value));
		return scale <= 1 ? scale / 2 : 0.5 + (scale - 1) / 2;
	}
	if (POSITION_ATTRIBUTES.has(attribute))
		return (Math.max(-2, Math.min(2, value)) + 2) / 4;
	if (
		attribute === "media.layer.rotation" ||
		attribute === "position.rotation"
	) {
		const limit = master ? 180 : 360;
		return (Math.max(-limit, Math.min(limit, value)) + limit) / (limit * 2);
	}
	if (SHAPER_POSITION_ATTRIBUTES.has(attribute))
		return Math.max(0, Math.min(100, value)) / 100;
	if (SHAPER_ANGLE_ATTRIBUTES.has(attribute))
		return (Math.max(-45, Math.min(45, value)) + 45) / 90;
	if (attribute === "shaper.rotation")
		return (Math.max(-180, Math.min(180, value)) + 180) / 360;
	return Math.max(0, Math.min(255, value)) / 255;
}

export function mediaRgbFromCmy(cyan: number, magenta: number, yellow: number) {
	return `#${[cyan, magenta, yellow]
		.map((component) =>
			Math.round((1 - Math.max(0, Math.min(1, component))) * 255)
				.toString(16)
				.padStart(2, "0"),
		)
		.join("")}`;
}

export function mediaCmyFromRgb(value: string) {
	const match = /^#([0-9a-f]{6})$/iu.exec(value);
	if (!match) return null;
	const packed = match[1];
	return [0, 2, 4].map(
		(offset) => 1 - Number.parseInt(packed.slice(offset, offset + 2), 16) / 255,
	) as [number, number, number];
}
