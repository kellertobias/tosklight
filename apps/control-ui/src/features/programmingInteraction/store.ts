import type {
	CommandLinePatch,
	CommandLineProjection,
	ProgrammingCapability,
	ProgrammingChange,
	ProgrammingSnapshot,
	SelectionPatch,
	SelectionProjection,
} from "./contracts";
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
	selected: readonly string[];
	hasExpression: boolean;
	expression: SelectionProjection["expression"];
}

type OptimisticOperation = CommandLineOperation | SelectionOperation;
type InstallDecision = "ignore" | "same" | "install";

export class ProgrammingInteractionStore {
	private readonly listeners = new Set<() => void>();
	private readonly operations = new Map<string, OptimisticOperation>();
	private authoritativeCommandLine: CommandLineProjection | null = null;
	private authoritativeSelection: SelectionProjection | null = null;
	private state: ProgrammingInteractionState = emptyState();

	readonly subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	readonly getSnapshot = () => this.state;

	reset(showId: string | null, deskId: string | null) {
		if (showId === this.state.showId && deskId === this.state.deskId) return;
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
		{ updateSessionState = true }: { updateSessionState?: boolean } = {},
	) {
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

	applyChange(change: ProgrammingChange, sequence: number) {
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

	beginOptimisticCommandLine(patch: CommandLinePatch) {
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

	beginOptimisticSelection(patch: SelectionPatch) {
		const current = this.renderSelection();
		if (!current) return null;
		const hasExpression = Object.hasOwn(patch, "expression");
		const optimistic: SelectionProjection = {
			...current,
			selected: [...patch.selected],
			...(hasExpression ? { expression: patch.expression ?? null } : {}),
		};
		if (sameValue(current, optimistic)) return null;
		const operation: SelectionOperation = {
			token: crypto.randomUUID(),
			capability: "selection",
			selected: [...patch.selected],
			hasExpression,
			expression: patch.expression ?? null,
		};
		this.operations.set(operation.token, operation);
		this.publishRendered();
		return operation.token;
	}

	commit(token: string | null) {
		if (!this.takeOperation(token)) return false;
		this.publishRendered();
		return true;
	}

	commitCommandLine(token: string | null, commandLine: CommandLineProjection) {
		if (!token) return false;
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

	authoritativeCommandLineRevision() {
		return this.authoritativeCommandLine?.revision ?? null;
	}

	rollback(token: string | null, _error: Error) {
		if (!this.takeOperation(token)) return false;
		this.publishRendered();
		return true;
	}

	setLoading() {
		if (this.state.status !== "loading") this.publishRendered({ status: "loading" });
	}

	setReady() {
		this.publishRendered({ status: "ready", error: null });
	}

	setError(error: Error) {
		this.publishRendered({ status: "error", error });
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

	private takeOperation(token: string | null) {
		if (!token) return null;
		const operation = this.operations.get(token) ?? null;
		if (operation) this.operations.delete(token);
		return operation;
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
				projection = {
					...projection,
					selected: [...operation.selected],
					...(operation.hasExpression
						? { expression: operation.expression }
						: {}),
				};
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

function sameValue(left: unknown, right: unknown): boolean {
	if (Object.is(left, right)) return true;
	if (Array.isArray(left) || Array.isArray(right))
		return (
			Array.isArray(left) &&
			Array.isArray(right) &&
			left.length === right.length &&
			left.every((value, index) => sameValue(value, right[index]))
		);
	if (!isRecord(left) || !isRecord(right)) return false;
	const leftKeys = Object.keys(left).sort();
	const rightKeys = Object.keys(right).sort();
	return (
		leftKeys.length === rightKeys.length &&
		leftKeys.every(
			(key, index) =>
				key === rightKeys[index] && sameValue(left[key], right[key]),
		)
	);
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}
