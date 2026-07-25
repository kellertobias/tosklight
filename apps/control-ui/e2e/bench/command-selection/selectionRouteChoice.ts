import type { SelectionTarget } from "./selectionContract";

export type SelectionRouteName =
	| "ui"
	| "touch"
	| "api"
	| "osc"
	| "keypad"
	| "fixtureSheet"
	| "stage"
	| "pool"
	| "click"
	| "shiftClick";

export type SelectionMutation = "replace" | "add" | "remove";
export type SelectionTargetKind = SelectionTarget["kind"];

export interface SelectionRouteCapabilities {
	actions: readonly SelectionMutation[];
	targets: readonly SelectionTargetKind[];
}

export interface SelectionRouteRequest {
	action: SelectionMutation;
	targets: readonly SelectionTarget[];
}

export interface SelectionRouteAdapter {
	name: SelectionRouteName;
	capabilities: SelectionRouteCapabilities;
	mutate(request: SelectionRouteRequest): Promise<unknown>;
}

export interface SelectionRouteChoiceReport {
	seed: string;
	actionIndex: number;
	action: SelectionMutation;
	targetKinds: readonly SelectionTargetKind[];
	candidates: readonly SelectionRouteName[];
	selected: SelectionRouteName;
}

export interface SelectionRouteChoiceOutcome {
	report: SelectionRouteChoiceReport;
	value: unknown;
}

/**
 * Chooses only truthful adapters for unqualified selection actions. Candidate order is
 * canonical, so a seed and action index replay identically regardless of registration order.
 */
export class SeededSelectionRouteChoice {
	private nextActionIndex = 0;
	private readonly adapters: ReadonlyMap<
		SelectionRouteName,
		SelectionRouteAdapter
	>;
	readonly reports: SelectionRouteChoiceReport[] = [];
	readonly seed: string;

	constructor(
		seed: string | number,
		adapters: readonly SelectionRouteAdapter[],
	) {
		this.seed = normalizeSeed(seed);
		this.adapters = uniqueAdapters(adapters);
	}

	async execute(
		request: SelectionRouteRequest,
	): Promise<SelectionRouteChoiceOutcome> {
		const actionIndex = this.nextActionIndex;
		const report = this.choose(request, actionIndex);
		const adapter = this.adapter(report.selected);
		this.nextActionIndex += 1;
		this.reports.push(report);
		return { report, value: await adapter.mutate(request) };
	}

	/**
	 * Replays a recorded choice after proving the seed, action, target kinds, candidates,
	 * and deterministic selection still match. Validation completes before mutation.
	 */
	async replay(
		report: SelectionRouteChoiceReport,
		request: SelectionRouteRequest,
	): Promise<SelectionRouteChoiceOutcome> {
		if (report.seed !== this.seed)
			throw new Error(
				`Selection route replay seed ${report.seed} does not match ${this.seed}`,
			);
		const current = this.choose(request, report.actionIndex);
		if (!sameReport(current, report))
			throw new Error(
				`Selection route replay does not match the recorded choice: expected ${JSON.stringify(report)}, received ${JSON.stringify(current)}`,
			);
		return {
			report: current,
			value: await this.adapter(current.selected).mutate(request),
		};
	}

	private choose(
		request: SelectionRouteRequest,
		actionIndex: number,
	): SelectionRouteChoiceReport {
		assertRequest(request);
		const targetKinds = request.targets.map((target) => target.kind);
		const candidates = [...this.adapters.values()]
			.filter((adapter) => supports(adapter.capabilities, request))
			.map((adapter) => adapter.name)
			.sort();
		if (candidates.length === 0)
			throw new UnsupportedSelectionRouteError(request, [
				...this.adapters.keys(),
			]);
		const selected =
			candidates[seededIndex(this.seed, actionIndex, candidates.length)];
		return {
			seed: this.seed,
			actionIndex,
			action: request.action,
			targetKinds,
			candidates,
			selected,
		};
	}

	private adapter(name: SelectionRouteName): SelectionRouteAdapter {
		const adapter = this.adapters.get(name);
		if (!adapter) throw new Error(`Selection route ${name} is not registered`);
		return adapter;
	}
}

export class UnsupportedSelectionRouteError extends Error {
	readonly name = "UnsupportedSelectionRouteError";

	constructor(
		readonly request: SelectionRouteRequest,
		readonly registered: readonly SelectionRouteName[],
	) {
		super(
			`No selection route supports ${request.action} for ${request.targets.map((target) => target.kind).join(", ") || "an empty target list"}`,
		);
	}
}

function supports(
	capabilities: SelectionRouteCapabilities,
	request: SelectionRouteRequest,
) {
	return (
		capabilities.actions.includes(request.action) &&
		request.targets.every((target) =>
			capabilities.targets.includes(target.kind),
		)
	);
}

function assertRequest(request: SelectionRouteRequest) {
	if (request.targets.length === 0)
		throw new Error("Selection route choice requires at least one target");
}

function uniqueAdapters(adapters: readonly SelectionRouteAdapter[]) {
	const indexed = new Map<SelectionRouteName, SelectionRouteAdapter>();
	for (const adapter of adapters) {
		if (indexed.has(adapter.name))
			throw new Error(`Selection route ${adapter.name} is registered twice`);
		indexed.set(adapter.name, adapter);
	}
	return indexed;
}

function normalizeSeed(seed: string | number) {
	const normalized = String(seed).trim();
	if (!normalized) throw new Error("Selection route seed must not be empty");
	return normalized;
}

function seededIndex(seed: string, actionIndex: number, length: number) {
	let hash = 0x811c9dc5;
	for (const character of `${seed}:${actionIndex}`) {
		hash ^= character.codePointAt(0) ?? 0;
		hash = Math.imul(hash, 0x01000193);
	}
	return (hash >>> 0) % length;
}

function sameReport(
	left: SelectionRouteChoiceReport,
	right: SelectionRouteChoiceReport,
) {
	return (
		left.seed === right.seed &&
		left.actionIndex === right.actionIndex &&
		left.action === right.action &&
		left.selected === right.selected &&
		same(left.targetKinds, right.targetKinds) &&
		same(left.candidates, right.candidates)
	);
}

function same<T>(left: readonly T[], right: readonly T[]) {
	return (
		left.length === right.length &&
		left.every((value, index) => value === right[index])
	);
}
