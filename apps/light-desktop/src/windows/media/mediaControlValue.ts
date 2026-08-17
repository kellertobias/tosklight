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

export function isMediaPercentAttribute(attribute: string) {
	return PERCENT_ATTRIBUTES.has(attribute);
}

export function mediaControlDefaultNormalized(attribute: string) {
	if (
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
) {
	const value = Math.max(0, Math.min(1, normalized));
	if (isMediaPercentAttribute(attribute)) return value * 100;
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
) {
	if (isMediaPercentAttribute(attribute))
		return Math.max(0, Math.min(100, value)) / 100;
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
