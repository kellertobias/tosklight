import type {
	FixtureDefinition,
	FixtureProfile,
	PatchSnapshot,
} from "../types";
import type { ClientTransport } from "./transport";
import { jsonRequest } from "./transport";

export class FixtureApiClient {
	constructor(private readonly transport: ClientTransport) {}

	patch(): Promise<PatchSnapshot> {
		return this.transport.request("/api/v1/patch", {}, false);
	}

	fixtureLibrary(): Promise<FixtureDefinition[]> {
		return this.transport.request("/api/v1/fixture-library", {}, false);
	}

	fixtureProfiles(): Promise<FixtureProfile[]> {
		return this.transport.request("/api/v1/fixture-profiles", {}, false);
	}

	fixtureProfileWarnings(): Promise<string[]> {
		return this.transport.request(
			"/api/v1/fixture-profiles/warnings",
			{},
			false,
		);
	}

	fixtureProfileRevisions(id: string): Promise<FixtureProfile[]> {
		const path = `/api/v1/fixture-profiles/${encodeURIComponent(id)}/revisions`;
		return this.transport.request(path, {}, false);
	}

	putFixtureProfile(profile: FixtureProfile, expectedRevision: number) {
		return this.transport.request<FixtureProfile>("/api/v1/fixture-profiles", {
			...jsonRequest("PUT", profile),
			headers: {
				"content-type": "application/json",
				"if-match": String(expectedRevision),
			},
		});
	}

	deleteFixtureProfile(id: string, revision: number): Promise<void> {
		const path = `/api/v1/fixture-profiles/${encodeURIComponent(id)}/${revision}`;
		return this.transport.request(path, { method: "DELETE" });
	}

	putFixtureProfileSourceGdtf(
		id: string,
		revision: number,
		source: Uint8Array,
	) {
		const path = `/api/v1/fixture-profiles/${encodeURIComponent(id)}/${revision}/source-gdtf`;
		return this.transport.request<void>(path, binaryRequest("PUT", source));
	}

	importFixturePackage(source: Uint8Array) {
		return this.transport.request<FixtureProfile>(
			"/api/v1/fixture-packages/import",
			binaryRequest("POST", source, "application/vnd.tosklight.fixture+zip"),
		);
	}

	exportFixturePackage(id: string, revision: number): Promise<Blob> {
		const path = `/api/v1/fixture-profiles/${encodeURIComponent(id)}/${revision}/package`;
		return this.transport.blob(path);
	}

	putFixtureDefinition(definition: FixtureDefinition) {
		return this.transport.request<FixtureDefinition>(
			"/api/v1/fixture-library",
			jsonRequest("PUT", definition),
		);
	}

	deleteFixtureDefinition(id: string, revision: number): Promise<void> {
		return this.transport.request(`/api/v1/fixture-library/${id}/${revision}`, {
			method: "DELETE",
		});
	}
}

function binaryRequest(
	method: "POST" | "PUT",
	source: Uint8Array,
	contentType = "application/octet-stream",
): RequestInit {
	const body = source.buffer.slice(
		source.byteOffset,
		source.byteOffset + source.byteLength,
	) as ArrayBuffer;
	return { method, headers: { "content-type": contentType }, body };
}
