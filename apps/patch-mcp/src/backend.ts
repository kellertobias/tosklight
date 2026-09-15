/**
 * What the tools need from whatever they are patching.
 *
 * There are two products in this workspace that hold a patch — the Control desk and the Architect —
 * and an operator's question ("put a wash on the downstage truss") is the same question for both.
 * So the tools are written once against this, and each product supplies its own way of answering.
 *
 * Not every product can do everything. A backend says so by leaving the optional member out, and
 * the tool that needs it fails with a sentence about the product rather than a type error about a
 * missing method — the honest answer to "rename this layer" on a product with no layer editing is
 * that it has none, not a stack trace.
 */

/** A fixture as a patch reports it. Loose on purpose: the product owns this shape, not this file. */
export type PatchedFixture = Record<string, unknown> & {
	fixture_id: string;
	fixture_number: number | null;
	/** A visual-only Venue object's `0.N` number, as N. Absent or null for a patchable fixture. */
	virtual_fixture_number?: number | null;
	name: string;
	profile_id: string;
	profile_revision: number;
	mode_id: string;
	layer_id: string;
	split_patches: Array<{
		split: number;
		universe: number | null;
		address: number | null;
	}>;
};

export interface PatchSnapshot {
	patch_revision: number;
	fixtures: PatchedFixture[];
}

/** A patch layer as a show stores it. */
export interface PatchLayer {
	id: string;
	revision: number;
	name: string;
	order: number;
}

/**
 * How a tool names a fixture: the number an operator says out loud, or a visual-only Venue object's
 * `0.N` number. `"0.10"` needs to be a string, because the number 0.10 is the number 0.1.
 */
export type FixtureRef = number | string;

export type ParsedFixtureRef =
	| { fixture_number: number; virtual_fixture_number?: undefined }
	| { virtual_fixture_number: number; fixture_number?: undefined };

export function parseFixtureRef(ref: FixtureRef): ParsedFixtureRef {
	const text = String(ref).trim();
	const virtual = /^0\.(\d+)$/.exec(text);
	if (virtual && Number(virtual[1]) > 0)
		return { virtual_fixture_number: Number(virtual[1]) };
	const number = Number(text);
	if (text && Number.isInteger(number) && number > 0)
		return { fixture_number: number };
	throw new Error(
		`${text || "an empty value"} is neither a fixture number nor a 0.N Venue object number`,
	);
}

/** The id an operator reads in the patch sheet: `12`, or `0.3` for a Venue object. */
export function fixtureLabel(
	fixture: Pick<PatchedFixture, "fixture_number" | "virtual_fixture_number">,
): string | null {
	if (fixture.virtual_fixture_number != null)
		return `0.${fixture.virtual_fixture_number}`;
	return fixture.fixture_number != null ? String(fixture.fixture_number) : null;
}

/** The fixture a reference names, or nothing. Both products look fixtures up the same way. */
export function findFixture(
	snapshot: PatchSnapshot,
	ref: FixtureRef,
): PatchedFixture | undefined {
	const parsed = parseFixtureRef(ref);
	return snapshot.fixtures.find((candidate) =>
		parsed.virtual_fixture_number !== undefined
			? candidate.virtual_fixture_number === parsed.virtual_fixture_number
			: candidate.fixture_number === parsed.fixture_number,
	);
}

/** One stored media layout object as the Architect reports it, body in the editor's spelling. */
export interface MediaEntryWire {
	object: { kind: string; body: Record<string, unknown> };
	revision: number;
}

/** The Architect's media layout snapshot, exactly as its local API returns it. */
export type MediaLayoutWire = Record<string, MediaEntryWire[]>;

export interface MediaOutcomeWire {
	requestId: string;
	replayed: boolean;
	changed: boolean;
	snapshot: MediaLayoutWire;
}

export interface PatchBackend {
	/** What this backend is, in words a tool can put in front of an operator. */
	readonly product: string;

	patch(): Promise<PatchSnapshot>;
	fixture(ref: FixtureRef): Promise<{
		snapshot: PatchSnapshot;
		fixture: PatchedFixture;
	}>;
	putFixtures(
		revision: number,
		fixtures: PatchedFixture[],
		removeFixtureIds?: string[],
	): Promise<void>;
	editFixture(
		ref: FixtureRef,
		change: (fixture: PatchedFixture) => PatchedFixture,
	): Promise<PatchedFixture>;
	profiles(): Promise<{ profiles: Array<Record<string, unknown>> }>;
	layers(): Promise<PatchLayer[]>;
	layer(id: string): Promise<PatchLayer | null>;

	/**
	 * Create or rename a layer.
	 *
	 * Absent on a product whose layers are not editable through its API. The Architect is one: its
	 * fixtures carry a layer, and it has no route that names or reorders one.
	 */
	saveLayer?(id: string, name: string, order: number): Promise<PatchLayer>;

	/**
	 * The planned media layout: media servers, their sources, LED module types, surfaces and
	 * projectors.
	 *
	 * Absent on the Control desk, whose media servers are patched fixtures rather than a planned
	 * layout.
	 */
	mediaLayout?(): Promise<MediaLayoutWire>;

	/** Put or delete one media layout object, guarded by the revision it was read at. */
	applyMediaIntent?(
		kind: string,
		id: string,
		intent: Record<string, unknown>,
	): Promise<MediaOutcomeWire>;
}

/** The refusal a tool gives when the product it is talking to cannot do the thing asked. */
export class UnsupportedByProduct extends Error {
	constructor(product: string, capability: string) {
		super(`${product} cannot ${capability}`);
	}
}
