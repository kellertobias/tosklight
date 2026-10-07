import type { StoredPreset } from "../../api/types";
import type { AttributeValue } from "../../api/types/playback";
import { normalizePresetFamily } from "../../presetFamilies";
import { colorValueColors, type PreviewColor } from "./colorDisplay";
import {
	dominantSpace,
	type PositionPoint,
	positionValuePoints,
	type PreviewDot,
	previewDots,
	representativePoints,
} from "./positionDots";

/** More segments than this stop reading as distinct colours on a pool tile. */
export const MAX_COLOR_SEGMENTS = 6;

export type PresetPreview =
	| {
			kind: "color";
			/** Distinct display colours, at most MAX_COLOR_SEGMENTS, in hue order. */
			colors: PreviewColor[];
			/** How many distinct colours the preset holds, shown or not. */
			distinct: number;
	  }
	| {
			kind: "position";
			space: PositionPoint["space"];
			dots: PreviewDot[];
			/** How many distinct aims the preset holds, shown or not. */
			distinct: number;
	  };

type PresetBody = Pick<StoredPreset, "family" | "values" | "group_values" | "universal_values">;
type GroupMembers = ReadonlyMap<string, readonly string[]>;
type Owner = "color" | "position";

const OWNER_KINDS: Record<Owner, ReadonlySet<string>> = {
	color: new Set(["color_program", "color_xyz"]),
	position: new Set(["position"]),
};

function intentValue(raw: unknown, owner: Owner): AttributeValue | null {
	if (!raw || typeof raw !== "object") return null;
	const { kind, value } = raw as { kind?: unknown; value?: unknown };
	if (typeof kind !== "string" || !value || typeof value !== "object") return null;
	if (OWNER_KINDS[owner].has(kind)) return raw as AttributeValue;
	return null;
}

/** One stored value and, when known, where its spread samples fall along the members it reaches. */
interface IntentEntry {
	value: AttributeValue;
	positions?: number[];
}

function rankPositions(count: number) {
	return count > 1 ? Array.from({ length: count }, (_, index) => index / (count - 1)) : undefined;
}

/** A Group value reaches its members; a Group family keeps per-member exceptions. */
function groupEntries(raw: unknown, owner: Owner, members: readonly string[] | undefined): IntentEntry[] {
	const record = raw as { kind?: unknown; value?: { owner?: unknown; template?: unknown; members?: Record<string, unknown> } };
	if (record?.kind === "group_family" && record.value?.owner === owner) {
		const exceptions = record.value.members ?? {};
		const entries: IntentEntry[] = Object.values(exceptions).flatMap((member) => {
			const value = intentValue(member, owner);
			return value ? [{ value }] : [];
		});
		const template = intentValue(record.value.template, owner);
		const templateMembers = members?.filter((member) => !(member in exceptions));
		if (template && (templateMembers === undefined || templateMembers.length > 0))
			entries.push({ value: template, positions: rankPositions(templateMembers?.length ?? 0) });
		return entries;
	}
	const value = intentValue(raw, owner);
	if (!value || members?.length === 0) return [];
	return [{ value, positions: rankPositions(members?.length ?? 0) }];
}

/** Every stored value of one family: per fixture, per Group and universal. */
function intentEntries(preset: PresetBody, owner: Owner, groupMembers?: GroupMembers): IntentEntry[] {
	const entries: IntentEntry[] = [];
	for (const attributes of Object.values(preset.values ?? {}))
		for (const raw of Object.values(attributes ?? {})) {
			const value = intentValue(raw, owner);
			if (value) entries.push({ value });
		}
	for (const [groupId, attributes] of Object.entries(preset.group_values ?? {}))
		for (const raw of Object.values(attributes ?? {}))
			entries.push(...groupEntries(raw, owner, groupMembers?.get(groupId)));
	for (const raw of Object.values(preset.universal_values ?? {})) {
		const value = intentValue(raw, owner);
		if (value) entries.push({ value });
	}
	return entries;
}

function channels(hex: string) {
	return [1, 3, 5].map((offset) => Number.parseInt(hex.slice(offset, offset + 2), 16));
}

