import { useCallback, useMemo } from "react";
import { useProgrammerPreloadValuesActions } from "../programmerPreloadValues/ProgrammerPreloadValuesView";
import { useStrictModeSafeStop } from "../shared/useStrictModeSafeStop";
import type {
	BatchProgrammerValuesInput,
	ProgrammerValuesMutation,
} from "./contracts";
import { LatestProgrammerValuesWriteQueue } from "./LatestProgrammerValuesWriteQueue";
import { useProgrammerValuesActions } from "./ProgrammerValuesView";
import {
	type ProgrammerValuesActivity,
	useProgrammerValuesActivity,
} from "./useProgrammerValuesActivity";

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
	unavailableReason: string | null;
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
		unavailableReason: programmerValuesWriteUnavailableReason(
			enabled,
			activity,
			actions !== null,
		),
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

export function programmerValuesWriteUnavailableReason(
	enabled: boolean,
	activity: Pick<ProgrammerValuesActivity, "authority" | "ready">,
	actionsAvailable: boolean,
): string | null {
	if (!enabled) return "Channel controls are inactive";
	if (activity.authority === "loading") return "Programmer mode is loading";
	if (!activity.ready)
		return activity.authority === "preload"
			? "Preload values are loading"
			: "Programmer values are loading";
	if (!actionsAvailable)
		return activity.authority === "preload"
			? "Preload control is unavailable"
			: "Programmer control is unavailable";
	return null;
}

export function normalizedFixtureMutations(
	assignments: readonly NormalizedProgrammerAssignment[],
	programmerFadeMillis: number | undefined,
	immediate = false,
): ProgrammerValuesMutation[] {
	const timing = immediate
		? { fade: false, fadeMillis: null, delayMillis: null }
		: {
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
			if (mutation.action === "set_selection")
				return `selection:${mutation.fixtureIds.join(",")}:${mutation.attribute}`;
			if (mutation.action === "set_selection_color_range")
				return `selection-color:${mutation.fixtureIds.join(",")}`;
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
