import type { HighlightActionRequest } from "../../../apps/light-desktop/src/api/generated/light-wire";
import { decodePlaybackSnapshot } from "../../../apps/light-desktop/src/api/playbackWire";
import { decodeProgrammingInteractionSnapshot } from "../../../apps/light-desktop/src/api/programmingWire";
import type {
	HighlightState,
	PatchedFixture,
	StoredGroup,
} from "../../../apps/light-desktop/src/api/types";
import type {
	SelectionExpression,
	SelectionReference,
} from "../../../apps/light-desktop/src/features/programmingInteraction/contracts";
import type { ApiDriver } from "../core/api";
import {
	gestureActiveProgrammingSelection,
	replaceActiveProgrammingSelection,
} from "./programmingSelection";
import {
	fixture,
	fixtureRange,
	group,
	groupRange,
	inclusiveSelectionNumbers,
	type SelectionPoint,
	type SelectionTarget,
	selectionRange,
} from "./selectionContract";
import {
	type IntentHttpDependencies,
	intentContextHeaders,
	intentFetch,
	intentRequestId,
	intentSession,
	intentUrl,
	responseJson,
} from "../core/v2IntentHttp";

interface GroupObject {
	id: string;
	body: StoredGroup;
}

interface SelectionAuthority {
	showId: string;
	patch: Awaited<ReturnType<ApiDriver["patch"]>>;
	groups: GroupObject[];
}

export interface NormalizedSelectionObservation {
	selected: readonly string[];
	targets: readonly ReturnType<typeof fixture>[];
	expression: SelectionExpression | null;
	revision: number;
	gestureOpen: boolean;
}

class SelectionItems {
	constructor(
		private readonly owner: BrowserSelection,
		private readonly kind: "fixture" | "group",
	) {}

	item(number: number) {
		return this.owner.targets(
			this.kind === "fixture" ? fixture(number) : group(number),
		);
	}

	items(...numbers: number[]) {
		return this.owner.targets(
			...numbers.map((number) =>
				this.kind === "fixture" ? fixture(number) : group(number),
			),
		);
	}

	range(first: number, last: number) {
		return this.owner.targets(
			this.kind === "fixture"
				? fixtureRange(first, last)
				: groupRange(first, last),
		);
	}
}

export class BrowserSelection {
	readonly fixtures = new SelectionItems(this, "fixture");
	readonly groups = new SelectionItems(this, "group");

	constructor(
		private readonly api: ApiDriver,
		private readonly dependencies: IntentHttpDependencies = {},
	) {}

	async targets(...targets: SelectionTarget[]) {
		await replaceActiveProgrammingSelection(
			this.api,
			{ surface: "api", fixtures: [] },
			this.dependencies,
		);
		return this.apply(false, targets);
	}

	add(...targets: SelectionTarget[]) {
		return this.apply(false, targets);
	}

	remove(...targets: SelectionTarget[]) {
		return this.apply(true, targets);
	}

	range(first: SelectionPoint, last: SelectionPoint) {
		return this.targets(selectionRange(first, last));
	}

	async clear() {
		await replaceActiveProgrammingSelection(
			this.api,
			{ surface: "api", fixtures: [] },
			this.dependencies,
		);
	}

	previous() {
		return this.step("previous");
	}

	next() {
		return this.step("next");
	}

	all() {
		return this.step("all");
	}

	async observe(): Promise<NormalizedSelectionObservation> {
		const authority = await this.authority();
		const session = intentSession(this.api);
		const path = "/api/v2/programming-interaction/snapshot";
		const response = await intentFetch(this.dependencies)(
			intentUrl(this.api, path),
			{
				headers: intentContextHeaders(session),
			},
		);
		const value = await responseJson(response, "Programming interaction");
		if (!response.ok)
			throw new Error(
				`Programming interaction returned HTTP ${response.status}`,
			);
		const projection = decodeProgrammingInteractionSnapshot(
			value,
			session.desk.id,
		).projection.selection;
		return {
			selected: projection.selected,
			targets: projection.selected.map((id) =>
				semanticFixture(authority.patch.fixtures, id),
			),
			expression: projection.expression,
			revision: projection.revision,
			gestureOpen: projection.gestureOpen,
		};
	}

	async expectSelection(...targets: SelectionTarget[]) {
		const authority = await this.authority();
		const expected = resolveTargets(authority, targets);
		const actual = await this.observe();
		if (!same(actual.selected, expected.fixtureIds))
			throw new Error(
				`Expected selection ${describeTargets(targets)}, received ${describeTargets(actual.targets)} (${actual.selected.join(", ")})`,
			);
		if (!sameReferences(actual.expression, expected.references))
			throw new Error(
				`Expected ordered selection sources ${JSON.stringify(expected.references)}, received ${JSON.stringify(actual.expression)}`,
			);
		return actual;
	}

	private async apply(remove: boolean, targets: readonly SelectionTarget[]) {
		const authority = await this.authority();
		const resolved = resolveTargets(authority, targets);
		for (const reference of resolved.actions)
			await gestureActiveProgrammingSelection(
				this.api,
				{
					surface: "api",
					source:
						reference.type === "fixture"
							? { type: "fixture", fixtureId: reference.id }
							: {
									type:
										reference.type === "group"
											? "live_group"
											: "dereferenced_group",
									groupId: reference.id,
								},
					remove,
				},
				this.dependencies,
			);
	}

