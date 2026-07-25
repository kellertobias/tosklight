import type {
	ProgrammerLifecycleChange,
	ProgrammerLifecycleProjection,
	ProgrammerLifecycleRow,
	ProgrammerLifecycleSnapshot,
} from "./contracts";
import {
	assertLifecycleCursor,
	canonicalLifecycleChange,
	canonicalLifecycleProjection,
	lifecycleProjectionFromCanonicalRows,
} from "./projectionValue";
import { ProgrammerLifecycleProtocolError } from "./transport";

export type ProgrammerLifecycleStatus = "idle" | "loading" | "ready" | "error";

export interface ProgrammerLifecycleState {
	authorityKey: string | null;
	eventSequence: number | null;
	projection: ProgrammerLifecycleProjection | null;
	status: ProgrammerLifecycleStatus;
	error: Error | null;
	repairRequired: boolean;
}

export class ProgrammerLifecycleStore {
	private readonly listeners = new Set<() => void>();
	private scope = 0;
	private state = emptyState();

	readonly subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	readonly getSnapshot = () => this.state;

	reset(authorityKey: string | null) {
		if (authorityKey === this.state.authorityKey) return;
		this.scope++;
		this.state = { ...emptyState(), authorityKey };
		this.emit();
	}

	installSnapshot(
		snapshot: ProgrammerLifecycleSnapshot,
		expectedScope = this.scope,
	) {
		return this.installSnapshotValue(snapshot, expectedScope, false);
	}

	installRepairSnapshot(
		snapshot: ProgrammerLifecycleSnapshot,
		expectedScope = this.scope,
	) {
		return this.installSnapshotValue(snapshot, expectedScope, true);
	}

	applyChange(
		change: ProgrammerLifecycleChange,
		sequence: number,
		expectedScope = this.scope,
	) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		try {
			assertLifecycleCursor(sequence);
			const projection = this.requireProjection(sequence);
			const currentSequence = this.requireSequence(sequence);
			if (sequence <= currentSequence) return true;
			const delta = canonicalLifecycleChange(change);
			this.assertNextRevision(delta.revision, projection.revision, sequence);
			this.publishProjection(applyDelta(projection, delta), sequence);
			return true;
		} catch (reason) {
			throw this.markProtocolRepair(reason, sequence);
		}
	}

	setLoading(expectedScope = this.scope) {
		return this.publishSessionState(
			{ status: "loading", error: null },
			expectedScope,
		);
	}

	setReady(expectedScope = this.scope) {
		return this.publishSessionState(
			{ status: "ready", error: null, repairRequired: false },
			expectedScope,
		);
	}

	setError(error: Error, expectedScope = this.scope) {
		return this.publishSessionState({ status: "error", error }, expectedScope);
	}

	setRepairRequired(error: Error, expectedScope = this.scope) {
		return this.publishSessionState(
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

	private installSnapshotValue(
		snapshot: ProgrammerLifecycleSnapshot,
		expectedScope: number,
		repair: boolean,
	) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		try {
			assertLifecycleCursor(snapshot.cursor);
			const projection = canonicalLifecycleProjection(snapshot.projection);
			if (repair) this.assertRepairDoesNotRegress(snapshot, projection);
			this.publishProjection(projection, snapshot.cursor);
			return true;
		} catch (reason) {
			throw this.markProtocolRepair(reason, snapshot.cursor);
		}
	}

	private assertRepairDoesNotRegress(
		snapshot: ProgrammerLifecycleSnapshot,
		projection: ProgrammerLifecycleProjection,
	) {
		if (
			this.state.eventSequence !== null &&
			snapshot.cursor < this.state.eventSequence
		)
			throw protocolError("repair cursor moved backwards", snapshot.cursor);
		if (
			this.state.projection &&
			projection.revision < this.state.projection.revision
		)
			throw protocolError("repair revision moved backwards", snapshot.cursor);
	}

	private requireProjection(sequence: number) {
		if (!this.state.projection)
			throw protocolError("event arrived before its snapshot", sequence);
		return this.state.projection;
	}

	private requireSequence(sequence: number) {
		if (this.state.eventSequence === null)
			throw protocolError("event arrived without a cursor", sequence);
		return this.state.eventSequence;
	}

	private assertNextRevision(
		revision: number,
		current: number,
		sequence: number,
	) {
		if (revision !== current + 1)
			throw protocolError("delta revision is not contiguous", sequence);
	}

	private publishProjection(
		projection: ProgrammerLifecycleProjection,
		eventSequence: number,
	) {
		this.state = {
			...this.state,
			projection,
			eventSequence,
			status: "ready",
			error: null,
			repairRequired: false,
		};
		this.emit();
	}

	private publishSessionState(
		update: Partial<ProgrammerLifecycleState>,
		expectedScope: number,
	) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		this.state = { ...this.state, ...update };
		this.emit();
		return true;
	}

	private markProtocolRepair(reason: unknown, sequence: number | null) {
		const error =
			reason instanceof ProgrammerLifecycleProtocolError
				? reason
				: new ProgrammerLifecycleProtocolError(String(reason), sequence);
		this.state = {
			...this.state,
			status: "error",
			error,
			repairRequired: true,
		};
		this.emit();
		return error;
	}

	private emit() {
		for (const listener of this.listeners) listener();
	}
}

function applyDelta(
	projection: ProgrammerLifecycleProjection,
	change: ProgrammerLifecycleChange,
) {
	const rows = [...projection.programmers];
	if (change.delta.type === "upsert") upsertRow(rows, change.delta.programmer);
	else removeRow(rows, change.delta.programmerId);
	return lifecycleProjectionFromCanonicalRows(change.revision, rows);
}

function upsertRow(
	rows: ProgrammerLifecycleRow[],
	row: ProgrammerLifecycleRow,
) {
	const index = rows.findIndex(
		(candidate) =>
			candidate.programmerId === row.programmerId ||
			candidate.userId === row.userId,
	);
	if (index >= 0) rows[index] = row;
	else rows.push(row);
}

function removeRow(rows: ProgrammerLifecycleRow[], programmerId: string) {
	const index = rows.findIndex((row) => row.programmerId === programmerId);
	if (index < 0)
		throw protocolError("delta removed an unknown Programmer", null);
	rows.splice(index, 1);
}

function emptyState(): ProgrammerLifecycleState {
	return {
		authorityKey: null,
		eventSequence: null,
		projection: null,
		status: "idle",
		error: null,
		repairRequired: false,
	};
}

function protocolError(message: string, sequence: number | null) {
	return new ProgrammerLifecycleProtocolError(
		`Programmer lifecycle ${message}`,
		sequence,
	);
}
