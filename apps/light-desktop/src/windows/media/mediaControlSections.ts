import type { BuildMediaPaneModelInput } from "./buildMediaPaneModel";
import {
	LAYOUT_SPECIFIC_ATTRIBUTES,
	MASTER_CONTROL_GROUPS,
	MEDIA_CONTROL_GROUPS,
	mediaControlLabel,
} from "./mediaControlGroups";
import {
	isMediaFrameAttribute,
	isMediaModelAngleAttribute,
	isMediaPercentAttribute,
	mediaControlDefaultNormalized,
	mediaControlOperatorValue,
	mediaMasterScaleIsSigned,
} from "./mediaControlValue";
import type {
	MediaControlSection,
	MediaPointFrameRate,
} from "./mediaPaneModel";
import { pointDisplay } from "./mediaPointTime";
import {
	normalizedValue,
	specializedControl,
} from "./specializedMediaControls";

/**
 * The attributes the addressed head actually carries. An unknown set (a personality that does not
 * report one) keeps every control enabled rather than greying out a working desk.
 */
function ownedAttributes(
	input: BuildMediaPaneModelInput,
	selectedMaster: boolean,
): ReadonlySet<string> | null {
	const attributes = selectedMaster
		? input.selectedServer?.master_attributes
		: input.selectedServer?.layers.find(
				(layer) => layer.fixture_id === input.selectedLayerId,
			)?.attributes;
	return attributes?.length ? new Set(attributes) : null;
}

const COLOUR_COMPONENTS = [
	"color.tint",
	"color.red",
	"color.green",
	"color.blue",
	"color.cyan",
	"color.magenta",
	"color.yellow",
];

function attributeIsOwned(
	owned: ReadonlySet<string> | null,
	attribute: string,
): boolean {
	if (!owned) return true;
	if (attribute === "color.tint")
		return COLOUR_COMPONENTS.some((component) => owned.has(component));
	return owned.has(attribute);
}

export function controlSections(
	input: BuildMediaPaneModelInput,
	controls: Array<{ attribute: string }>,
): MediaControlSection[] {
	const selectedMaster = input.selectedLayerId === "master";
	const selectedLayer = input.selectedServer?.layers.some(
		(layer) => layer.fixture_id === input.selectedLayerId,
	);
	if (!selectedLayer && !selectedMaster) return [];
	const groups = selectedMaster ? MASTER_CONTROL_GROUPS : MEDIA_CONTROL_GROUPS;
	const standardAttributes = new Set<string>(
		groups.flatMap((group) => [...group.attributes]),
	);
	for (const component of ["red", "green", "blue", "cyan", "magenta", "yellow"])
		standardAttributes.add(`color.${component}`);
	const remaining = new Map(
		(selectedMaster ? [] : controls)
			.filter((control) => !standardAttributes.has(control.attribute))
			.map((control) => [control.attribute, control]),
	);
	const owned = ownedAttributes(input, selectedMaster);
	const signedMasterScale =
		selectedMaster &&
		mediaMasterScaleIsSigned(input.selectedServer?.master_attributes);
	const sections: MediaControlSection[] = groups
		.map((group) => ({
			id: group.id,
			label: group.label,
			capability: "supported" as const,
			controls: group.attributes
				// A control only one channel layout carries is left out for a head that reports it
				// does not carry it, rather than shown as a dead control.
				.filter(
					(attribute) =>
						!LAYOUT_SPECIFIC_ATTRIBUTES.has(attribute) ||
						attributeIsOwned(owned, attribute),
				)
				.map((attribute) => ({
					...advertisedControl(input, attribute, signedMasterScale),
					disabled: !attributeIsOwned(owned, attribute),
				})),
		}))
		.filter((section) => section.controls.length > 0);
	if (remaining.size) {
		sections.push({
			id: "other",
			label: "Other",
			capability: "supported",
			controls: [...remaining.values()].map((control) =>
				advertisedControl(input, control.attribute),
			),
		});
	}
	return sections;
}

