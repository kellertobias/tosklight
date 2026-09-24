/**
 * How the CAD add buttons read the fixture library and place a Venue object in the show.
 *
 * The library is read afresh whenever a menu or dialog opens and on every press, so the buttons offer
 * what this computer's library holds now. A part is placed at the stage origin with the first free
 * virtual ID, written straight to the show, so a refusal carries the show's own reason.
 */
import {
	type FixtureDefinition,
	mergeFixtureDefinitions,
	newPatchFixtureCandidate,
	type PatchFixtureWrite,
} from "@tosklight/patch";
import { useEffect, useState } from "react";
import { documentSession } from "../document/session";
import { TauriPatchTransport } from "../document/transport";
import type { PlanPlacement } from "./bulkPlacement";
import { definitionForProfile, nextVirtualNumber } from "./venueParts";

const transport = new TauriPatchTransport();

/** The library as last read: still reading, read, or refused with a reason. */
export type FixtureLibrary =
	| { state: "loading" }
	| { state: "ready"; definitions: readonly FixtureDefinition[] }
	| { state: "failed"; reason: string };

export async function readLibrary(): Promise<Exclude<FixtureLibrary, { state: "loading" }>> {
	try {
		const definitions = mergeFixtureDefinitions(await documentSession.fixtureProfiles(), []);
		return { state: "ready", definitions };
	} catch (reason) {
		return { state: "failed", reason: String(reason) };
	}
}

/** The library, read once when the component that shows it mounts. */
export function useFixtureLibrary(): FixtureLibrary {
	const [library, setLibrary] = useState<FixtureLibrary>({ state: "loading" });
	useEffect(() => {
		let live = true;
		void readLibrary().then((next) => {
			if (live) setLibrary(next);
		});
		return () => {
			live = false;
		};
	}, []);
	return library;
}

/**
 * Places objects where a layout puts them, in one write, and returns their fixture IDs in order.
 *
 * Each takes the next free virtual ID, counted on from the ones already taken as the batch is
 * built, so a field of twenty decks numbers itself the way twenty presses would without any two
 * landing on the same number. The whole batch goes to the show as one mutation: either every
 * element is placed or none is, and the show's own reason comes back for the batch.
 */
async function placeDefinitions(
	definition: FixtureDefinition,
	placements: readonly PlanPlacement[],
	sceneryOptions?: PatchFixtureWrite["sceneryOptions"],
): Promise<string[]> {
	const snapshot = await documentSession.patchSnapshot();
	const taken = snapshot.fixtures.map((fixture) => fixture.virtualFixtureNumber);
	const fixtureIds: string[] = [];
	const fixtures = placements.map((placement) => {
		const virtual = nextVirtualNumber(taken);
		taken.push(virtual);
		const candidate = newPatchFixtureCandidate({
			name: definition.name,
			fixture_number: null,
			virtual_fixture_number: virtual,
			definition,
			universe: null,
			address: null,
			layer_id: "default",
		});
		fixtureIds.push(candidate.fixture.fixture_id);
		return {
			...candidate.input,
			location: placement.position,
			rotation: placement.rotation,
			...(sceneryOptions ? { sceneryOptions } : {}),
		};
	});
	if (!fixtures.length) return [];
	await transport.patchFixtures(snapshot.showId, snapshot.patchRevision, {
		requestId: crypto.randomUUID(),
		fixtures,
		removeFixtureIds: [],
	});
	return fixtureIds;
}

/** Where one object pressed on its own lands: the stage origin, turned as it was built. */
const AT_THE_ORIGIN: readonly PlanPlacement[] = [
	{ position: { x: 0, y: 0, z: 0 }, rotation: { x: 0, y: 0, z: 0 } },
];

export type PlaceResult =
	| { ok: true; fixtureIds: readonly string[] }
	| { ok: false; reason: string };

/**
 * Places a profile's newest revision, reading the library first unless a read one is given. `label`
 * names the object in the reason a placement fails. Without a layout it places one at the origin,
 * which is what a press of an add button does. `sceneryOptions` go with every placed object, such
 * as the handrails a flight of stairs was chosen with.
 */
export async function placeProfile(
	profileId: string,
	label: string,
	known?: FixtureLibrary,
	placements: readonly PlanPlacement[] = AT_THE_ORIGIN,
	sceneryOptions?: PatchFixtureWrite["sceneryOptions"],
): Promise<PlaceResult> {
	const library = known?.state === "ready" ? known : await readLibrary();
	if (library.state === "failed")
		return { ok: false, reason: `The fixture library could not be read: ${library.reason}` };
	const definition = definitionForProfile(library.definitions, profileId);
	if (!definition)
		return { ok: false, reason: `The ${label} is not in this computer's fixture library.` };
	try {
		return { ok: true, fixtureIds: await placeDefinitions(definition, placements, sceneryOptions) };
	} catch (reason) {
		return { ok: false, reason: `The show refused the ${label}: ${String(reason)}` };
	}
}
