import { useCallback, useMemo, useState, useSyncExternalStore } from "react";
import type { LightApi } from "../../api/client/api";
import type {
	ScheduleOccurrenceProjection as WireOccurrence,
	ScheduleProjection as WireProjection,
	ScheduleTarget as WireTarget,
	ScheduleTrigger as WireTrigger,
} from "../../api/client/schedules";
import type { ShowObjectsStore } from "../showObjects/store";
import type {
	PlaybackScheduleTarget,
	ScheduleDefinition,
	ScheduleDraft,
	ScheduleOccurrence,
	ScheduleProjection,
	ScheduleResult,
	SchedulerController,
	SchedulerSnapshot,
} from "./contracts";
import type { SchedulerRuntimeStore } from "./runtimeStore";

interface SchedulerAuthority {
	api: LightApi;
	showId: string | null;
	showObjectsStore: ShowObjectsStore;
	runtimeStore: SchedulerRuntimeStore;
	canWrite: boolean;
	reportError(message: string): void;
}

const LOADING: SchedulerSnapshot = {
	status: "loading",
	timezone: "",
	serverDate: "",
	schedules: [],
	playbackTargets: [],
	canWrite: false,
	error: null,
};

export function useServerSchedulerController({
	api,
	showId,
	showObjectsStore,
	runtimeStore,
	canWrite,
	reportError,
}: SchedulerAuthority): SchedulerController {
	const objects = useSyncExternalStore(
		showObjectsStore.subscribe,
		showObjectsStore.getSnapshot,
		showObjectsStore.getSnapshot,
	);
	const targets = useMemo(() => playbackTargets(objects), [objects]);
	const [snapshot, setSnapshot] = useState<SchedulerSnapshot>(LOADING);

	const refresh = useCallback(async () => {
		if (!showId) {
			setSnapshot({
				...LOADING,
				status: "ready",
				canWrite: false,
				playbackTargets: targets,
			});
			return;
		}
		try {
			const wire = await api.schedules.snapshot(showId);
			setSnapshot({
				status: "ready",
				timezone: wire.timezone,
				serverDate: serverDate(wire.server_now, wire.timezone),
				schedules: wire.schedules.map((schedule) =>
					projection(schedule, targets),
				),
				playbackTargets: targets,
				canWrite,
				error: null,
			});
		} catch (reason) {
			const message = errorMessage(reason);
			setSnapshot((current) => ({
				...current,
				status: "error",
				playbackTargets: targets,
				canWrite: false,
				error: message,
			}));
		}
	}, [api.schedules, canWrite, showId, targets]);

	const mutate = useCallback(
		async (operation: () => Promise<unknown>) => {
			if (!showId || !canWrite) return false;
			try {
				await operation();
				await refresh();
				return true;
			} catch (reason) {
				reportError(errorMessage(reason));
				return false;
			}
		},
		[canWrite, refresh, reportError, showId],
	);

	return useMemo(
		() => ({
			snapshot,
			activate: () => {
				void refresh();
				const unsubscribe = runtimeStore.subscribe(() => void refresh());
				// A low-rate snapshot repairs any event gap without polling the live desk path.
				const timer = globalThis.setInterval(() => void refresh(), 30_000);
				return () => {
					unsubscribe();
					globalThis.clearInterval(timer);
				};
			},
			retry: refresh,
			preview: async (draft: ScheduleDraft, signal: AbortSignal) => {
				if (!showId)
					return {
						status: "invalid" as const,
						occurrences: [],
						message: "No show is active.",
					};
				try {
					const result = await api.schedules.preview(
						showId,
						{ trigger: wireTrigger(draft), count: 5 },
						signal,
					);
					return {
						status: "ready" as const,
						occurrences: result.occurrences.map(occurrence),
						message: null,
					};
				} catch (reason) {
					return {
						status: "invalid" as const,
						occurrences: [],
						message: errorMessage(reason),
					};
				}
			},
			create: (draft: ScheduleDraft) =>
				mutate(() =>
					api.schedules.create(showId ?? "", {
						name: draft.name,
						enabled: draft.enabled,
						trigger: wireTrigger(draft),
						target: wireTarget(draft),
					}),
				),
			update: (id: string, expectedRevision: number, draft: ScheduleDraft) =>
				mutate(() =>
					api.schedules.update(showId ?? "", id, {
						expected_revision: expectedRevision,
						patch: {
							name: draft.name,
							enabled: draft.enabled,
							trigger: wireTrigger(draft),
							target: wireTarget(draft),
						},
					}),
				),
			setEnabled: (id: string, expectedRevision: number, enabled: boolean) =>
				mutate(() =>
					api.schedules.update(showId ?? "", id, {
						expected_revision: expectedRevision,
						patch: { enabled },
					}),
				),
			duplicate: (id: string, expectedRevision: number) =>
				mutate(() =>
					api.schedules.duplicate(showId ?? "", id, expectedRevision),
				),
			delete: (id: string, expectedRevision: number) =>
				mutate(() => api.schedules.delete(showId ?? "", id, expectedRevision)),
		}),
		[api.schedules, mutate, refresh, runtimeStore, showId, snapshot],
	);
}