function advertisedControl(
	input: BuildMediaPaneModelInput,
	attribute: string,
	signedMasterScale = false,
): MediaControlSection["controls"][number] {
	const normalized =
		normalizedValue(input.liveProgrammer, attribute) ??
		(attribute === "intensity"
			? input.selectedLayerId === "master"
				? 1
				: 0
			: mediaControlDefaultNormalized(attribute, signedMasterScale));
	const rawValue = Math.round(normalized * 255);
	const specialized = specializedControl(
		input,
		attribute,
		normalized,
		rawValue,
	);
	if (specialized) return specialized;
	const selectedMaster = input.selectedLayerId === "master";
	const value = mediaControlOperatorValue(
		attribute,
		normalized,
		selectedMaster,
		signedMasterScale,
	);
	if (isMediaPercentAttribute(attribute)) {
		const percent = Math.round(value);
		return {
			id: attribute,
			label: mediaControlLabel(attribute),
			kind: "value",
			value: percent,
			minimum: 0,
			maximum: 100,
			step: 1,
			display: `${percent}%`,
		};
	}
	if (isMediaFrameAttribute(attribute))
		return pointTimeControl(
			attribute,
			value,
			input.pointFrameRate,
			input.retryPointFrameRate,
		);
	return rangedMediaControl(
		attribute,
		value,
		selectedMaster,
		signedMasterScale,
	);
}

/** An In or Out point in `mm:ss.ff` at the server's rate, or in frames with a notice without one. */
function pointTimeControl(
	attribute: string,
	frames: number,
	rate: MediaPointFrameRate | undefined,
	retry: (() => void) | undefined,
): MediaControlSection["controls"][number] {
	const reference = attribute === "media.out_point" ? "end" : "start";
	const framesPerSecond = rate?.kind === "known" ? rate.framesPerSecond : null;
	return {
		id: attribute,
		label: mediaControlLabel(attribute),
		kind: "point-time",
		value: frames,
		reference,
		framesPerSecond,
		display: pointDisplay(reference, frames, framesPerSecond),
		onRetryFrameRate:
			rate?.kind === "unknown" && rate.retryable ? retry : undefined,
		rateNotice:
			rate?.kind === "known"
				? undefined
				: rate?.kind === "loading"
					? "Reading the Media Server's frame rate…"
					: `Frame rate unknown: ${rate?.detail ?? "this Media Server does not report the rate its In and Out points count in."} Points are entered as frame counts until it is known.`,
	};
}

function rangedMediaControl(
	attribute: string,
	value: number,
	selectedMaster: boolean,
	signedMasterScale = false,
): MediaControlSection["controls"][number] {
	if (attribute === "media.scale.x" || attribute === "media.scale.y")
		return valueControl(
			attribute,
			value,
			// The mapping Master's scale is signed; a negative axis mirrors the output.
			selectedMaster && signedMasterScale ? -4 : 0,
			selectedMaster ? 4 : 10,
			0.01,
			`${value.toFixed(2)}×`,
		);
	if (isMediaModelAngleAttribute(attribute))
		return valueControl(
			attribute,
			value,
			-360,
			360,
			1,
			`${Math.round(value)}°`,
		);
	if (attribute === "media.mask.scale.x" || attribute === "media.mask.scale.y")
		return valueControl(attribute, value, 0, 2, 0.01, `${value.toFixed(2)}×`);
	if (
		attribute === "media.position.x" ||
		attribute === "media.position.y" ||
		attribute === "media.mask.position.x" ||
		attribute === "media.mask.position.y"
	)
		return valueControl(attribute, value, -2, 2, 0.01, value.toFixed(2));
	if (attribute === "position.rotation" || attribute === "shaper.rotation")
		return valueControl(
			attribute,
			value,
			selectedMaster ? -180 : -360,
			selectedMaster ? 180 : 360,
			1,
			`${Math.round(value)}°`,
		);
	if (/^shaper\.blade\.[1-4]\.position$/u.test(attribute))
		return valueControl(attribute, value, 0, 100, 1, `${Math.round(value)}%`);
	if (/^shaper\.blade\.[1-4]\.angle$/u.test(attribute))
		return valueControl(attribute, value, -45, 45, 1, `${Math.round(value)}°`);
	return {
		id: attribute,
		label: mediaControlLabel(attribute),
		kind: "value",
		value,
		minimum: 0,
		maximum: 255,
		step: 1,
	};
}

function valueControl(
	attribute: string,
	value: number,
	minimum: number,
	maximum: number,
	step: number,
	display: string,
): MediaControlSection["controls"][number] {
	return {
		id: attribute,
		label: mediaControlLabel(attribute),
		kind: "value",
		value,
		minimum,
		maximum,
		step,
		display,
	};
}
