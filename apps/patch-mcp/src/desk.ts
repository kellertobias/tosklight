/**
 * The ToskLight desk, as the patch tools need it.
 *
 * Every editing capability here is a read-modify-write against `/api/v2/patch/fixtures`: that
 * endpoint takes whole fixture records and a patch revision, so a tool that wants to change one
 * field has to read the fixture first, change that field, and send the rest back untouched. Doing
 * that in one place is what stops a tool quietly dropping a field it did not know about.
 */

import type {
	PatchBackend,
	PatchLayer,
	PatchSnapshot,
	PatchedFixture,
} from "./backend";

export interface DeskOptions {
	/** Where the desk is listening. */
	baseUrl: string;
	/** The desk alias this client presents as. One alias is one desk. */
	deskId: string;
	fetch?: typeof globalThis.fetch;
}

// The patch shapes are the same on both products and now live in `backend.ts`, beside the
// interface the tools are written against. Re-exported here so existing importers are undisturbed.
export type { PatchLayer, PatchedFixture, PatchSnapshot } from "./backend";

interface PatchLayerBody {
	name?: string;
	order?: number;
}

export class DeskError extends Error {}

export class Desk implements PatchBackend {
	readonly product = "The Control desk";

	private readonly options: DeskOptions;
	private readonly http: typeof globalThis.fetch;
	private token: string | null = null;

	constructor(options: DeskOptions) {
		this.options = options;
		this.http = options.fetch ?? globalThis.fetch;
	}

	/**
	 * A session, opened once and reused.
	 *
	 * The desk admits a client by desk alias; it carries no user, so there is nothing here to
	 * prompt for and nothing to store.
	 */
	private async session(): Promise<string> {
		if (this.token) return this.token;
		const opened = await this.request<{ token?: string; id?: string }>(
			"POST",
			"/api/v2/sessions",
			{ desk_id: this.options.deskId },
			false,
		);
		const token = opened.token ?? opened.id;
		if (!token) throw new DeskError("the desk opened a session without a token");
		this.token = token;
		return token;
	}

	async request<T>(
		method: string,
		path: string,
		body?: unknown,
		authenticate = true,
		revision?: number,
	): Promise<T> {
		const headers: Record<string, string> = {};
		if (body !== undefined) headers["content-type"] = "application/json";
		if (authenticate) headers.authorization = `Bearer ${await this.session()}`;
		if (revision !== undefined) headers["if-match"] = String(revision);
		const response = await this.http(`${this.options.baseUrl}${path}`, {
			method,
			headers,
			body: body === undefined ? undefined : JSON.stringify(body),
		});
		const text = await response.text();
		if (!response.ok) {
			throw new DeskError(
				`${method} ${path} failed: ${response.status} ${text.slice(0, 400)}`,
			);
		}
		return (text ? JSON.parse(text) : undefined) as T;
	}

	patch(): Promise<PatchSnapshot> {
		return this.request<PatchSnapshot>("GET", "/api/v2/patch");
	}

	/**
	 * Find one fixture by its operator-facing number.
	 *
	 * Fixture numbers are what an operator says out loud, so they are what the tools take. The
	 * internal id is never asked for and never has to be guessed at.
	 */
	async fixture(number: number): Promise<{
		snapshot: PatchSnapshot;
		fixture: PatchedFixture;
	}> {
		const snapshot = await this.patch();
		const fixture = snapshot.fixtures.find(
			(candidate) => candidate.fixture_number === number,
		);
		if (!fixture) throw new DeskError(`no fixture numbered ${number}`);
		return { snapshot, fixture };
	}

	/** Send whole fixture records back, against the revision they were read at. */
	async putFixtures(
		revision: number,
		fixtures: PatchedFixture[],
		removeFixtureIds: string[] = [],
	): Promise<void> {
		await this.request(
			"POST",
			"/api/v2/patch/fixtures",
			{
				request_id: crypto.randomUUID(),
				fixtures,
				remove_fixture_ids: removeFixtureIds,
			},
			true,
			revision,
		);
	}

	/**
	 * Change one fixture and write it back unchanged in every other respect.
	 *
	 * The read and the write share a revision, so an edit made against a patch that has since
	 * moved is refused by the desk rather than silently overwriting someone else's work.
	 */
	async editFixture(
		number: number,
		change: (fixture: PatchedFixture) => PatchedFixture,
	): Promise<PatchedFixture> {
		const { snapshot, fixture } = await this.fixture(number);
		const edited = change(structuredClone(fixture));
		await this.putFixtures(snapshot.patch_revision, [edited]);
		return edited;
	}

	profiles(): Promise<{ profiles: Array<Record<string, unknown>> }> {
		return this.request("GET", "/api/v2/fixture-library/profiles");
	}

	/**
	 * The patch layers the show actually stores.
	 *
	 * A layer is a show object with a name and an order, not something inferred from the fixtures
	 * standing on it: a show may hold a named layer nothing is patched onto yet, and that layer is
	 * as real as any other.
	 */
	async layers(): Promise<PatchLayer[]> {
		const snapshot = await this.request<{
			objects: Array<{ id: string; revision: number; body: PatchLayerBody }>;
		}>("GET", "/api/v2/objects/patch_layer");
		return snapshot.objects
			.map((object) => ({
				id: object.id,
				revision: object.revision,
				name: object.body?.name ?? object.id,
				order: object.body?.order ?? 0,
			}))
			.sort((left, right) => left.order - right.order);
	}

	async layer(id: string): Promise<PatchLayer | null> {
		return (await this.layers()).find((layer) => layer.id === id) ?? null;
	}

	/**
	 * Create a layer, or rename and reorder an existing one.
	 *
	 * The desk has one route for both, and the revision decides which it is: a layer that does not
	 * exist yet is saved against revision 0. The revision read here is the one written back, so a
	 * layer someone else has moved in between is refused rather than overwritten.
	 */
	async saveLayer(id: string, name: string, order: number): Promise<PatchLayer> {
		const existing = await this.layer(id);
		await this.request(
			"POST",
			`/api/v2/patch/layers/${encodeURIComponent(id)}/update`,
			{
				request_id: crypto.randomUUID(),
				action: {
					type: "save",
					expected_revision: existing?.revision ?? 0,
					layer: { name, order },
				},
			},
		);
		return { id, revision: (existing?.revision ?? 0) + 1, name, order };
	}
}
