import type {
	CommandLinePatch,
	CommandLineProjection,
	ProgrammingCapability,
	ProgrammingChange,
	ProgrammingSnapshot,
	SelectionProjection,
} from "./contracts";
import { sameValue } from "./projectionValue";
import { ProgrammingProtocolError } from "./transport";

export interface ProgrammingInteractionState {
	showId: string | null;
	deskId: string | null;
	eventSequence: number | null;
	commandLine: CommandLineProjection | null;
	selection: SelectionProjection | null;
	pendingCapabilities: ReadonlySet<ProgrammingCapability>;
	status: "idle" | "loading" | "ready" | "error";
	error: Error | null;
}

interface OperationBase {
	token: string;
	capability: ProgrammingCapability;
}

interface CommandLineOperation extends OperationBase {
	capability: "commandLine";
	patch: CommandLinePatch;
}

interface SelectionOperation extends OperationBase {
	capability: "selection";
	apply(current: SelectionProjection): SelectionProjection;
}

type OptimisticOperation = CommandLineOperation | SelectionOperation;
type InstallDecision = "ignore" | "same" | "install";

export class ProgrammingInteractionStore {
	private readonly listeners = new Set<() => void>();
	private readonly operations = new Map<string, OptimisticOperation>();
	private authoritativeCommandLine: CommandLineProjection | null = null;
	private authoritativeSelection: SelectionProjection | null = null;
	private scope = 0;
	private state: ProgrammingInteractionState = emptyState();

	readonly subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	readonly getSnapshot = () => this.state;

	reset(showId: string | null, deskId: string | null) {
		if (showId === this.state.showId && deskId === this.state.deskId) return;
		this.scope++;
		this.authoritativeCommandLine = null;
		this.authoritativeSelection = null;
		this.operations.clear();
		this.state = {
			...emptyState(),
			showId,
			deskId,
			status: showId && deskId ? "loading" : "idle",
		};
		this.emit();
	}