/** Rounding noise between equal intents must not split one colour into two segments. */
function sameColor(left: PreviewColor, right: PreviewColor) {
	if (left.hex === null || right.hex === null) return left.hex === right.hex;
	if (Boolean(left.uv) !== Boolean(right.uv)) return false;
	const [a, b] = [channels(left.hex), channels(right.hex)];
	return a.every((channel, index) => Math.abs(channel - b[index]) <= 3);
}

/** Chromatic colours by hue, then neutrals from dark to light, then unknown appearances. */
function colorOrder(color: PreviewColor) {
	if (color.hex === null) return [3, 0];
	if (color.uv) return [1, 0];
	const [r, g, b] = channels(color.hex).map((channel) => channel / 255);
	const max = Math.max(r, g, b);
	const delta = max - Math.min(r, g, b);
	if (max === 0 || delta / max < 0.08) return [2, r + g + b];
	let hue = max === r ? (g - b) / delta : max === g ? (b - r) / delta + 2 : (r - g) / delta + 4;
	hue = (hue + 6) % 6;
	return [0, hue];
}

/** Evenly spaced picks that always keep the first and the last of an ordered list. */
function evenly<T>(items: readonly T[], limit: number) {
	if (items.length <= limit) return [...items];
	return Array.from({ length: limit }, (_, index) => items[Math.round((index * (items.length - 1)) / (limit - 1))]);
}

export function presetColorPreview(preset: PresetBody, groupMembers?: GroupMembers): PresetPreview | null {
	const distinct: PreviewColor[] = [];
	for (const entry of intentEntries(preset, "color", groupMembers))
		for (const color of colorValueColors(entry.value, entry.positions))
			if (!distinct.some((known) => sameColor(known, color))) distinct.push(color);
	if (distinct.length === 0) return null;
	const ordered = distinct
		.map((color) => ({ color, order: colorOrder(color) }))
		.sort((left, right) => left.order[0] - right.order[0] || left.order[1] - right.order[1]
			|| (left.color.hex ?? "").localeCompare(right.color.hex ?? ""))
		.map(({ color }) => color);
	return { kind: "color", colors: evenly(ordered, MAX_COLOR_SEGMENTS), distinct: distinct.length };
}

export function presetPositionPreview(preset: PresetBody, groupMembers?: GroupMembers): PresetPreview | null {
	const points = intentEntries(preset, "position", groupMembers).flatMap((entry) =>
		positionValuePoints(entry.value, entry.positions),
	);
	const space = dominantSpace(points);
	if (!space) return null;
	const shared = points.filter((point) => point.space === space);
	const unique = representativePoints(shared, Number.POSITIVE_INFINITY);
	return {
		kind: "position",
		space,
		dots: previewDots(representativePoints(unique)),
		distinct: unique.length,
	};
}

/**
 * The automatic preview a stored preset's programming intention gives its pool tile. Color and
 * Position presets preview their own family; a Mixed preset shows its colour, or else its aim.
 * Intensity and Beam presets, and presets stored before intentions, have none.
 */
export function presetIntentPreview(preset: PresetBody, groupMembers?: GroupMembers): PresetPreview | null {
	const family = normalizePresetFamily(preset.family);
	if (family === "Color") return presetColorPreview(preset, groupMembers);
	if (family === "Position") return presetPositionPreview(preset, groupMembers);
	if (family === "Mixed")
		return presetColorPreview(preset, groupMembers) ?? presetPositionPreview(preset, groupMembers);
	return null;
}

function chosen(value: string | null | undefined) {
	return value?.trim() ? value : undefined;
}

export interface PresetTileArtwork {
	icon?: string;
	color?: string;
	preview: PresetPreview | null;
}

/**
 * Precedence of a preset tile's artwork. An icon or colour the operator chose, on the button or in
 * the show, wins and keeps today's tile. Otherwise the stored intention previews itself; a preset
 * without one keeps the plain tile.
 */
export function presetTileArtwork(
	body: Pick<StoredPreset, "icon" | "color">,
	customization: { icon?: string | null; color?: string | null } | undefined,
	preview: PresetPreview | null,
): PresetTileArtwork {
	const icon = chosen(customization?.icon ?? body.icon);
	const color = chosen(customization?.color ?? body.color);
	if (icon || color) return { icon, color, preview: null };
	return { preview };
}
