import type { ShowObject } from "./contracts";

type GroupObject = ShowObject<"group">;

function applyRule(
	fixtures: readonly string[],
	rule: NonNullable<GroupObject["body"]["derived_from"]>["rule"],
	): string[] | null {
	if (!["all", "odd", "even", "every_nth"].includes(rule.type)) return null;
	return fixtures.filter((_, index) => {
		const oneBased = index + 1;
		switch (rule.type) {
			case "all":
				return true;
			case "odd":
				return oneBased % 2 === 1;
			case "even":
				return oneBased % 2 === 0;
			case "every_nth": {
				const n = rule.n ?? 0;
				const offset = rule.offset ?? 0;
				return n > 0 && index >= offset && (index - offset) % n === 0;
			}
			default:
				return false;
		}
	});
}

/** Resolves live derived Group membership without changing frozen or stored source membership. */
export function projectLiveGroupMembership(groups: GroupObject[]): GroupObject[] {
	const byId = new Map(groups.map((group) => [group.id, group]));
	const resolved = new Map<string, string[]>();

	function resolve(id: string, visiting: Set<string>): string[] | null {
		const cached = resolved.get(id);
		if (cached) return cached;
		const group = byId.get(id);
		if (!group || visiting.has(id)) return null;
		const derived = group.body.derived_from;
		if (!derived) {
			const fixtures = [...group.body.fixtures];
			resolved.set(id, fixtures);
			return fixtures;
		}
		visiting.add(id);
		const source = resolve(derived.source_group_id, visiting);
		visiting.delete(id);
		if (!source) return null;
		const fixtures = applyRule(source, derived.rule);
		if (!fixtures) return null;
		resolved.set(id, fixtures);
		return fixtures;
	}

	return groups.map((group) => {
		if (!group.body.derived_from) return group;
		const fixtures = resolve(group.id, new Set());
		return fixtures
			? { ...group, body: { ...group.body, fixtures } }
			: group;
	});
}