	installSnapshot(
		snapshot: ProgrammingSnapshot,
		{
			updateSessionState = true,
			expectedScope = this.scope,
		}: { updateSessionState?: boolean; expectedScope?: number } = {},
	) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		if (!this.matchesDesk(snapshot.projection.deskId)) return false;
		const sequence = snapshot.cursor;
		const commandDecision = this.installDecision(
			"command line",
			this.authoritativeCommandLine,
			snapshot.projection.commandLine,
			sequence,
		);
		const selectionDecision = this.installDecision(
			"selection",
			this.authoritativeSelection,
			snapshot.projection.selection,
			sequence,
		);
		if (commandDecision === "install")
			this.authoritativeCommandLine = snapshot.projection.commandLine;
		if (selectionDecision === "install")
			this.authoritativeSelection = snapshot.projection.selection;
		this.publishAuthoritative(sequence, updateSessionState);
		return true;
	}

	applyChange(
		change: ProgrammingChange,
		sequence: number,
		expectedScope = this.scope,
	) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		if (!this.matchesDesk(change.deskId)) return false;
		const commandDecision =
			"commandLine" in change
				? this.installDecision(
						"command line",
						this.authoritativeCommandLine,
						change.commandLine,
						sequence,
					)
				: "same";
		const selectionDecision =
			"selection" in change
				? this.installDecision(
						"selection",
						this.authoritativeSelection,
						change.selection,
						sequence,
					)
				: "same";
		if (commandDecision === "install" && "commandLine" in change)
			this.authoritativeCommandLine = change.commandLine;
		if (selectionDecision === "install" && "selection" in change)
			this.authoritativeSelection = change.selection;
		this.publishAuthoritative(sequence);
		return true;
	}

	beginOptimisticCommandLine(
		patch: CommandLinePatch,
		expectedScope = this.scope,
	) {
		if (!this.isScopeCurrent(expectedScope)) return null;
		const current = this.renderCommandLine();
		if (!current) return null;
		const normalized = definedCommandLinePatch(patch);
		if (Object.keys(normalized).length === 0) return null;
		const optimistic = { ...current, ...normalized };
		if (sameValue(current, optimistic)) return null;
		const operation: CommandLineOperation = {
			token: crypto.randomUUID(),
			capability: "commandLine",
			patch: normalized,
		};
		this.operations.set(operation.token, operation);
		this.publishRendered();
		return operation.token;
	}

	beginOptimisticSelectionUpdate(
		update: (current: SelectionProjection) => SelectionProjection,
		expectedScope = this.scope,
	) {
		if (!this.isScopeCurrent(expectedScope)) return null;
		const current = this.renderSelection();
		if (!current) return null;
		const apply = (selection: SelectionProjection) => ({
			...update(selection),
			revision: selection.revision,
		});
		// Validate the reducer before registering it. A malformed optimistic
		// operation must never poison every later render or stream event.
		apply(current);
		const operation: SelectionOperation = {
			token: crypto.randomUUID(),
			capability: "selection",
			apply,
		};
		this.operations.set(operation.token, operation);
		this.publishRendered();
		return operation.token;
	}

	commit(token: string | null, expectedScope = this.scope) {
		if (!this.takeOperation(token, expectedScope)) return false;
		this.publishRendered();
		return true;
	}

	commitCommandLine(
		token: string | null,
		commandLine: CommandLineProjection,
		expectedScope = this.scope,
	) {
		if (!token || !this.isScopeCurrent(expectedScope)) return false;
		const operation = this.operations.get(token);
		if (operation?.capability !== "commandLine") return false;
		const decision = this.installDecision(
			"command line",
			this.authoritativeCommandLine,
			commandLine,
			this.state.eventSequence ?? 0,
		);
		if (decision === "install") this.authoritativeCommandLine = commandLine;
		this.operations.delete(token);
		this.publishRendered();
		return true;
	}

	commitSelection(
		token: string | null,
		selection: SelectionProjection,
		expectedScope = this.scope,
	) {
		if (!token || !this.isScopeCurrent(expectedScope)) return false;
		const operation = this.operations.get(token);
		if (operation?.capability !== "selection") return false;
		const decision = this.installDecision(
			"selection",
			this.authoritativeSelection,
			selection,
			this.state.eventSequence ?? 0,
		);
		if (decision === "install") this.authoritativeSelection = selection;
		this.operations.delete(token);
		this.publishRendered();
		return true;
	}

	authoritativeCommandLineRevision(expectedScope = this.scope) {
		if (!this.isScopeCurrent(expectedScope)) return null;
		return this.authoritativeCommandLine?.revision ?? null;
	}

	authoritativeSelectionRevision(expectedScope = this.scope) {
		if (!this.isScopeCurrent(expectedScope)) return null;
		return this.authoritativeSelection?.revision ?? null;
	}

	captureScope() {
		return this.scope;
	}

	isScopeCurrent(scope: number) {
		return scope === this.scope;
	}

	installSelectionRepair(
		token: string,
		scope: number,
		snapshot: ProgrammingSnapshot,
	) {
		if (!this.hasOperation(token, "selection", scope)) return false;
		if (!this.installSnapshot(snapshot, { updateSessionState: false, expectedScope: scope }))
			return false;
		return this.commit(token, scope);
	}

	rollback(token: string | null, _error: Error, expectedScope = this.scope) {
		if (!this.takeOperation(token, expectedScope)) return false;
		this.publishRendered();
		return true;
	}

	setLoading(expectedScope = this.scope) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		if (this.state.status !== "loading") this.publishRendered({ status: "loading" });
		return true;
	}

	setReady(expectedScope = this.scope) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		this.publishRendered({ status: "ready", error: null });
		return true;
	}

	setError(error: Error, expectedScope = this.scope) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		this.publishRendered({ status: "error", error });
		return true;
	}

	private installDecision<T extends { revision: number }>(
		label: string,
		current: T | null,
		incoming: T,
		sequence: number,
	): InstallDecision {
		if (!current || incoming.revision > current.revision) return "install";
		if (incoming.revision < current.revision) return "ignore";
		if (sameValue(current, incoming)) return "same";
		throw new ProgrammingProtocolError(
			`Conflicting ${label} projections at revision ${incoming.revision}`,
			sequence,
		);
	}

	private takeOperation(token: string | null, expectedScope = this.scope) {
		if (!token || !this.isScopeCurrent(expectedScope)) return null;
		const operation = this.operations.get(token) ?? null;
		if (operation) this.operations.delete(token);
		return operation;
	}

	private hasOperation(
		token: string,
		capability: ProgrammingCapability,
		expectedScope: number,
	) {
		return (
			this.isScopeCurrent(expectedScope) &&
			this.operations.get(token)?.capability === capability
		);
	}

	private publishAuthoritative(
		sequence: number,
		updateSessionState = true,
	) {
		const update: Partial<ProgrammingInteractionState> = {
			eventSequence: Math.max(this.state.eventSequence ?? 0, sequence),
		};
		if (updateSessionState) {
			update.status = "ready";
			update.error = null;
		}
		this.publishRendered(update);
	}

	private publishRendered(update: Partial<ProgrammingInteractionState> = {}) {
		this.state = {
			...this.state,
			commandLine: this.renderCommandLine(),
			selection: this.renderSelection(),
			pendingCapabilities: new Set(
				[...this.operations.values()].map(({ capability }) => capability),
			),
			...update,
		};
		this.emit();
	}

	private renderCommandLine() {
		let projection = this.authoritativeCommandLine;
		if (!projection) return null;
		for (const operation of this.operations.values())
			if (operation.capability === "commandLine")
				projection = { ...projection, ...operation.patch };
		return projection;
	}

	private renderSelection() {
		let projection = this.authoritativeSelection;
		if (!projection) return null;
		for (const operation of this.operations.values())
			if (operation.capability === "selection")
				projection = operation.apply(projection);
		return projection;
	}

	private matchesDesk(deskId: string) {
		return Boolean(this.state.showId) && deskId === this.state.deskId;
	}

	private emit() {
		for (const listener of this.listeners) listener();
	}
}

function emptyState(): ProgrammingInteractionState {
	return {
		showId: null,
		deskId: null,
		eventSequence: null,
		commandLine: null,
		selection: null,
		pendingCapabilities: new Set(),
		status: "idle",
		error: null,
	};
}

function definedCommandLinePatch(patch: CommandLinePatch): CommandLinePatch {
	return Object.fromEntries(
		Object.entries(patch).filter(([, value]) => value !== undefined),
	) as CommandLinePatch;
}
