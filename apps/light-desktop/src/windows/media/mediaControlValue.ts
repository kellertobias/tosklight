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

export function isMediaPercentAttribute(attribute: string) {
	return PERCENT_ATTRIBUTES.has(attribute);
}

export function mediaControlDefaultNormalized(attribute: string) {
	if (
		LAYER_SCALE_ATTRIBUTES.has(attribute) ||
		MASK_SCALE_ATTRIBUTES.has(attribute) ||
		POSITION_ATTRIBUTES.has(attribute) ||
		attribute === "media.layer.rotation" ||
		attribute === "position.rotation"
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
) {
	const value = Math.max(0, Math.min(1, normalized));
	if (isMediaPercentAttribute(attribute)) return value * 100;
	if (LAYER_SCALE_ATTRIBUTES.has(attribute))
		return value <= 0.5 ? value * 2 : 1 + (value - 0.5) * 18;
	if (MASK_SCALE_ATTRIBUTES.has(attribute))
		return value <= 0.5 ? value * 2 : 1 + (value - 0.5) * 2;
	if (POSITION_ATTRIBUTES.has(attribute)) return value * 4 - 2;
	if (attribute === "media.layer.rotation" || attribute === "position.rotation")
		return value * 720 - 360;
	return Math.round(value * 255);
}

export function mediaControlNormalizedValue(attribute: string, value: number) {
	if (isMediaPercentAttribute(attribute))
		return Math.max(0, Math.min(100, value)) / 100;
	if (LAYER_SCALE_ATTRIBUTES.has(attribute)) {
		const scale = Math.max(0, Math.min(10, value));
		return scale <= 1 ? scale / 2 : 0.5 + (scale - 1) / 18;
	}
	if (MASK_SCALE_ATTRIBUTES.has(attribute)) {
		const scale = Math.max(0, Math.min(2, value));
		return scale <= 1 ? scale / 2 : 0.5 + (scale - 1) / 2;
	}
	if (POSITION_ATTRIBUTES.has(attribute))
		return (Math.max(-2, Math.min(2, value)) + 2) / 4;
	if (attribute === "media.layer.rotation" || attribute === "position.rotation")
		return (Math.max(-360, Math.min(360, value)) + 360) / 720;
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
