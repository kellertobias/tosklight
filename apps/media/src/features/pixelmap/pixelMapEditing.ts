// The pure part of editing a pixel map: what a new zone, route or region looks like, and what the
// operator is told is wrong with the one they have.
//
// Kept apart from the component so the rules can be read and tested without rendering anything.

import type {
	DisplayRegionView,
	PixelMapView,
	PixelRouteView,
	PixelZoneHandoffView,
	PixelZoneView,
} from "../../shared/api/generated/media-wire";

/// The channel layouts offered by name. The model takes any sequence of components, so this is a
/// convenience for the common fixtures rather than the limit of what a zone can hold.
export const PIXEL_LAYOUTS: { name: string; components: string[] }[] = [
	{ name: "RGB", components: ["red", "green", "blue"] },
	{ name: "RGBW", components: ["red", "green", "blue", "white"] },
	{ name: "RGBA", components: ["red", "green", "blue", "amber"] },
	{
		name: "RGBWA+UV",
		components: ["red", "green", "blue", "white", "amber", "ultra-violet"],
	},
	{ name: "Dimmer RGB", components: ["dimmer", "red", "green", "blue"] },
];

export const PIXEL_ORDERS = [
	{ value: "row-major", label: "Rows, left to right" },
	{ value: "column-major", label: "Columns, top to bottom" },
	{ value: "serpentine-rows", label: "Rows, folding back" },
	{ value: "serpentine-columns", label: "Columns, folding back" },
];

export const REGION_ROTATIONS = [
	{ value: "none", label: "Upright" },
	{ value: "clockwise-90", label: "Turned clockwise" },
	{ value: "half", label: "Upside down" },
	{ value: "counter-clockwise-90", label: "Turned anti-clockwise" },
];

export const REGION_FITS = [
	{ value: "fill", label: "Fill the screen" },
	{ value: "contain", label: "Fit on the screen" },
	{ value: "stretch", label: "Stretch to the screen" },
];

/** A short, stable identity for something the operator just made. */
export function newId(prefix: string): string {
	const random = globalThis.crypto?.randomUUID?.() ?? String(Date.now());
	return `${prefix}-${random.slice(0, 8)}`;
}

/// A new zone covering the middle of the canvas, so it is visible the moment it is added and can
/// be dragged or typed into place from there.
export function newZone(existing: readonly PixelZoneView[]): PixelZoneView {
	const layout = PIXEL_LAYOUTS[0];
	return {
		id: newId("zone"),
		name: `Zone ${existing.length + 1}`,
		start: { x: 0.25, y: 0.4 },
		end: { x: 0.75, y: 0.6 },
		columns: 12,
		rows: 1,
		layout: { name: layout.name, components: [...layout.components] },
		order: "row-major",
		universe: 1,
		startAddress: nextFreeAddress(existing, 1, 12 * layout.components.length),
		enabled: true,
		footprint: 12 * layout.components.length,
	};
}

/// The lowest address in a universe with room for a zone of this size, so a second zone does not
/// land on the first one and have to be moved by hand.
export function nextFreeAddress(
	zones: readonly PixelZoneView[],
	universe: number,
	footprint: number,
): number {
	const taken = zones
		.filter((zone) => zone.universe === universe)
		.map((zone) => [zone.startAddress, zone.startAddress + zone.footprint - 1])
		.sort((left, right) => left[0] - right[0]);
	let candidate = 1;
	for (const [from, to] of taken) {
		if (candidate + footprint - 1 < from) break;
		candidate = Math.max(candidate, to + 1);
	}
	return candidate;
}

export function newRoute(existing: readonly PixelRouteView[]): PixelRouteView {
	const universe = existing.length + 1;
	return {
		id: newId("route"),
		name: `Universe ${universe}`,
		protocol: "art-net",
		universe,
		destination: null,
		enabled: true,
	};
}

export function newHandoff(zone: PixelZoneView): PixelZoneHandoffView {
	return {
		zoneId: zone.id,
		fixtureName: zone.name,
		protocol: "art-net",
		inputUniverse: zone.universe,
		inputStartAddress: 3,
		dimmerAddress: 1,
		mixAddress: 2,
		fixtureFootprint: footprintOf(zone),
		automaticPatch: false,
	};
}

