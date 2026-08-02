import type {
	GroupFixtureSource,
	GroupReference,
	GroupSelectionRule,
	StoredGroup,
} from "../../api/types";
import type { ShowObject } from "./contracts";

type GroupObject = ShowObject<"group">;

function applyRule(
	fixtures: readonly string[],
	rule: GroupSelectionRule,
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

/**
 * Returns the canonical source when it has a supported, structurally valid representation.
 * Unknown future shapes deliberately fall through to the tolerant legacy compatibility fields.
 */
export function canonicalGroupSource(
	body: StoredGroup,
): GroupFixtureSource | null {
	const source: unknown = body.source;
	if (!source || typeof source !== "object") return null;
	const record = source as Record<string, unknown>;
	if (record.type === "explicit" && isStringArray(record.fixture_ids))
		return { type: "explicit", fixture_ids: record.fixture_ids };
	if (record.type !== "references" || !Array.isArray(record.references))
		return null;
	const references: GroupReference[] = [];
	for (const candidate of record.references) {
		const reference = canonicalReference(candidate);
		if (!reference) return null;
		references.push(reference);
	}
	return { type: "references", references };
}

/** Canonical references take precedence; the single legacy derivation remains read-only fallback. */
export function groupReferenceIds(body: StoredGroup): string[] {
	const source = canonicalGroupSource(body);
	if (source?.type === "explicit") return [];
	if (source?.type === "references")
		return source.references.map((reference) => reference.group_id);
	return body.derived_from?.source_group_id
		? [body.derived_from.source_group_id]
		: [];
}

/** Resolves canonical and tolerant legacy membership for desktop readers without ranking it. */
export function resolveGroupMembership(
	groups: readonly Pick<GroupObject, "id" | "body">[],
): ReadonlyMap<string, readonly string[]> {
	const byId = new Map(groups.map((group) => [group.id, group]));
	const resolved = new Map<string, string[]>();
	const unavailable = new Set<string>();
	const projected = new Map<string, string[]>();

	function resolve(id: string, visiting: Set<string>): string[] | null {
		if (unavailable.has(id)) return null;
		const cached = resolved.get(id);
		if (cached !== undefined) return cached;
		const group = byId.get(id);
		if (!group || visiting.has(id)) return null;
		const source = canonicalGroupSource(group.body);
		if (source?.type === "explicit") {
			const fixtures = deduplicate(source.fixture_ids);
			resolved.set(id, fixtures);
			return fixtures;
		}
		if (source?.type === "references") {
			visiting.add(id);
			const fixtures: string[] = [];
			for (const reference of source.references) {
				const referenced = resolve(reference.group_id, visiting);
				if (!referenced) {
					visiting.delete(id);
					unavailable.add(id);
					return null;
				}
				const selected = applyRule(referenced, reference.rule);
				if (!selected) {
					visiting.delete(id);
					unavailable.add(id);
					return null;
				}
				fixtures.push(...selected);
			}
			visiting.delete(id);
			const unique = deduplicate(fixtures);
			resolved.set(id, unique);
			return unique;
		}
		if (hasCanonicalSourceField(group.body)) {
			unavailable.add(id);
			return null;
		}
		const derived = legacyReference(group.body);
		if (!derived) {
			const fixtures = deduplicate(group.body.fixtures);
			resolved.set(id, fixtures);
			return fixtures;
		}
		visiting.add(id);
		const referenced = resolve(derived.group_id, visiting);
		visiting.delete(id);
		if (!referenced) {
			unavailable.add(id);
			return null;
		}
		const fixtures = applyRule(referenced, derived.rule);
		if (!fixtures) {
			unavailable.add(id);
			return null;
		}
		const unique = deduplicate(fixtures);
		resolved.set(id, unique);
		return unique;
	}

	for (const group of groups) {
		const fixtures = resolve(group.id, new Set());
		// A present canonical source remains authoritative even when it is malformed, cyclic, or
		// references an unavailable Group. Its desktop membership is empty until authority recovers;
		// only source-less legacy bodies may use the decoded compatibility cache.
		projected.set(
			group.id,
			fixtures ??
				(hasCanonicalSourceField(group.body)
					? []
					: deduplicate(group.body.fixtures)),
		);
	}
	return projected;
}

/** Resolves live Group membership without changing canonical source, mapping, or frozen metadata. */
export function projectLiveGroupMembership(
	groups: GroupObject[],
): GroupObject[] {
	const membership = resolveGroupMembership(groups);
	return groups.map((group) => {
		const fixtures = [...(membership.get(group.id) ?? group.body.fixtures)];
		return sameValues(fixtures, group.body.fixtures)
			? group
			: { ...group, body: { ...group.body, fixtures } };
	});
}

function canonicalReference(value: unknown): GroupReference | null {
	if (!value || typeof value !== "object") return null;
	const record = value as Record<string, unknown>;
	if (typeof record.group_id !== "string") return null;
	const rule = canonicalRule(record.rule);
	return rule ? { group_id: record.group_id, rule } : null;
}

function canonicalRule(value: unknown): GroupSelectionRule | null {
	if (!value || typeof value !== "object") return null;
	const rule = value as Record<string, unknown>;
	if (rule.type === "all" || rule.type === "odd" || rule.type === "even")
		return { type: rule.type };
	if (
		rule.type === "every_nth" &&
		Number.isInteger(rule.n) &&
		(rule.n as number) > 0 &&
		Number.isInteger(rule.offset) &&
		(rule.offset as number) >= 0
	)
		return {
			type: "every_nth",
			n: rule.n as number,
			offset: rule.offset as number,
		};
	return null;
}

function legacyReference(body: StoredGroup): GroupReference | null {
	const derived = body.derived_from;
	if (!derived) return null;
	const rule = canonicalRule(derived.rule);
	return rule ? { group_id: derived.source_group_id, rule } : null;
}

function isStringArray(value: unknown): value is string[] {
	return (
		Array.isArray(value) && value.every((item) => typeof item === "string")
	);
}

function hasCanonicalSourceField(body: StoredGroup) {
	return body.source !== undefined && body.source !== null;
}

function deduplicate(values: readonly string[]) {
	return [...new Set(values)];
}

function sameValues(left: readonly string[], right: readonly string[]) {
	return (
		left.length === right.length &&
		left.every((value, index) => value === right[index])
	);
}
