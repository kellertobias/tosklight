import type {
	ShowObject,
	ShowObjectKind,
} from "../features/showObjects/contracts";
import { decodeShowObject } from "./showObjectWire";
import { WireValidationError } from "./wireValidation";

interface HttpShowObjectSnapshotTransportOptions {
	baseUrl: string;
	sessionToken: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
}

export interface ShowObjectCollectionSnapshot<K extends ShowObjectKind> {
	objects: ShowObject<K>[];
	showRevision: number;
}

export interface ShowObjectExactSnapshot<K extends ShowObjectKind> {
	object: ShowObject<K> | null;
	showRevision: number;
}

/** Narrow authenticated collection hydration; construction performs no network work. */
export class HttpShowObjectSnapshotTransport {
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;

	constructor(private readonly options: HttpShowObjectSnapshotTransportOptions) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation = options.fetch ?? globalThis.fetch.bind(globalThis);
	}

	async collection<K extends ShowObjectKind>(
		showId: string,
		kind: K,
	): Promise<ShowObjectCollectionSnapshot<K>> {
		const path = `/api/v1/shows/${encodeURIComponent(showId)}/objects/${encodeURIComponent(kind)}`;
		const response = await this.fetchImplementation(`${this.baseUrl}${path}`, {
			headers: this.headers(),
		});
		if (!response.ok) throw new Error(await response.text());
		const value: unknown = await response.json();
		if (!Array.isArray(value))
			throw new WireValidationError("$", "show-object array", value);
		return {
			objects: value.map((object, index) =>
				decodeShowObject(object, kind, `$[${index}]`),
			),
			showRevision: revisionEtag(response),
		};
	}

	async object<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		objectId: string,
	): Promise<ShowObjectExactSnapshot<K>> {
		const path = `/api/v1/shows/${encodeURIComponent(showId)}/objects/${encodeURIComponent(kind)}/${encodeURIComponent(objectId)}`;
		const response = await this.fetchImplementation(`${this.baseUrl}${path}`, {
			headers: this.headers(),
		});
		const showRevision = revisionHeader(response, "x-light-show-revision");
		if (response.status === 404) return { object: null, showRevision };
		if (!response.ok) throw new Error(await response.text());
		const object = decodeShowObject(await response.json(), kind);
		if (object.id !== objectId)
			throw new WireValidationError("$.id", objectId, object.id);
		return { object, showRevision };
	}

	private headers() {
		const headers = new Headers({
			authorization: `Bearer ${this.options.sessionToken}`,
		});
		if (this.options.deskBoundaryToken)
			headers.set("x-light-desk-token", this.options.deskBoundaryToken);
		return headers;
	}
}

function revisionEtag(response: Response) {
	return revisionHeader(response, "etag");
}

function revisionHeader(response: Response, name: string) {
	const value = response.headers.get(name);
	const match = /^"(0|[1-9]\d*)"$/.exec(value ?? "");
	if (!match)
		throw new WireValidationError(
			"$.headers.etag",
			"quoted non-negative Show revision",
			value,
		);
	const revision = Number(match[1]);
	if (!Number.isSafeInteger(revision))
		throw new WireValidationError(
			"$.headers.etag",
			"safe Show revision",
			value,
		);
	return revision;
}