export function newRegion(
	existing: readonly DisplayRegionView[],
): DisplayRegionView {
	return {
		id: newId("region"),
		name: `Screen ${existing.length + 1}`,
		start: { x: 0, y: 0 },
		end: { x: 1, y: 1 },
		rotation: "none",
		fit: "fill",
		enabled: true,
	};
}

/** How many slots a zone of this shape and layout occupies. */
export function footprintOf(zone: PixelZoneView): number {
	return zone.columns * zone.rows * zone.layout.components.length;
}

/// What is wrong with this map, in the operator's words.
///
/// The server refuses the same things, but saying so before the save means an address clash is a
/// number to correct rather than a rejected edit.
export function pixelMapProblems(map: PixelMapView): string[] {
	const problems: string[] = [];
	const enabled = map.zones.filter((zone) => zone.enabled);
	for (const zone of map.zones) {
		if (zone.columns < 1 || zone.rows < 1) {
			problems.push(
				`${zone.name} has no pixels; give it at least one column and one row.`,
			);
		}
		if (zone.layout.components.length === 0) {
			problems.push(`${zone.name} has no colour channels to send.`);
		}
		const last = zone.startAddress + footprintOf(zone) - 1;
		if (zone.startAddress < 1 || last > 512) {
			problems.push(
				`${zone.name} needs ${footprintOf(zone)} slots from ${zone.startAddress}, which runs past the end of universe ${zone.universe}.`,
			);
		}
	}
	for (let index = 0; index < enabled.length; index += 1) {
		for (let other = index + 1; other < enabled.length; other += 1) {
			const first = enabled[index];
			const second = enabled[other];
			if (first.universe !== second.universe) continue;
			const firstLast = first.startAddress + footprintOf(first) - 1;
			const secondLast = second.startAddress + footprintOf(second) - 1;
			if (
				first.startAddress <= secondLast &&
				second.startAddress <= firstLast
			) {
				problems.push(
					`${first.name} and ${second.name} both use universe ${first.universe} around address ${Math.max(first.startAddress, second.startAddress)}.`,
				);
			}
		}
	}
	const carried = new Set(
		map.routes.filter((route) => route.enabled).map((route) => route.universe),
	);
	for (const zone of enabled) {
		if (!carried.has(zone.universe)) {
			problems.push(
				`${zone.name} sends universe ${zone.universe}, which no enabled output route carries.`,
			);
		}
	}
	if (map.mode === "direct" && map.handoffs.length > 0) {
		problems.push("Direct Media Server mode cannot carry desk input patches.");
	}
	if (map.mode === "desk-merge") {
		for (const zone of enabled) {
			const matching = map.handoffs.filter(
				(handoff) => handoff.zoneId === zone.id,
			);
			if (matching.length !== 1) {
				problems.push(`${zone.name} needs exactly one desk input patch.`);
				continue;
			}
			const handoff = matching[0];
			if (handoff.fixtureFootprint < footprintOf(zone)) {
				problems.push(
					`${zone.name}'s zone fixture is smaller than its mapped pixel channels.`,
				);
			}
			if (handoff.dimmerAddress === handoff.mixAddress) {
				problems.push(
					`${zone.name}'s Dimmer and Mix addresses must be different.`,
				);
			}
			if (
				handoff.inputStartAddress < 1 ||
				handoff.inputStartAddress + handoff.fixtureFootprint - 1 > 512
			) {
				problems.push(`${zone.name}'s desk input patch runs past address 512.`);
			}
			if (zone.startAddress + handoff.fixtureFootprint - 1 > 512) {
				problems.push(
					`${zone.name}'s Media Server output patch runs past address 512.`,
				);
			}
		}
	}
	for (const region of map.regions) {
		if (region.start.x === region.end.x || region.start.y === region.end.y) {
			problems.push(`${region.name} covers none of the canvas.`);
		}
	}
	return problems;
}
