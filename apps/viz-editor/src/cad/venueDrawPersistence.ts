import { documentSession } from "../document/session";
import { ARCHITECTURE_PROFILE, newArchitecture } from "./architectureDraft";
import { placeProfile } from "./cadPlacement";
import { cadSession } from "./session";
import type { OpeningPreview } from "./venueDrawingGeometry";

export async function placeWallSegment(
	start: readonly [number, number, number],
	end: readonly [number, number, number],
): Promise<readonly string[]> {
	const length = Math.round(Math.hypot(end[0] - start[0], end[1] - start[1]));
	if (length < 10) return [];
	const draft = newArchitecture("wall");
	const result = await placeProfile(
		ARCHITECTURE_PROFILE,
		"wall",
		undefined,
		[
			{
				position: {
					x: Math.round((start[0] + end[0]) / 2),
					y: Math.round((start[1] + end[1]) / 2),
					z: start[2],
				},
				rotation: {
					x: 0,
					y: 0,
					z: (Math.atan2(end[1] - start[1], end[0] - start[0]) * 180) / Math.PI,
				},
			},
		],
		{
			name: "Wall",
			sceneryOptions: {
				architecture: draft.architecture,
				colourSrgb: draft.colourSrgb,
			},
			scenerySizeMetres: { ...draft.size, x: length },
		},
	);
	if (!result.ok) throw new Error(result.reason);
	return result.fixtureIds;
}
export async function placeDrawnObject(
	kind: "pillar" | "stairs",
	point: readonly [number, number, number],
): Promise<readonly string[]> {
	const draft = newArchitecture(kind);
	const result = await placeProfile(
		ARCHITECTURE_PROFILE,
		draft.name,
		undefined,
		[
			{
				position: {
					x: Math.round(point[0]),
					y: Math.round(point[1]),
					z: Math.round(point[2]),
				},
				rotation: { x: 0, y: 0, z: 0 },
			},
		],
		{
			name: draft.name,
			sceneryOptions: {
				architecture: draft.architecture,
				colourSrgb: draft.colourSrgb,
			},
			scenerySizeMetres: draft.size,
		},
	);
	if (!result.ok) throw new Error(result.reason);
	return result.fixtureIds;
}
export async function placeWallOpening(
	preview: OpeningPreview,
): Promise<string> {
	const scene = await cadSession.snapshot();
	const snapshot = await documentSession.patchSnapshot();
	const fixture = snapshot.fixtures.find(
		(f) => f.fixtureId === preview.wall.logicalFixtureId,
	);
	if (!fixture || fixture.sceneryOptions?.architecture?.kind !== "wall")
		throw new Error(
			"This wall is no longer available. Refresh the drawing and try again.",
		);
	const opening = { ...preview.opening, id: crypto.randomUUID() };
	await cadSession.patchVenue(scene.sceneRevision, [
		{
			...fixture,
			sceneryOptions: {
				...fixture.sceneryOptions,
				architecture: {
					...fixture.sceneryOptions.architecture,
					openings: [...fixture.sceneryOptions.architecture.openings, opening],
				},
			},
		},
	]);
	return opening.id;
}
