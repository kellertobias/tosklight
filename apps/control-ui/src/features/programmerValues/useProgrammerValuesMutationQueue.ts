import { useCallback, useMemo } from "react";
import { useProgrammerPreloadValuesActions } from "../programmerPreloadValues/ProgrammerPreloadValuesView";
import { useStrictModeSafeStop } from "../shared/useStrictModeSafeStop";
import type {
	BatchProgrammerValuesInput,
	ProgrammerValuesMutation,
} from "./contracts";
import { LatestProgrammerValuesWriteQueue } from "./LatestProgrammerValuesWriteQueue";
import { useProgrammerValuesActions } from "./ProgrammerValuesView";
import { useProgrammerValuesActivity } from "./useProgrammerValuesActivity";

interface ProgrammerValuesBatchPort {
	batch(input: BatchProgrammerValuesInput): Promise<unknown>;
}

export interface NormalizedProgrammerAssignment {
	fixtureId: string;
	attribute: string;
	value: number;
}

export interface ProgrammerValuesMutationQueueController {
	canWrite: boolean;
	route: "normal" | "preload" | null;
	submitLatest(
		key: string,
		mutations: readonly ProgrammerValuesMutation[],
	): Promise<unknown | null>;
	submitBarrier(
		mutations: readonly ProgrammerValuesMutation[],
	): Promise<unknown | null>;
}

export function useProgrammerValuesMutationQueue(
	enabled = true,
): ProgrammerValuesMutationQueueController {
	const activity = useProgrammerValuesActivity(enabled);
	const normalActions = useProgrammerValuesActions();
	const preloadActions = useProgrammerPreloadValuesActions();
	const actions = selectActions(
		activity.ready ? activity.authority : "loading",
		normalActions,
		preloadActions,
	);
	const queue = useMemo(
		() => new LatestProgrammerValuesWriteQueue(),
		[actions],
	);
	useStrictModeSafeStop(queue);
	const submit = useCallback(
		(mutations: readonly ProgrammerValuesMutation[]) => {
			if (!actions || mutations.length === 0) return Promise.resolve(null);
			return actions.batch({ requestId: crypto.randomUUID(), mutations });
		},
		[actions],
	);
	return {
		canWrite: enabled && activity.ready && actions !== null,
		route:
			activity.ready && activity.authority !== "loading"
				? activity.authority
				: null,
		submitLatest: useCallback(
			(key: string, mutations: readonly ProgrammerValuesMutation[]) =>
				queue.submitLatest(key, JSON.stringify(mutations), () =>
					submit(mutations),
				),
			[queue, submit],
		),
		submitBarrier: useCallback(
			(mutations: readonly ProgrammerValuesMutation[]) =>
				queue.submitBarrier(() => submit(mutations)),
			[queue, submit],
		),
	};
}

export function normalizedFixtureMutations(
	assignments: readonly NormalizedProgrammerAssignment[],
	programmerFadeMillis: number | undefined,
): ProgrammerValuesMutation[] {
	const timing = {
		fade: true,
		fadeMillis: programmerFadeMillis ?? 3_000,
		delayMillis: null,
	};
	return assignments.map(({ fixtureId, attribute, value }) => ({
		action: "set_fixture",
		fixtureId,
		attribute,
		value: { kind: "normalized", value },
		timing,
	}));
}

export function programmerValuesMutationKey(
	mutations: readonly ProgrammerValuesMutation[],
) {
	return mutations
		.map((mutation) => {
			if (
				mutation.action === "set_fixture" ||
				mutation.action === "release_fixture"
			)
				return `fixture:${mutation.fixtureId}:${mutation.attribute}`;
			return `group:${mutation.groupId}:${mutation.attribute}`;
		})
		.join("\u0000");
}

function selectActions(
	authority: "loading" | "normal" | "preload",
	normal: ProgrammerValuesBatchPort | null,
	preload: ProgrammerValuesBatchPort | null,
) {
	if (authority === "normal") return normal;
	if (authority === "preload") return preload;
	return null;
}
