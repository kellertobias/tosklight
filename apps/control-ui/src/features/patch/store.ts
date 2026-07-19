import type { PatchedFixture } from "../../api/types";
import type {
	PatchChange,
	PatchMutationOutcome,
	PatchProfileRevision,
	PatchSnapshot,
} from "./contracts";
import {
	type PatchDefinitionResolver,
	type PatchFixtureCandidate,
	projectionToPatchedFixture,
} from "./model";

export type PatchStoreStatus =
	| "loading"
	| "ready"
	| "repairing"
	| "error";

export interface PatchStoreSnapshot {
	status: PatchStoreStatus;
	showId: string;
	showRevision: number | null;
	patchRevision: number | null;
	cursor: number | null;
	fixtures: readonly PatchedFixture[];
	pendingFixtureIds: ReadonlySet<string>;
	error: string | null;
}

export type PatchDeltaResult = "applied" | "stale" | "repair";

interface PendingPatch {
	candidates: readonly PatchFixtureCandidate[];
	removeFixtureIds: readonly string[];
}

export class PatchStore {
	private readonly listeners = new Set<() => void>();
	private authoritative = new Map<string, PatchedFixture>();
	private profiles = new Map<string, PatchProfileRevision>();
	private pending = new Map<string, PendingPatch>();
	private showRevision: number | null = null;
	private patchRevision: number | null = null;
	private cursor: number | null = null;
	private status: PatchStoreStatus = "loading";
	private error: string | null = null;
	private value: PatchStoreSnapshot;

	constructor(
		private readonly showId: string,
		private readonly resolveDefinition: PatchDefinitionResolver,
		initialFixtures: readonly PatchedFixture[] = [],
	) {
		this.authoritative = new Map(
			initialFixtures.map((fixture) => [fixture.fixture_id, fixture]),
		);
		this.value = this.buildSnapshot();
	}

	subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	getSnapshot = () => this.value;

	begin(
		requestId: string,
		candidates: readonly PatchFixtureCandidate[],
		removeFixtureIds: readonly string[],
	): void {
		if (this.pending.has(requestId))
			throw new Error("Patch request is already pending: " + requestId);
		this.pending.set(requestId, { candidates, removeFixtureIds });
		this.error = null;
		this.publish();
	}

	rollback(requestId: string, error: Error): void {
		this.pending.delete(requestId);
		this.error = error.message;
		this.status = this.showRevision == null ? "error" : "ready";
		this.publish();
	}

	replacePending(
		requestId: string,
		candidates: readonly PatchFixtureCandidate[],
		removeFixtureIds: readonly string[],
	): void {
		if (!this.pending.has(requestId))
			throw new Error("Patch request is no longer pending: " + requestId);
		this.pending.set(requestId, { candidates, removeFixtureIds });
		this.publish();
	}

	fixtureBefore(
		requestId: string,
		fixtureId: string,
	): PatchedFixture | undefined {
		let fixture = this.authoritative.get(fixtureId);
		for (const [pendingId, patch] of this.pending) {
			if (pendingId === requestId) break;
			if (patch.removeFixtureIds.includes(fixtureId)) fixture = undefined;
			const candidate = patch.candidates.find(
				(item) => item.fixture.fixture_id === fixtureId,
			);
			if (candidate) fixture = candidate.fixture;
		}
		return fixture;
	}

	applySnapshot(snapshot: PatchSnapshot): void {
		if (snapshot.showId !== this.showId)
			throw new Error(
				"Patch snapshot belongs to show " +
					snapshot.showId +
					", expected " +
					this.showId,
			);
		const profiles = profileMap(snapshot.profileRevisions);
		const next = new Map<string, PatchedFixture>();
		for (const projection of snapshot.fixtures) {
			const profile = requiredProfile(profiles, projection);
			const fallback = this.fixtureHint(projection.fixtureId);
			next.set(
				projection.fixtureId,
				projectionToPatchedFixture(
					projection,
					profile,
					this.resolveDefinition,
					fallback,
				),
			);
		}
		this.authoritative = next;
		this.profiles = profiles;
		this.showRevision = snapshot.showRevision;
		this.patchRevision = snapshot.patchRevision;
		this.cursor = snapshot.cursor;
		this.status = "ready";
		this.error = null;
		this.publish();
	}

	applyDelta(delta: PatchChange, sequence = delta.eventSequence): PatchDeltaResult {
		const result = this.applyDeltaWithoutPublishing(delta, sequence);
		if (result !== "repair") this.publish();
		return result;
	}

