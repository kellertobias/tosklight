/**
 * How the CAD add buttons read the fixture library and place a Venue object in the show.
 *
 * The library is read afresh whenever a menu or dialog opens and on every press, so the buttons offer
 * what this computer's library holds now. A part is placed at the stage origin with the first free
 * virtual ID, written straight to the show, so a refusal carries the show's own reason.
 */
import { type FixtureDefinition, mergeFixtureDefinitions, newPatchFixtureCandidate } from "@tosklight/patch";
import { useEffect, useState } from "react";
import { documentSession } from "../document/session";
import { TauriPatchTransport } from "../document/transport";
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

/** Places one object at the stage origin with the first free virtual ID, and returns its fixture ID. */
async function placeDefinition(definition: FixtureDefinition): Promise<string> {
	const snapshot = await documentSession.patchSnapshot();
	const candidate = newPatchFixtureCandidate({
		name: definition.name,
		fixture_number: null,
		virtual_fixture_number: nextVirtualNumber(
			snapshot.fixtures.map((fixture) => fixture.virtualFixtureNumber),
		),
		definition,
		universe: null,
		address: null,
		layer_id: "default",
	});
	await transport.patchFixtures(snapshot.showId, snapshot.patchRevision, {
		requestId: crypto.randomUUID(),
		fixtures: [candidate.input],
		removeFixtureIds: [],
	});
	return candidate.fixture.fixture_id;
}

export type PlaceResult = { ok: true; fixtureId: string } | { ok: false; reason: string };

/**
 * Places a profile's newest revision, reading the library first unless a read one is given. `label`
 * names the object in the reason a placement fails.
 */
export async function placeProfile(
	profileId: string,
	label: string,
	known?: FixtureLibrary,
): Promise<PlaceResult> {
	const library = known?.state === "ready" ? known : await readLibrary();
	if (library.state === "failed")
		return { ok: false, reason: `The fixture library could not be read: ${library.reason}` };
	const definition = definitionForProfile(library.definitions, profileId);
	if (!definition)
		return { ok: false, reason: `The ${label} is not in this computer's fixture library.` };
	try {
		return { ok: true, fixtureId: await placeDefinition(definition) };
	} catch (reason) {
		return { ok: false, reason: `The show refused the ${label}: ${String(reason)}` };
	}
}