	private async authority(): Promise<SelectionAuthority> {
		const session = intentSession(this.api);
		const fetch = intentFetch(this.dependencies);
		const response = await fetch(
			intentUrl(this.api, "/api/v2/playback-runtime/snapshot"),
			{
				method: "POST",
				headers: {
					...intentContextHeaders(session),
					"content-type": "application/json",
				},
				body: JSON.stringify({ identities: [] }),
			},
		);
		const value = await responseJson(response, "Playback runtime");
		if (!response.ok)
			throw new Error(`Playback runtime returned HTTP ${response.status}`);
		const playback = decodePlaybackSnapshot(value);
		const showId = playback.desk.scope.show_id;
		const [patch, groups] = await Promise.all([
			this.api.patch(),
			this.api.showObjects<StoredGroup>(showId, "group"),
		]);
		return { showId, patch, groups };
	}

	private async step(action: "previous" | "next" | "all") {
		const session = intentSession(this.api);
		const authority = await this.authority();
		const request: HighlightActionRequest = {
			request_id: intentRequestId(this.dependencies),
			action,
		};
		return this.api.request<HighlightState>(
			"POST",
			"/api/v2/output/highlight/actions",
			request,
			true,
			undefined,
			{ showId: authority.showId, deskId: session.desk.id },
		);
	}
}

interface ResolvedSelection {
	fixtureIds: string[];
	references: SelectionReference[];
	actions: Array<
		| { type: "fixture"; id: string }
		| { type: "group" | "dereferenced_group"; id: string }
	>;
}

function resolveTargets(
	authority: SelectionAuthority,
	targets: readonly SelectionTarget[],
): ResolvedSelection {
	const fixtureIds: string[] = [];
	const references: SelectionReference[] = [];
	const actions: ResolvedSelection["actions"] = [];
	const groups = new Map(
		authority.groups.map((candidate) => [candidate.id, candidate]),
	);
	for (const target of expandTargets(targets, groups)) {
		if (target.kind === "fixture") {
			for (const id of resolveFixture(authority.patch.fixtures, target)) {
				actions.push({ type: "fixture", id });
				references.push({ type: "fixture", fixtureId: id });
				pushUnique(fixtureIds, id);
			}
			continue;
		}
		const id = String(target.number);
		const stored = groups.get(id);
		if (!stored)
			throw new Error(`Group ${id} is not present in the active Show`);
		if (target.kind === "group") {
			actions.push({ type: "group", id });
			references.push({ type: "live_group", groupId: id });
		} else {
			actions.push({ type: "dereferenced_group", id });
			for (const fixtureId of stored.body.fixtures)
				references.push({ type: "fixture", fixtureId });
		}
		for (const fixtureId of stored.body.fixtures)
			pushUnique(fixtureIds, fixtureId);
	}
	return { fixtureIds, references, actions };
}

function expandTargets(
	targets: readonly SelectionTarget[],
	groups: ReadonlyMap<string, GroupObject>,
) {
	return targets.flatMap((target) => {
		if (target.kind === "fixture_range")
			return inclusiveSelectionNumbers(target.first, target.last).map(
				(number) => ({
					...fixture(number, target.head),
					rangeMember: target.head === undefined,
				}),
			);
		if (target.kind === "group_range")
			return inclusiveSelectionNumbers(target.first, target.last)
				.filter((number) => groups.has(String(number)))
				.map(group);
		return [target];
	});
}

function resolveFixture(
	fixtures: readonly PatchedFixture[],
	target: ReturnType<typeof fixture> & { rangeMember?: boolean },
) {
	const patched = fixtures.filter(
		(candidate) => candidate.fixture_number === target.number,
	);
	if (patched.length !== 1)
		throw new Error(
			`Fixture ${target.number} ${patched.length ? "is ambiguous" : "is not present in the active patch"}`,
		);
	const owner = patched[0];
	if (target.head === 0) return [owner.fixture_id];
	if (target.head !== undefined) {
		const head = owner.logical_heads.find(
			(candidate) => candidate.head_index === target.head,
		);
		if (!head)
			throw new Error(`Fixture ${target.number} has no head ${target.head}`);
		return [head.fixture_id];
	}
	if (target.rangeMember && owner.logical_heads.length)
		return owner.logical_heads.map((head) => head.fixture_id);
	return [
		owner.fixture_id,
		...owner.logical_heads.map((head) => head.fixture_id),
	];
}

function semanticFixture(
	fixtures: readonly PatchedFixture[],
	fixtureId: string,
) {
	for (const patched of fixtures) {
		const number = patched.fixture_number;
		if (number == null) continue;
		if (patched.fixture_id === fixtureId) return fixture(number, 0);
		const head = patched.logical_heads.find(
			(candidate) => candidate.fixture_id === fixtureId,
		);
		if (head) return fixture(number, head.head_index);
	}
	throw new Error(
		`Selected fixture ${fixtureId} is not present in the active patch`,
	);
}

function pushUnique(values: string[], value: string) {
	if (!values.includes(value)) values.push(value);
}

function same(first: readonly string[], second: readonly string[]) {
	return (
		first.length === second.length &&
		first.every((value, index) => value === second[index])
	);
}

function sameReferences(
	expression: SelectionExpression | null,
	expected: readonly SelectionReference[],
) {
	if (expected.length === 0)
		return (
			expression?.type === "static" ||
			(expression?.type === "sources" && expression.items.length === 0)
		);
	return (
		expression?.type === "sources" &&
		JSON.stringify(expression.items) === JSON.stringify(expected)
	);
}

function describeTargets(targets: readonly SelectionTarget[]) {
	return targets
		.map((target) =>
			target.kind === "fixture"
				? `${target.number}${target.head === undefined ? "" : `.${target.head}`}`
				: `${target.kind} ${"number" in target ? target.number : `${target.first}..${target.last}`}`,
		)
		.join(", ");
}
