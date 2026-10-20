/** Venue authoring uses the same millimetres and local outlines as the portable show contract. */
import type {
	PatchFixtureProjection,
	VenueArchitecture,
} from "@tosklight/patch";

export const ARCHITECTURE_PROFILE = "0087038f-6a2f-5d74-9185-8d14d7e1ee48";
export type ArchitectureKind = VenueArchitecture["kind"];
export interface ArchitectureDraft {
	name: string;
	architecture: VenueArchitecture;
	size: { x: number; y: number; z: number };
	position: { x: number; y: number; z: number };
	angle: number;
}
const rectangle = [
	{ x: -3000, y: -2000 },
	{ x: 3000, y: -2000 },
	{ x: 3000, y: 2000 },
	{ x: -3000, y: 2000 },
];
export function newArchitecture(kind: ArchitectureKind): ArchitectureDraft {
	const architecture: VenueArchitecture =
		kind === "wall"
			? { kind, openings: [], ceiling_height_millimetres: 4000 }
			: kind === "slab"
				? {
						kind,
						purpose: "balcony",
						outline_millimetres: rectangle,
						elevation_millimetres: 3000,
						thickness_millimetres: 200,
					}
				: kind === "pillar"
					? { kind, shape: "rectangular" }
					: kind === "table"
						? { kind, shape: "rectangular" }
						: kind === "chair"
							? { kind, style: "black_metal" }
							: { kind: "bar" };
	const sizes = {
		wall: [6000, 4000, 200],
		slab: [6000, 200, 4000],
		pillar: [500, 3000, 500],
		table: [1600, 750, 800],
		chair: [500, 900, 500],
		bar: [2000, 1100, 700],
	};
	const [x, y, z] = sizes[kind];
	return {
		name:
			kind === "slab"
				? "Balcony"
				: kind.charAt(0).toUpperCase() + kind.slice(1),
		architecture,
		size: { x, y, z },
		position: { x: 0, y: 0, z: 0 },
		angle: 0,
	};
}
export function architectureFromFixture(
	fixture: PatchFixtureProjection,
): ArchitectureDraft {
	return {
		name: fixture.name,
		architecture:
			fixture.sceneryOptions?.architecture ??
			newArchitecture("wall").architecture,
		size: fixture.scenerySizeMetres ?? { x: 6000, y: 4000, z: 200 },
		position: fixture.location,
		angle: fixture.rotation.z,
	};
}
/** Snap either wall end to another wall end, preserving the chosen length and heading. */
export function snapWallDraft(
	draft: ArchitectureDraft,
	fixtures: readonly PatchFixtureProjection[],
	except?: string,
): ArchitectureDraft {
	if (draft.architecture.kind !== "wall") return draft;
	const angle = (draft.angle * Math.PI) / 180;
	const dx = (Math.cos(angle) * draft.size.x) / 2,
		dy = (Math.sin(angle) * draft.size.x) / 2;
	let best = 250;
	let shift: { x: number; y: number } | null = null;
	for (const fixture of fixtures) {
		if (
			fixture.fixtureId === except ||
			fixture.sceneryOptions?.architecture?.kind !== "wall"
		)
			continue;
		if (Math.abs(fixture.location.z - draft.position.z) > 250) continue;
		const a = (fixture.rotation.z * Math.PI) / 180;
		const half = (fixture.scenerySizeMetres?.x ?? 0) / 2;
		for (const source of [-1, 1])
			for (const target of [-1, 1]) {
				const x =
					fixture.location.x +
					target * Math.cos(a) * half -
					draft.position.x -
					source * dx;
				const y =
					fixture.location.y +
					target * Math.sin(a) * half -
					draft.position.y -
					source * dy;
				const distance = Math.hypot(x, y);
				if (distance < best) {
					best = distance;
					shift = { x, y };
				}
			}
	}
	return shift
		? {
				...draft,
				position: {
					...draft.position,
					x: Math.round(draft.position.x + shift.x),
					y: Math.round(draft.position.y + shift.y),
				},
			}
		: draft;
}
