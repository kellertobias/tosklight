import {
	oscProgrammerActionForKey,
	type SoftwareKey,
} from "@tosklight/ui/programmer-keypad";
import {
	type fixture,
	group,
	inclusiveSelectionNumbers,
	type SelectionTarget,
} from "./selectionContract";

export type SelectionMutation = "replace" | "add" | "remove";
export type SelectionKeyPhase = "press" | "release";

export interface SelectionTransportAuthority {
	defaultTarget: "FIXTURE" | "GROUP";
	/** Stored Group numbers. When omitted, the server remains the range authority. */
	groupNumbers?: readonly number[];
}

export interface LogicalSelectionKeyEvent {
	key: SoftwareKey;
	phase: SelectionKeyPhase;
}

export interface OscSelectionKeyEvent extends LogicalSelectionKeyEvent {
	address: string;
	arguments: readonly [pressed: boolean];
}

export interface LogicalKeySink {
	send(key: SoftwareKey, phase: SelectionKeyPhase): Promise<void>;
}

export interface OscProgrammerSink {
	send(address: string, arguments_: readonly [pressed: boolean]): Promise<void>;
}

export function selectionKeys(
	mutation: SelectionMutation,
	targets: readonly SelectionTarget[],
	authority: SelectionTransportAuthority,
): SoftwareKey[] {
	const terms = expandTerms(targets, authority);
	if (terms.length === 0)
		throw new Error(
			"Selection transport requires at least one supported target",
		);
	const keys: SoftwareKey[] = [];
	if (mutation === "add") keys.push("+");
	if (mutation === "remove") keys.push("-");
	for (const [index, term] of terms.entries()) {
		if (index > 0) keys.push("+");
		keys.push(...termKeys(term, authority.defaultTarget));
	}
	keys.push("ENT");
	return keys;
}

export function logicalSelectionEvents(
	mutation: SelectionMutation,
	targets: readonly SelectionTarget[],
	authority: SelectionTransportAuthority,
): LogicalSelectionKeyEvent[] {
	return selectionKeys(mutation, targets, authority).flatMap((key) => [
		{ key, phase: "press" },
		{ key, phase: "release" },
	]);
}

export function oscSelectionEvents(
	deskAlias: string,
	mutation: SelectionMutation,
	targets: readonly SelectionTarget[],
	authority: SelectionTransportAuthority,
): OscSelectionKeyEvent[] {
	if (!validDeskAlias(deskAlias))
		throw new Error(
			"OSC selection requires a non-empty desk alias without slashes",
		);
	return logicalSelectionEvents(mutation, targets, authority).map((event) => ({
		...event,
		address: `/light/${deskAlias}/programmer/${oscProgrammerActionForKey(event.key)}`,
		arguments: [event.phase === "press"],
	}));
}

export class KeypadSelectionTransport {
	constructor(
		private readonly sink: LogicalKeySink,
		private readonly authority: () => Promise<SelectionTransportAuthority>,
	) {}

	targets(...targets: SelectionTarget[]) {
		return this.apply("replace", targets);
	}

	add(...targets: SelectionTarget[]) {
		return this.apply("add", targets);
	}

	remove(...targets: SelectionTarget[]) {
		return this.apply("remove", targets);
	}

	private async apply(
		mutation: SelectionMutation,
		targets: readonly SelectionTarget[],
	) {
		const events = logicalSelectionEvents(
			mutation,
			targets,
			await this.authority(),
		);
		for (const event of events) await this.sink.send(event.key, event.phase);
	}
}

export class OscSelectionTransport {
	constructor(
		private readonly deskAlias: string,
		private readonly sink: OscProgrammerSink,
		private readonly authority: () => Promise<SelectionTransportAuthority>,
	) {}

	targets(...targets: SelectionTarget[]) {
		return this.apply("replace", targets);
	}

	add(...targets: SelectionTarget[]) {
		return this.apply("add", targets);
	}

	remove(...targets: SelectionTarget[]) {
		return this.apply("remove", targets);
	}

	private async apply(
		mutation: SelectionMutation,
		targets: readonly SelectionTarget[],
	) {
		const events = oscSelectionEvents(
			this.deskAlias,
			mutation,
			targets,
			await this.authority(),
		);
		for (const event of events)
			await this.sink.send(event.address, event.arguments);
	}
}

type SelectionTerm =
	| ReturnType<typeof fixture>
	| ReturnType<typeof group>
	| {
			kind: "fixture_range";
			first: number;
			last: number;
			head?: number;
	  }
	| { kind: "group_range"; first: number; last: number }
	| { kind: "dereferenced_group"; number: number };

function expandTerms(
	targets: readonly SelectionTarget[],
	authority: SelectionTransportAuthority,
): SelectionTerm[] {
	const knownGroups =
		authority.groupNumbers === undefined
			? undefined
			: new Set(authority.groupNumbers);
	const terms: SelectionTerm[] = [];
	for (const target of targets) {
		switch (target.kind) {
			case "fixture":
			case "fixture_range":
				terms.push(target);
				break;
			case "group":
			case "dereferenced_group":
				if (knownGroups && !knownGroups.has(target.number))
					throw new Error(
						`Group ${target.number} is not present in the selection authority`,
					);
				terms.push(target);
				break;
			case "group_range":
				if (!knownGroups) {
					terms.push(target);
					break;
				}
				terms.push(
					...inclusiveSelectionNumbers(target.first, target.last)
						.filter((number) => knownGroups.has(number))
						.map(group),
				);
				break;
			default:
				unsupportedTarget(target);
		}
	}
	return terms;
}

function termKeys(
	term: SelectionTerm,
	defaultTarget: SelectionTransportAuthority["defaultTarget"],
): SoftwareKey[] {
	switch (term.kind) {
		case "fixture":
			return [
				...targetPrefix("FIXTURE", defaultTarget),
				...fixtureAddress(term.number, term.head),
			];
		case "fixture_range":
			return [
				...targetPrefix("FIXTURE", defaultTarget),
				...fixtureAddress(term.first, term.head),
				"TRU",
				...fixtureAddress(term.last, term.head),
			];
		case "group":
			return [...targetPrefix("GROUP", defaultTarget), ...digits(term.number)];
		case "group_range":
			return [
				...targetPrefix("GROUP", defaultTarget),
				...digits(term.first),
				"TRU",
				...digits(term.last),
			];
		case "dereferenced_group":
			return ["GRP", "GRP", ...digits(term.number)];
	}
}

function targetPrefix(
	target: "FIXTURE" | "GROUP",
	defaultTarget: "FIXTURE" | "GROUP",
): SoftwareKey[] {
	return target === defaultTarget ? [] : ["GRP"];
}

function fixtureAddress(number: number, head?: number): SoftwareKey[] {
	return head === undefined
		? digits(number)
		: [...digits(number), ".", ...digits(head)];
}

function digits(value: number): SoftwareKey[] {
	if (!Number.isSafeInteger(value) || value < 0)
		throw new Error(
			`Selection address ${value} must be a non-negative safe integer`,
		);
	return [...String(value)] as SoftwareKey[];
}

function validDeskAlias(value: string) {
	return value.trim().length > 0 && !value.includes("/");
}

function unsupportedTarget(value: never): never {
	throw new Error(
		`Unsupported semantic selection target ${JSON.stringify(value)}`,
	);
}
