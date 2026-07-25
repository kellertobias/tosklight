import type {
	FixtureDefinition,
	FixtureProfile,
} from "../types";
import type {
	FixtureLibraryAction,
	FixtureLibraryActionOutcome,
	FixtureDefinitionsSnapshot,
	FixtureLibraryWarningsSnapshot,
	FixtureProfilesSnapshot,
	FixtureProfileRevisionsSnapshot,
} from "../generated/light-wire";
import type { PatchSnapshot } from "../../features/patch/contracts";
import { decodePatchSnapshot } from "../patchWire";
import type { ClientTransport } from "./transport";
import { jsonRequest } from "./transport";

export class FixtureApiClient {
	constructor(private readonly transport: ClientTransport) {}

	async patch(): Promise<PatchSnapshot> {
		return decodePatchSnapshot(
			await this.transport.request<unknown>("/api/v2/patch"),
		);
	}

	fixtureLibrary(): Promise<FixtureDefinition[]> {
		return this.transport
			.request<FixtureDefinitionsSnapshot>(
				"/api/v2/fixture-library/definitions",
			)
			.then((snapshot) =>
			decodeFixtureDefinitions(snapshot.definitions),
		);
	}

	fixtureProfiles(): Promise<FixtureProfile[]> {
		return this.profileSnapshot().then((snapshot) =>
			decodeFixtureProfiles(snapshot.profiles),
		);
	}

	fixtureProfileWarnings(): Promise<string[]> {
		return this.transport
			.request<FixtureLibraryWarningsSnapshot>(
				"/api/v2/fixture-library/warnings",
			)
			.then((snapshot) => snapshot.warnings);
	}

	async fixtureProfileRevisions(id: string): Promise<FixtureProfile[]> {
		const path = `/api/v2/fixture-library/profiles/${encodeURIComponent(id)}/revisions`;
		const snapshot =
			await this.transport.request<FixtureProfileRevisionsSnapshot>(path);
		return decodeFixtureProfiles(snapshot.profiles);
	}

	async putFixtureProfile(
		profile: FixtureProfile,
		expectedRevision: number,
	): Promise<FixtureProfile> {
		const result = await this.fixtureAction({
			type: "save_profile",
			profile,
			expected_revision: expectedRevision,
		});
		if (result.type !== "profile") {
			throw new Error(`Expected fixture profile result, received ${result.type}`);
		}
		return this.profileFromAuthority(result.profile_id, result.revision);
	}

	async deleteFixtureProfile(id: string, revision: number): Promise<void> {
		await this.fixtureAction({
			type: "delete_profile_revision",
			profile_id: id,
			revision,
		});
	}

	async putFixtureProfileSourceGdtf(
		id: string,
		revision: number,
		source: Uint8Array,
	): Promise<void> {
		await this.fixtureAction({
			type: "attach_gdtf",
			profile_id: id,
			revision,
			source_base64: await bytesToBase64(source),
		});
	}

	async importFixturePackage(source: Uint8Array): Promise<FixtureProfile> {
		const result = await this.fixtureAction({
			type: "import_package",
			package_base64: await bytesToBase64(source),
		});
		if (result.type !== "profile") {
			throw new Error(`Expected fixture profile result, received ${result.type}`);
		}
		return this.profileFromAuthority(result.profile_id, result.revision);
	}

	exportFixturePackage(id: string, revision: number): Promise<Blob> {
		const path = `/api/v2/fixture-library/profiles/${encodeURIComponent(id)}/revisions/${revision}/package`;
		return this.transport.blob(path);
	}

	async putFixtureDefinition(
		definition: FixtureDefinition,
	): Promise<FixtureDefinition> {
		const result = await this.fixtureAction({
			type: "save_definition",
			definition,
		});
		if (result.type !== "definition") {
			throw new Error(
				`Expected fixture definition result, received ${result.type}`,
			);
		}
		const snapshot = await this.definitionSnapshot();
		const storedDefinition = decodeFixtureDefinitions(snapshot.definitions).find(
			(candidate) =>
				candidate.id === result.definition_id &&
				candidate.revision === result.revision,
		);
		if (!storedDefinition) {
			throw new Error("Saved fixture definition is missing from the snapshot");
		}
		return storedDefinition;
	}

	async deleteFixtureDefinition(id: string, revision: number): Promise<void> {
		await this.fixtureAction({
			type: "delete_definition_revision",
			definition_id: id,
			revision,
		});
	}

	private profileSnapshot(): Promise<FixtureProfilesSnapshot> {
		return this.transport.request("/api/v2/fixture-library/profiles");
	}

	private definitionSnapshot(): Promise<FixtureDefinitionsSnapshot> {
		return this.transport.request("/api/v2/fixture-library/definitions");
	}

	private async profileFromAuthority(
		id: string,
		revision: number,
	): Promise<FixtureProfile> {
		const profiles = decodeFixtureProfiles(
			(await this.profileSnapshot()).profiles,
		);
		const profile = profiles.find(
			(candidate) => candidate.id === id && candidate.revision === revision,
		);
		if (!profile) {
			throw new Error("Saved fixture profile is missing from the snapshot");
		}
		return profile;
	}

	private async fixtureAction(action: FixtureLibraryAction) {
		const outcome = await this.transport.request<FixtureLibraryActionOutcome>(
			"/api/v2/fixture-library",
			jsonRequest("POST", {
				request_id: crypto.randomUUID(),
				action,
			}),
		);
		return outcome.result;
	}
}

function decodeFixtureProfiles(values: unknown[]): FixtureProfile[] {
	return values.map(decodeFixtureProfile);
}

function decodeFixtureDefinitions(values: unknown[]): FixtureDefinition[] {
	return values.map(decodeFixtureDefinition);
}

function decodeFixtureProfile(value: unknown): FixtureProfile {
	if (!isFixtureRecord(value) || !Array.isArray(value.modes)) {
		throw new Error("Fixture profile response is malformed");
	}
	return value as unknown as FixtureProfile;
}

function decodeFixtureDefinition(value: unknown): FixtureDefinition {
	if (!isFixtureRecord(value) || typeof value.mode !== "string") {
		throw new Error("Fixture definition response is malformed");
	}
	return value as unknown as FixtureDefinition;
}

function isFixtureRecord(
	value: unknown,
): value is Record<string, unknown> & { id: string; revision: number } {
	return (
		typeof value === "object" &&
		value !== null &&
		typeof (value as { id?: unknown }).id === "string" &&
		typeof (value as { revision?: unknown }).revision === "number"
	);
}

function bytesToBase64(source: Uint8Array): Promise<string> {
	const body = source.buffer.slice(
		source.byteOffset,
		source.byteOffset + source.byteLength,
	) as ArrayBuffer;
	return new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.onerror = () =>
			reject(reader.error ?? new Error("Unable to encode fixture archive"));
		reader.onload = () => {
			const encoded = String(reader.result).split(",", 2)[1];
			if (encoded === undefined) {
				reject(new Error("Unable to encode fixture archive"));
				return;
			}
			resolve(encoded);
		};
		reader.readAsDataURL(new Blob([body]));
	});
}
