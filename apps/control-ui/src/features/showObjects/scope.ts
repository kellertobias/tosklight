import type { ShowObjectKind, ShowObjectsChange } from "./contracts";
import type {
	ShowObjectEventIdentity,
	ShowObjectsEventScope,
} from "./transport";

export interface HydrationTarget {
	kind: ShowObjectKind;
	objectId?: string;
}

export class ShowObjectsViewScope {
	private readonly kinds = new Map<ShowObjectKind, number>();
	private readonly objects = new Map<ShowObjectKind, Map<string, number>>();
	private readonly groupDependencies = new Map<string, Set<string>>();

	activate(kind: ShowObjectKind, objectId?: string) {
		if (objectId === undefined) {
			this.kinds.set(kind, (this.kinds.get(kind) ?? 0) + 1);
			return;
		}
		const identities = this.objects.get(kind) ?? new Map<string, number>();
		identities.set(objectId, (identities.get(objectId) ?? 0) + 1);
		this.objects.set(kind, identities);
	}

	deactivate(kind: ShowObjectKind, objectId?: string) {
		if (objectId === undefined) {
			const count = this.kinds.get(kind) ?? 0;
			if (count > 1) this.kinds.set(kind, count - 1);
			else this.kinds.delete(kind);
			return count <= 1;
		}
		const identities = this.objects.get(kind);
		const count = identities?.get(objectId) ?? 0;
		if (count > 1) identities?.set(objectId, count - 1);
		else identities?.delete(objectId);
		if (!identities?.size) this.objects.delete(kind);
		if (kind === "group" && count <= 1)
			this.groupDependencies.delete(objectId);
		return count <= 1;
	}

	clear() {
		this.kinds.clear();
		this.objects.clear();
		this.groupDependencies.clear();
	}

	hasViews() {
		return this.kinds.size > 0 || this.objects.size > 0;
	}

	hasViewKind(kind: ShowObjectKind) {
		return this.kinds.has(kind) || this.objects.has(kind);
	}

	isTargetActive(target: HydrationTarget) {
		return target.objectId === undefined
			? this.kinds.has(target.kind)
			: this.objects.get(target.kind)?.has(target.objectId) === true;
	}

	targets(): HydrationTarget[] {
		const targets: HydrationTarget[] = [...this.kinds.keys()].map((kind) => ({
			kind,
		}));
		for (const [kind, identities] of this.objects) {
			if (this.kinds.has(kind)) continue;
			for (const objectId of identities.keys()) targets.push({ kind, objectId });
		}
		return targets;
	}

	includesChange(change: ShowObjectsChange["changes"][number]) {
		return (
			this.kinds.has(change.kind) ||
			this.objects.get(change.kind)?.has(change.objectId) === true ||
			(change.kind === "group" && this.isGroupDependency(change.objectId))
		);
	}

	setGroupDependencies(targetId: string, dependencies: Iterable<string>) {
		if (this.objects.get("group")?.has(targetId) !== true) return;
		const next = new Set([...dependencies].filter((id) => id !== targetId));
		if (sameValues(this.groupDependencies.get(targetId), next)) return;
		this.groupDependencies.set(targetId, next);
	}

	affectedExactGroups(changedIds: ReadonlySet<string>) {
		const affected: string[] = [];
		for (const target of this.objects.get("group")?.keys() ?? []) {
			if (
				changedIds.has(target) ||
				[...(this.groupDependencies.get(target) ?? [])].some((id) =>
					changedIds.has(id),
				)
			)
				affected.push(target);
		}
		return affected;
	}

	subscription(): ShowObjectsEventScope {
		const kinds = [...this.kinds.keys()].sort();
		const objects = new Map<string, ShowObjectEventIdentity>();
		for (const [kind, identities] of this.objects) {
			if (this.kinds.has(kind)) continue;
			for (const objectId of identities.keys())
				objects.set(identityKey(kind, objectId), { kind, objectId });
		}
		if (!this.kinds.has("group"))
			for (const dependencies of this.groupDependencies.values())
				for (const objectId of dependencies)
					objects.set(identityKey("group", objectId), {
						kind: "group",
						objectId,
					});
		return {
			kinds,
			objects: [...objects.values()].sort((left, right) =>
				identityKey(left.kind, left.objectId).localeCompare(
					identityKey(right.kind, right.objectId),
				),
			),
		};
	}

	key() {
		return JSON.stringify(this.subscription());
	}

	private isGroupDependency(objectId: string) {
		for (const dependencies of this.groupDependencies.values())
			if (dependencies.has(objectId)) return true;
		return false;
	}
}

export function hydrationKey(target: HydrationTarget) {
	return target.objectId === undefined
		? `kind:${target.kind}`
		: `object:${target.kind}:${target.objectId}`;
}

function identityKey(kind: ShowObjectKind, objectId: string) {
	return `${kind}\0${objectId}`;
}

function sameValues(left: ReadonlySet<string> | undefined, right: ReadonlySet<string>) {
	return (
		left?.size === right.size && [...right].every((value) => left.has(value))
	);
}