	applyOutcome(
		requestId: string,
		outcome: PatchMutationOutcome,
	): PatchDeltaResult {
		const result = this.applyDeltaWithoutPublishing(
			outcome,
			outcome.eventSequence,
			requestId,
		);
		this.pending.delete(requestId);
		if (result === "repair") {
			this.status = "repairing";
		} else {
			this.status = "ready";
			this.error = null;
		}
		this.publish();
		return result;
	}

	markRepairing(): void {
		this.status = "repairing";
		this.publish();
	}

	setError(error: Error): void {
		this.error = error.message;
		this.status = this.showRevision == null ? "error" : "ready";
		this.publish();
	}

	private applyDeltaWithoutPublishing(
		delta: PatchChange,
		sequence: number | null,
		requestId?: string,
	): PatchDeltaResult {
		if (
			delta.showId !== this.showId ||
			this.showRevision == null ||
			this.patchRevision == null
		)
			return "repair";
		if (delta.patchRevision < this.patchRevision) return "stale";
		if (delta.patchRevision === this.patchRevision) {
			if (
				sequence != null &&
				this.cursor != null &&
				sequence > this.cursor
			)
				return "repair";
			return "stale";
		}
		if (
			delta.patchRevision !== this.patchRevision + 1 ||
			delta.showRevision <= this.showRevision ||
			(sequence != null && this.cursor != null && sequence <= this.cursor)
		)
			return "repair";

		const profiles = new Map(this.profiles);
		for (const profile of delta.profileRevisions)
			profiles.set(profileKey(profile.profileId, profile.profileRevision), profile);
		const authoritative = new Map(this.authoritative);
		for (const fixtureId of delta.removedFixtureIds)
			authoritative.delete(fixtureId);
		for (const projection of delta.fixtures) {
			const profile = requiredProfile(profiles, projection);
			authoritative.set(
				projection.fixtureId,
				projectionToPatchedFixture(
					projection,
					profile,
					this.resolveDefinition,
					this.fixtureHint(projection.fixtureId, requestId),
				),
			);
		}
		this.authoritative = authoritative;
		this.profiles = profiles;
		this.showRevision = delta.showRevision;
		this.patchRevision = delta.patchRevision;
		if (sequence != null)
			this.cursor = Math.max(this.cursor ?? 0, sequence);
		this.status = "ready";
		this.error = null;
		return "applied";
	}

	private fixtureHint(
		fixtureId: string,
		requestId?: string,
	): PatchedFixture | undefined {
		const requested = requestId
			? this.pending.get(requestId)
			: undefined;
		const requestedCandidate = requested?.candidates.find(
			(item) => item.fixture.fixture_id === fixtureId,
		);
		if (requestedCandidate) return requestedCandidate.fixture;
		const authoritative = this.authoritative.get(fixtureId);
		if (authoritative) return authoritative;
		for (const patch of this.pending.values()) {
			const candidate = patch.candidates.find(
				(item) => item.fixture.fixture_id === fixtureId,
			);
			if (candidate) return candidate.fixture;
		}
		return undefined;
	}

	private buildSnapshot(): PatchStoreSnapshot {
		const visible = new Map(this.authoritative);
		const pendingFixtureIds = new Set<string>();
		for (const patch of this.pending.values()) {
			for (const fixtureId of patch.removeFixtureIds) visible.delete(fixtureId);
			for (const candidate of patch.candidates) {
				visible.set(candidate.fixture.fixture_id, candidate.fixture);
				pendingFixtureIds.add(candidate.fixture.fixture_id);
			}
		}
		return {
			status: this.status,
			showId: this.showId,
			showRevision: this.showRevision,
			patchRevision: this.patchRevision,
			cursor: this.cursor,
			fixtures: [...visible.values()],
			pendingFixtureIds,
			error: this.error,
		};
	}

	private publish(): void {
		this.value = this.buildSnapshot();
		for (const listener of this.listeners) listener();
	}
}

function profileMap(
	profiles: readonly PatchProfileRevision[],
): Map<string, PatchProfileRevision> {
	return new Map(
		profiles.map((profile) => [
			profileKey(profile.profileId, profile.profileRevision),
			profile,
		]),
	);
}

function requiredProfile(
	profiles: ReadonlyMap<string, PatchProfileRevision>,
	fixture: { profileId: string; profileRevision: number },
): PatchProfileRevision {
	const profile = profiles.get(
		profileKey(fixture.profileId, fixture.profileRevision),
	);
	if (!profile)
		throw new Error(
			"Patch projection references missing profile " +
				fixture.profileId +
				" revision " +
				fixture.profileRevision,
		);
	return profile;
}

function profileKey(profileId: string, revision: number) {
	return profileId + ":" + revision;
}
