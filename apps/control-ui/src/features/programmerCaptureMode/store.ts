import type {
	ProgrammerCaptureModeProjection,
	ProgrammerCaptureModeSnapshot,
} from "./contracts";
import {
	assertCaptureModeCursor,
	canonicalCaptureModeProjection,
	sameCaptureModeProjection,
} from "./projectionValue";
import { ProgrammerCaptureModeProtocolError } from "./transport";

export type ProgrammerCaptureModeStatus =
	| "idle"
	| "loading"
	| "ready"
	| "error";

export interface ProgrammerCaptureModeState {
	showId: string | null;
	userId: string | null;
	eventSequence: number | null;
	projection: ProgrammerCaptureModeProjection | null;
	status: ProgrammerCaptureModeStatus;
	error: Error | null;
	repairRequired: boolean;
}

export class ProgrammerCaptureModeStore {
	private readonly listeners = new Set<() => void>();
	private authoritative: ProgrammerCaptureModeProjection | null = null;
	private authorityKey: string | null = null;
	private scope = 0;
	private state = emptyCaptureModeState();

	readonly subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	readonly getSnapshot = () => this.state;

	reset(showId: string | null, userId: string | null, authorityKey = "") {
		if (this.matchesScope(showId, userId, authorityKey)) return;
		this.scope++;
		this.authoritative = null;
		this.authorityKey = authorityKey;
		this.state = { ...emptyCaptureModeState(), showId, userId };
		this.emit();
	}

	installSnapshot(
		snapshot: ProgrammerCaptureModeSnapshot,
		expectedScope = this.scope,
	) {
		return this.install(snapshot.projection, snapshot.cursor, expectedScope);
	}

	installRepairSnapshot(
		snapshot: ProgrammerCaptureModeSnapshot,
		expectedScope = this.scope,
	) {
		if (!this.canAccept(snapshot.projection.userId, expectedScope))
			return false;
		try {
			assertCaptureModeCursor(snapshot.cursor);
			if (
				this.state.eventSequence !== null &&
				snapshot.cursor < this.state.eventSequence
			)
				throw new ProgrammerCaptureModeProtocolError(
					"Programmer capture mode repair cursor moved backwards",
					snapshot.cursor,
				);
			this.publishAuthority(
				canonicalCaptureModeProjection(snapshot.projection),
				snapshot.cursor,
			);
			return true;
		} catch (reason) {
			return this.rejectProtocol(reason, snapshot.cursor);
		}
	}

	applyProjection(
		projection: ProgrammerCaptureModeProjection,
		sequence: number,
		expectedScope = this.scope,
	) {
		return this.install(projection, sequence, expectedScope);
	}

	setLoading(expectedScope = this.scope) {
		return this.publishState({ status: "loading", error: null }, expectedScope);
	}

	setReady(expectedScope = this.scope) {
		return this.publishState(
			{ status: "ready", error: null, repairRequired: false },
			expectedScope,
		);
	}

	setError(error: Error, expectedScope = this.scope) {
		return this.publishState({ status: "error", error }, expectedScope);
	}

	setRepairRequired(error: Error, expectedScope = this.scope) {
		return this.publishState(
			{ status: "error", error, repairRequired: true },
			expectedScope,
		);
	}

	captureScope() {
		return this.scope;
	}

	isScopeCurrent(scope: number) {
		return scope === this.scope;
	}

	private install(
		projection: ProgrammerCaptureModeProjection,
		sequence: number,
		expectedScope: number,
	) {
		if (!this.canAccept(projection.userId, expectedScope)) return false;
		try {
			assertCaptureModeCursor(sequence);
			const incoming = canonicalCaptureModeProjection(projection);
			const decision = this.chooseAuthority(incoming, sequence);
			if (decision.publish)
				this.publishAuthority(decision.projection, decision.sequence);
			return true;
		} catch (reason) {
			return this.rejectProtocol(reason, sequence);
		}
	}

	private chooseAuthority(
		incoming: ProgrammerCaptureModeProjection,
		sequence: number,
	) {
		const currentSequence = this.state.eventSequence;
		if (currentSequence !== null && sequence < currentSequence)
			return this.decision(
				this.authoritative ?? incoming,
				currentSequence,
				false,
			);
		if (currentSequence === sequence && this.authoritative) {
			if (sameCaptureModeProjection(this.authoritative, incoming))
				return this.decision(this.authoritative, sequence, false);
			throw this.conflict("events", sequence);
		}
		return this.chooseRevision(incoming, sequence);
	}

	private chooseRevision(
		incoming: ProgrammerCaptureModeProjection,
		sequence: number,
	) {
		if (!this.authoritative || incoming.revision > this.authoritative.revision)
			return this.decision(incoming, sequence, true);
		if (incoming.revision < this.authoritative.revision)
			return this.decision(this.authoritative, sequence, true);
		if (sameCaptureModeProjection(this.authoritative, incoming))
			return this.decision(this.authoritative, sequence, true);
		throw this.conflict("projections", sequence);
	}

	private decision(
		projection: ProgrammerCaptureModeProjection,
		sequence: number,
		publish: boolean,
	) {
		return { projection, sequence, publish };
	}

	private conflict(subject: string, sequence: number) {
		return new ProgrammerCaptureModeProtocolError(
			`Conflicting Programmer capture mode ${subject}`,
			sequence,
		);
	}

	private publishAuthority(
		projection: ProgrammerCaptureModeProjection,
		sequence: number,
	) {
		this.authoritative = projection;
		this.state = {
			...this.state,
			projection,
			eventSequence: sequence,
			status: "ready",
			error: null,
			repairRequired: false,
		};
		this.emit();
	}

	private publishState(
		update: Partial<ProgrammerCaptureModeState>,
		expectedScope: number,
	) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		this.state = { ...this.state, ...update };
		this.emit();
		return true;
	}

	private canAccept(userId: string, expectedScope: number) {
		return (
			this.isScopeCurrent(expectedScope) &&
			Boolean(this.state.showId) &&
			userId === this.state.userId
		);
	}

	private matchesScope(
		showId: string | null,
		userId: string | null,
		authorityKey: string,
	) {
		return (
			showId === this.state.showId &&
			userId === this.state.userId &&
			authorityKey === this.authorityKey
		);
	}

	private rejectProtocol(reason: unknown, sequence: number): never {
		const error =
			reason instanceof ProgrammerCaptureModeProtocolError
				? reason
				: new ProgrammerCaptureModeProtocolError(String(reason), sequence);
		this.state = {
			...this.state,
			status: "error",
			error,
			repairRequired: true,
		};
		this.emit();
		throw error;
	}

	private emit() {
		for (const listener of this.listeners) listener();
	}
}

function emptyCaptureModeState(): ProgrammerCaptureModeState {
	return {
		showId: null,
		userId: null,
		eventSequence: null,
		projection: null,
		status: "idle",
		error: null,
		repairRequired: false,
	};
}
