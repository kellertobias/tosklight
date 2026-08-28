/**
 * The Architect, as the patch tools need it.
 *
 * The Architect keeps the same patch as the desk and calls its fields the same things — a fixture
 * has a `profile_id`, a `layer_id`, `split_patches`. It only *spells* them differently on the wire,
 * in camelCase, because its own windows are a TypeScript application and that is their convention.
 * So this is the desk client with a spelling change at the boundary, not a second model.
 *
 * Doing it here rather than in the tools is the point: a tool should be able to say
 * `fixture.layer_id = "truss"` without knowing which product it is holding.
 *
 * Finding the editor is the other difference. A desk listens on a port an operator configured; the
 * Architect's editing API binds an ephemeral loopback port and writes the port and its token to a
 * file in its own data directory. Reading that file is how a tool the operator started finds the
 * editor the operator is running — and why a program that was not started by them cannot.
 */

import type {
	PatchBackend,
	PatchLayer,
	PatchSnapshot,
	PatchedFixture,
} from "./backend";
import { DeskError } from "./desk";

export interface ArchitectOptions {
	/** Where the editor's local API is, when it is already known. */
	baseUrl?: string;
	/** The token from the editor's handle file, when it is already known. */
	token?: string;
	/** Where to look for the handle file. Defaults to the platform's app data directory. */
	handlePath?: string;
	fetch?: typeof globalThis.fetch;
	/**
	 * How to read the handle file.
	 *
	 * Supplied by the caller rather than imported here, which keeps this package free of a Node
	 * type dependency it would otherwise need for one call — the same reason `main.ts` declares the
	 * two environment variables it reads instead of pulling in the whole platform.
	 */
	readFile?: (path: string) => Promise<string>;
}

interface Handle {
	port: number;
	token: string;
}

/** Keys as the Architect spells them. */
function toCamel(value: unknown): unknown {
	if (Array.isArray(value)) return value.map(toCamel);
	if (value === null || typeof value !== "object") return value;
	return Object.fromEntries(
		Object.entries(value as Record<string, unknown>).map(([key, inner]) => [
			key.replace(/_([a-z0-9])/g, (_, character: string) =>
				character.toUpperCase(),
			),
			toCamel(inner),
		]),
	);
}

/** Keys as the tools and the desk spell them. */
function toSnake(value: unknown): unknown {
	if (Array.isArray(value)) return value.map(toSnake);
	if (value === null || typeof value !== "object") return value;
	return Object.fromEntries(
		Object.entries(value as Record<string, unknown>).map(([key, inner]) => [
			key.replace(/[A-Z]/g, (character) => `_${character.toLowerCase()}`),
			toSnake(inner),
		]),
	);
}

export class Architect implements PatchBackend {
	readonly product = "The Architect";

	private readonly options: ArchitectOptions;
	private readonly http: typeof globalThis.fetch;
	private handle: Handle | null = null;

	constructor(options: ArchitectOptions = {}) {
		this.options = options;
		this.http = options.fetch ?? globalThis.fetch;
		if (options.baseUrl && options.token) {
			const port = Number(new URL(options.baseUrl).port);
			this.handle = { port, token: options.token };
		}
	}

	/**
	 * Where the running editor is, read once.
	 *
	 * The file is rewritten on every launch, so a tool that starts after the editor finds the port
	 * this process owns rather than one a previous run left behind.
	 */
	private async reach(): Promise<Handle> {
		if (this.handle) return this.handle;
		const path = this.options.handlePath ?? defaultHandlePath();
		const read = this.options.readFile;
		if (!read)
			throw new DeskError(
				"this Architect client was built without a way to read the editor's handle file",
			);
		let raw: string;
		try {
			raw = await read(path);
		} catch {
			throw new DeskError(
				`no running Architect found: ${path} is not readable. Start the Architect first.`,
			);
		}
		const parsed = JSON.parse(raw) as Partial<Handle>;
		if (typeof parsed.port !== "number" || typeof parsed.token !== "string")
			throw new DeskError(`${path} does not name a port and a token`);
		this.handle = { port: parsed.port, token: parsed.token };
		return this.handle;
	}

