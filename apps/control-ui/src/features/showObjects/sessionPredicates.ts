import type { ShowObjectKind } from "./contracts";
import {
	hydrationKey,
	type HydrationTarget,
	type ShowObjectsViewScope,
} from "./scope";

export function isHydratedTarget(
	hydrated: ReadonlySet<string>,
	target: HydrationTarget,
) {
	return (
		hydrated.has(hydrationKey({ kind: target.kind })) ||
		hydrated.has(hydrationKey(target))
	);
}

export function isNeededTarget(
	scope: ShowObjectsViewScope,
	target: HydrationTarget,
) {
	const key = hydrationKey(target);
	return scope.targets().some((candidate) => hydrationKey(candidate) === key);
}

export function isChangeHydrating(
	scope: ShowObjectsViewScope,
	hasRun: (key: string) => boolean,
	kind: ShowObjectKind,
	objectId: string,
) {
	if (hasRun(hydrationKey({ kind }))) return true;
	if (hasRun(hydrationKey({ kind, objectId }))) return true;
	return (
		kind === "group" &&
		scope
			.affectedExactGroups(new Set([objectId]))
			.some((target) => hasRun(hydrationKey({ kind, objectId: target })))
	);
}