function projection(
	wire: WireProjection,
	targets: readonly PlaybackScheduleTarget[],
): ScheduleProjection {
	const definition = scheduleDefinition(wire, targets);
	const next = wire.next_occurrence ? occurrence(wire.next_occurrence) : null;
	return {
		definition,
		nextOccurrence: next,
		upcomingOccurrences: next ? [next] : [],
		lastResult: wire.last_result ? result(wire.last_result) : null,
		validationMessage: wire.validation_error ?? null,
	};
}

function scheduleDefinition(
	wire: WireProjection,
	targets: readonly PlaybackScheduleTarget[],
): ScheduleDefinition {
	const definition = wire.definition;
	return {
		id: definition.id,
		revision: wire.object_revision,
		name: definition.name,
		enabled: definition.enabled,
		timing: timing(definition.trigger),
		target: target(definition.target, targets),
	};
}

function timing(trigger: WireTrigger): ScheduleDefinition["timing"] {
	switch (trigger.type) {
		case "interval":
			return {
				type: "interval",
				everySeconds: trigger.every_seconds,
				anchor: "activation",
			};
		case "one_time": {
			const [localDate = "", localTime = ""] = trigger.at.split("T");
			return {
				type: "one_time",
				localDate,
				localTime,
				remainEnabledAfterSuccess: false,
			};
		}
		case "calendar":
			return {
				type: "calendar_expression",
				expression:
					trigger.rule.type === "expression" ? trigger.rule.expression : "",
				summary:
					trigger.rule.type === "expression"
						? trigger.rule.expression
						: "Guided calendar rule",
			};
	}
}

function target(
	wire: WireTarget,
	targets: readonly PlaybackScheduleTarget[],
): ScheduleDefinition["target"] {
	if (wire.type === "macro")
		return { type: "macro", macroId: wire.macro_id, label: wire.macro_id };
	const id = `${wire.page}:${wire.slot}:${wire.playback_number}`;
	const known = targets.find((candidate) => candidate.id === id);
	return {
		type: "playback",
		playbackId: id,
		label: known?.label ?? `Playback ${wire.playback_number}`,
		page: wire.page,
		slot: wire.slot,
		playback: wire.playback_number,
		action: wire.action,
		masterPercent: wire.master_transition
			? wire.master_transition.level * 100
			: null,
		fadeMillis: wire.master_transition?.fade_millis ?? null,
	};
}

function wireTrigger(draft: ScheduleDraft): WireTrigger {
	switch (draft.timing.type) {
		case "interval":
			return {
				type: "interval",
				every_seconds: draft.timing.everySeconds,
				enabled_at: new Date().toISOString(),
			};
		case "calendar_expression":
			return {
				type: "calendar",
				rule: { type: "expression", expression: draft.timing.expression },
			};
		case "one_time":
			return {
				type: "one_time",
				at: `${draft.timing.localDate}T${draft.timing.localTime}`,
			};
	}
}

function wireTarget(draft: ScheduleDraft): WireTarget {
	if (draft.target.type === "macro")
		return { type: "macro", macro_id: draft.target.macroId };
	return {
		type: "playback",
		page: draft.target.page,
		slot: draft.target.slot,
		playback_number: draft.target.playback,
		action: draft.target.action,
		master_transition:
			draft.target.masterPercent == null
				? null
				: {
						level: draft.target.masterPercent / 100,
						fade_millis: draft.target.fadeMillis ?? 0,
					},
	};
}

function playbackTargets(
	objects: ReturnType<ShowObjectsStore["getSnapshot"]>,
): PlaybackScheduleTarget[] {
	const playbacks = new Map(
		objects.playbacks.map((playback) => [playback.body.number, playback.body]),
	);
	const result: PlaybackScheduleTarget[] = [];
	for (const page of objects.playbackPages) {
		for (const [slotValue, playbackNumber] of Object.entries(page.body.slots)) {
			const slot = Number(slotValue);
			const playback = playbacks.get(playbackNumber);
			if (!playback) continue;
			const type = playback.target.type;
			if (!["cue_list", "dynamic", "group"].includes(type)) continue;
			result.push({
				id: `${page.body.number}:${slot}:${playbackNumber}`,
				label: playback.name,
				page: page.body.number,
				slot,
				playback: playbackNumber,
				supportedActions:
					type === "group"
						? ["on", "off", "release", "toggle"]
						: ["go", "pause", "on", "off", "release", "toggle"],
				supportsMaster: true,
			});
		}
	}
	return result.sort(
		(left, right) => left.page - right.page || left.slot - right.slot,
	);
}

function occurrence(wire: WireOccurrence): ScheduleOccurrence {
	const [localDate = "", localTime = ""] = wire.local_time.split("T");
	return {
		id: wire.occurrence_id,
		instant: wire.scheduled_for,
		localDate,
		localTime,
	};
}

function result(
	wire: NonNullable<WireProjection["last_result"]>,
): ScheduleResult {
	return {
		status:
			wire.status === "completed" || wire.status === "skipped"
				? wire.status
				: "failed",
		occurredAt: wire.recorded_at,
		message:
			wire.message ??
			(wire.status === "interrupted"
				? "Interrupted when the server stopped."
				: ""),
	};
}

function serverDate(instant: string, timezone: string) {
	return new Intl.DateTimeFormat("en-CA", {
		timeZone: timezone,
		year: "numeric",
		month: "2-digit",
		day: "2-digit",
	})
		.format(new Date(instant))
		.replaceAll("/", "-");
}

function errorMessage(reason: unknown) {
	return reason instanceof Error ? reason.message : String(reason);
}