	private async request<T>(
		method: string,
		path: string,
		body?: unknown,
		revision?: number,
	): Promise<T> {
		const handle = await this.reach();
		const headers: Record<string, string> = {
			authorization: `Bearer ${handle.token}`,
		};
		if (body !== undefined) headers["content-type"] = "application/json";
		if (revision !== undefined) headers["if-match"] = String(revision);
		// A handle file outlives the editor that wrote it, so the first symptom of a closed editor is
		// a refused connection on a port nothing is listening to. Saying that plainly beats handing
		// an operator an ECONNREFUSED and a port number.
		let response: Response;
		try {
			response = await this.http(`http://127.0.0.1:${handle.port}${path}`, {
				method,
				headers,
				body: body === undefined ? undefined : JSON.stringify(body),
			});
		} catch (reason) {
			throw new DeskError(
				`the Architect is not answering on port ${handle.port}. It was running when it wrote its handle file; it may have been closed since. Reason: ${reason instanceof Error ? reason.message : String(reason)}`,
			);
		}
		const text = await response.text();
		if (!response.ok)
			throw new DeskError(
				`${method} ${path} failed: ${response.status} ${text.slice(0, 400)}`,
			);
		return (text ? JSON.parse(text) : undefined) as T;
	}

	async patch(): Promise<PatchSnapshot> {
		const snapshot = await this.request<unknown>("GET", "/api/v2/patch");
		return toSnake(snapshot) as PatchSnapshot;
	}

	async fixture(number: number) {
		const snapshot = await this.patch();
		const fixture = snapshot.fixtures.find(
			(candidate) => candidate.fixture_number === number,
		);
		if (!fixture) throw new DeskError(`no fixture numbered ${number}`);
		return { snapshot, fixture };
	}

	async putFixtures(
		revision: number,
		fixtures: PatchedFixture[],
		removeFixtureIds: string[] = [],
	): Promise<void> {
		await this.request(
			"POST",
			"/api/v2/patch/fixtures",
			toCamel({
				request_id: crypto.randomUUID(),
				fixtures,
				remove_fixture_ids: removeFixtureIds,
			}),
			revision,
		);
	}

	async editFixture(
		number: number,
		change: (fixture: PatchedFixture) => PatchedFixture,
	): Promise<PatchedFixture> {
		const { snapshot, fixture } = await this.fixture(number);
		const edited = change(structuredClone(fixture));
		await this.putFixtures(snapshot.patch_revision, [edited]);
		return edited;
	}

	/**
	 * The editor's fixture library, flattened into the shape the tools read.
	 *
	 * The Architect reports a profile as an envelope with the whole profile inside it; the desk
	 * reports the profile itself. The tools want the profile, so the envelope is opened here.
	 */
	async profiles(): Promise<{ profiles: Array<Record<string, unknown>> }> {
		const listed = await this.request<{
			profiles: Array<Record<string, unknown>>;
		}>("GET", "/api/v2/fixture-library/profiles");
		const profiles = (listed.profiles ?? []).map((entry) => {
			const inner = toSnake(entry.profile) as Record<string, unknown> | null;
			return { ...(inner ?? {}), id: entry.id, revision: entry.revision };
		});
		return { profiles };
	}

	async layers(): Promise<PatchLayer[]> {
		const snapshot = await this.request<{
			objects: Array<{ id: string; revision: number; body: unknown }>;
		}>("GET", "/api/v2/objects/patch_layer");
		return (snapshot.objects ?? [])
			.map((object) => {
				const body = (object.body ?? {}) as { name?: string; order?: number };
				return {
					id: object.id,
					revision: object.revision,
					name: body.name ?? object.id,
					order: body.order ?? 0,
				};
			})
			.sort((left, right) => left.order - right.order);
	}

	async layer(id: string): Promise<PatchLayer | null> {
		return (await this.layers()).find((layer) => layer.id === id) ?? null;
	}

	// No saveLayer: the Architect has no route that names or reorders a layer. Leaving it out is
	// what lets the tool say so plainly instead of failing somewhere in an HTTP call.
}

/** Where the Architect writes its handle, per platform. */
function defaultHandlePath(): string {
	const environment = (
		globalThis as {
			process?: { env?: Record<string, string | undefined>; platform?: string };
		}
	).process;
	const home = environment?.env?.HOME ?? environment?.env?.USERPROFILE ?? ".";
	const identifier = "de.tokenet.tosklight.visualizer";
	if (environment?.platform === "darwin")
		return `${home}/Library/Application Support/${identifier}/local-api.json`;
	if (environment?.platform === "win32")
		return `${environment?.env?.APPDATA ?? home}/${identifier}/local-api.json`;
	return `${environment?.env?.XDG_DATA_HOME ?? `${home}/.local/share`}/${identifier}/local-api.json`;
}
