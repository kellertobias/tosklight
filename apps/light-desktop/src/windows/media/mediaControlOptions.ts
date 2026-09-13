import type { MediaEffectLibrarySlot } from "./mediaPaneModel";

const BLEND_MODES = [
	"Normal",
	"Add",
	"Screen",
	"Multiply",
	"Overlay",
	"Difference",
	"Lighten",
	"Darken",
];
/** Blend mode strobe: raw 128–249 run 1 Hz to 25 Hz; 250–255 is Normal without strobe. */
const BLEND_STROBE = { from: 128, to: 249, slowHz: 1, fastHz: 25 };
const BLEND_NO_STROBE = 250;

function blendStrobeRaw(hz: number) {
	const { from, to, slowHz, fastHz } = BLEND_STROBE;
	return from + Math.round(((hz - slowHz) / (fastHz - slowHz)) * (to - from));
}

export function blendStrobeHz(rawValue: number) {
	const { from, to, slowHz, fastHz } = BLEND_STROBE;
	if (rawValue < from || rawValue > to) return null;
	return slowHz + ((rawValue - from) / (to - from)) * (fastHz - slowHz);
}

export const BLEND_MODE_OPTIONS = [
	...BLEND_MODES.map((label, index) => ({
		value: String(index * 16),
		label,
	})),
	...[1, 2, 5, 10, 15, 20, 25].map((hz) => ({
		value: String(blendStrobeRaw(hz)),
		label: `Strobe ${hz} Hz`,
	})),
	{ value: String(BLEND_NO_STROBE), label: "Normal, no strobe" },
];

export function nearestBlendValue(rawValue: number) {
	if (rawValue < BLEND_STROBE.from) return Math.floor(rawValue / 16) * 16;
	if (rawValue > BLEND_STROBE.to) return BLEND_NO_STROBE;
	return BLEND_MODE_OPTIONS.slice(BLEND_MODES.length, -1).reduce(
		(nearest, option) => {
			const value = Number(option.value);
			return Math.abs(rawValue - value) < Math.abs(rawValue - nearest)
				? value
				: nearest;
		},
		BLEND_STROBE.from,
	);
}

export const SPEED_OPTIONS = [
	...Array.from({ length: 15 }, (_, index) => ({
		value: String(index * 8),
		label: `/${16 - index}`,
	})),
	{ value: "127", label: "1×" },
	...Array.from({ length: 15 }, (_, index) => {
		const multiplier = index + 2;
		const raw = Math.ceil(135 + (index * 121) / 15);
		return { value: String(raw), label: `${multiplier}×` };
	}),
];

export const PLAY_MODE_OPTIONS = [
	[0, "Loop"],
	[20, "Reverse"],
	[40, "Bounce"],
	[60, "Once — Hold"],
	[68, "Once — Black"],
	[76, "Once — Transparent"],
	[84, "Reverse Once — Hold"],
	[92, "Reverse Once — Black"],
	[100, "Reverse Once — Transparent"],
	[108, "Loop Synced"],
	[128, "Reverse Synced"],
	[148, "Bounce Synced"],
	[168, "Once Synced — Hold"],
	[176, "Once Synced — Black"],
	[184, "Once Synced — Transparent"],
	[192, "Reverse Once Synced — Hold"],
	[200, "Reverse Once Synced — Black"],
	[208, "Reverse Once Synced — Transparent"],
	[216, "Stop"],
	[236, "Pause"],
].map(([value, label]) => ({ value: String(value), label: String(label) }));

export const SCALING_MODE_OPTIONS = [
	{ value: "0", label: "Fit" },
	{ value: "64", label: "Fill" },
	{ value: "128", label: "Original" },
	{ value: "192", label: "Stretch" },
];

export const MASK_INVERT_OPTIONS = [
	{ value: "0", label: "Normal" },
	{ value: "255", label: "Invert" },
];

export const FLIP_MIRROR_OPTIONS = [
	{ value: "0", label: "None" },
	{ value: "1", label: "Horizontal" },
	{ value: "2", label: "Vertical" },
	{ value: "3", label: "Both" },
];

export function effectLibrarySlotOptions(
	slots?: readonly MediaEffectLibrarySlot[],
) {
	const statusBySlot = new Map(slots?.map((slot) => [slot.slot, slot]));
	return [
		{ value: "0", label: "Off" },
		...Array.from({ length: 255 }, (_, index) => {
			const slot = index + 1;
			const state = statusBySlot.get(slot);
			const suffix =
				state?.status === "assigned"
					? state.name?.trim() || "Assigned"
					: state?.status === "unsupported"
						? "Unsupported"
						: state?.status === "unassigned"
							? "Unassigned"
							: null;
			return {
				value: String(slot),
				label: suffix ? `Slot ${slot} · ${suffix}` : `Slot ${slot}`,
			};
		}),
	];
}

export function effectLibrarySlotDescription(
	selected: number,
	slots?: readonly MediaEffectLibrarySlot[],
) {
	if (selected === 0) return "Off bypasses this bank.";
	const state = slots?.find((slot) => slot.slot === selected);
	if (!state || state.status === "assigned") return undefined;
	return (
		state.detail ??
		(state.status === "unsupported"
			? `Slot ${selected} uses an unsupported effect preset.`
			: `Slot ${selected} is unassigned.`)
	);
}

export const OPACITY_CYCLE_OPTIONS = [
	{ value: "0", label: "Off" },
	{ value: "1", label: "/16" },
	{ value: "32", label: "/8" },
	{ value: "64", label: "/4" },
	{ value: "96", label: "/2" },
	{ value: "128", label: "1×" },
	{ value: "160", label: "2×" },
	{ value: "192", label: "4×" },
	{ value: "224", label: "8×" },
	{ value: "240", label: "16×" },
];

export function nearestOpacityCycleValue(rawValue: number) {
	return OPACITY_CYCLE_OPTIONS.reduce((nearest, option) => {
		const value = Number(option.value);
		return Math.abs(rawValue - value) < Math.abs(rawValue - nearest)
			? value
			: nearest;
	}, 0);
}
