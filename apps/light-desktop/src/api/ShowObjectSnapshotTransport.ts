import type {
	ShowObject,
	ShowObjectKind,
} from "../features/showObjects/contracts";
import {
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
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
		const path = `/api/v2/objects/${encodeURIComponent(kind)}`;
		const response = await this.fetchImplementation(`${this.baseUrl}${path}`, {
			headers: this.headers(showId),
		});
		if (!response.ok) throw new Error(await response.text());
		const snapshot = recordAt(await response.json(), "$");
		assertIdentity(snapshot.show_id, showId, "$.show_id");
		assertIdentity(snapshot.kind, kind, "$.kind");
		if (!Array.isArray(snapshot.objects))
			throw new WireValidationError(
				"$.objects",
				"show-object array",
				snapshot.objects,
			);
		return {
			objects: snapshot.objects.map((object, index) =>
				decodeShowObject(object, kind, `$.objects[${index}]`),
			),
			showRevision: integerAt(snapshot.show_revision, "$.show_revision"),
		};
	}

	async object<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		objectId: string,
	): Promise<ShowObjectExactSnapshot<K>> {
		const path = `/api/v2/objects/${encodeURIComponent(kind)}/${encodeURIComponent(objectId)}`;
		const response = await this.fetchImplementation(`${this.baseUrl}${path}`, {
			headers: this.headers(showId),
		});
		if (!response.ok) throw new Error(await response.text());
		const snapshot = recordAt(await response.json(), "$");
		assertIdentity(snapshot.show_id, showId, "$.show_id");
		assertIdentity(snapshot.kind, kind, "$.kind");
		assertIdentity(snapshot.object_id, objectId, "$.object_id");
		const showRevision = integerAt(
			snapshot.show_revision,
			"$.show_revision",
		);
		if (snapshot.object === null) return { object: null, showRevision };
		const object = decodeShowObject(snapshot.object, kind, "$.object");
		if (object.id !== objectId)
			throw new WireValidationError("$.object.id", objectId, object.id);
		return { object, showRevision };
	}

	private headers(showId: string) {
		const headers = new Headers({
			authorization: `Bearer ${this.options.sessionToken}`,
			"x-tosk-show": showId,
		});
		if (this.options.deskBoundaryToken)
			headers.set("x-light-desk-token", this.options.deskBoundaryToken);
		return headers;
	}
}

function assertIdentity(value: unknown, expected: string, path: string) {
	const actual = stringAt(value, path);
	if (actual !== expected) throw new WireValidationError(path, expected, actual);
}
