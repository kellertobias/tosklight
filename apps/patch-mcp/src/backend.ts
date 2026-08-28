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

export interface PatchBackend {
	/** What this backend is, in words a tool can put in front of an operator. */
	readonly product: string;

	patch(): Promise<PatchSnapshot>;
	fixture(number: number): Promise<{
		snapshot: PatchSnapshot;
		fixture: PatchedFixture;
	}>;
	putFixtures(
		revision: number,
		fixtures: PatchedFixture[],
		removeFixtureIds?: string[],
	): Promise<void>;
	editFixture(
		number: number,
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
}

/** The refusal a tool gives when the product it is talking to cannot do the thing asked. */
export class UnsupportedByProduct extends Error {
	constructor(product: string, capability: string) {
		super(`${product} cannot ${capability}`);
	}
}
