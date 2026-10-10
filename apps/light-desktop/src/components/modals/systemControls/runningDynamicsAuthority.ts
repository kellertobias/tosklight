import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { DynamicsApiClient } from "../../../api/client/dynamics";
import type { DynamicRuntimeStopOwner } from "../../../api/dynamicRuntimeStopWire";
import type { RuntimeCapabilityEvent } from "../../../api/types";

export type RunningDynamicsApi = Pick<
	DynamicsApiClient,
	"runtime" | "offLive" | "stopRuntimeLive"
>;
export type RunningDynamicsSnapshot = Awaited<
	ReturnType<DynamicsApiClient["runtime"]>
>;
export type RunningDynamicsEvent = RuntimeCapabilityEvent;

/**
 * Listener-only view of the established desk event socket. The socket remains owned by
 * LightClientRuntime; this authority only adds and removes its scoped invalidation listener.
 */
export interface RunningDynamicsEventSource {
	onEvent(listener: (event: RunningDynamicsEvent) => void): () => unknown;
}

export interface RunningDynamicController {
	key: string;
	instanceId: string;
	dynamicId: string;
	poolNumber: number;
	name: string;
	targets: readonly string[];
	pending: boolean;
	instancePaused: boolean;
	speedSource: string;
	controllerId: string;
	source: string;
	stopOwner?: DynamicRuntimeStopOwner;
	stopMode?: "playback" | "programmer";
	stopGuidance?: string;
	priority: number;
	size: number;
	speedMultiplier: number;
	phaseOffsetDegrees: number;
	paused: boolean;
	winning: boolean;
	releasing: boolean;
	activationMix: number;
}

export interface RunningDynamicsAuthority {
	ready: boolean;
	loading: boolean;
	error: string | null;
	rows: readonly RunningDynamicController[];
	stoppingControllerIds: ReadonlySet<string>;
	canStop: boolean;
	off(row: RunningDynamicController): Promise<boolean>;
}

interface AuthorityState {
	ready: boolean;
	loading: boolean;
	error: string | null;
	snapshot: RunningDynamicsSnapshot | null;
}

const INACTIVE_STATE: AuthorityState = {
	ready: false,
	loading: false,
	error: null,
	snapshot: null,
};

/**
 * Snapshot-plus-push authority for the Running Dynamics section.
 *
 * Runtime events are invalidations because their compact payload does not contain a complete
 * instance/controller projection. Concurrent invalidations are coalesced into one follow-up
 * snapshot, so a burst never becomes polling and an event arriving during a read is not lost.
 */
export function useRunningDynamicsAuthority(
	enabled: boolean,
	showId: string | null,
	api: RunningDynamicsApi | null,
	events: RunningDynamicsEventSource | null,
): RunningDynamicsAuthority {
	const [state, setState] = useState<AuthorityState>(INACTIVE_STATE);
	const [stoppingControllerIds, setStoppingControllerIds] = useState<
		ReadonlySet<string>
	>(() => new Set());
	const stoppingRef = useRef(new Set<string>());
	const refreshRef = useRef<() => Promise<void>>(async () => undefined);

	useEffect(() => {
		if (!enabled || !showId || !api) {
			refreshRef.current = async () => undefined;
			setState(INACTIVE_STATE);
			setStoppingControllerIds(new Set());
			return;
		}

		let mounted = true;
		let running: Promise<void> | null = null;
		let invalidated = false;

		const refresh = (): Promise<void> => {
			if (running) {
				invalidated = true;
				return running;
			}
			running = (async () => {
				do {
					invalidated = false;
					try {
						const snapshot = await api.runtime(showId);
						if (!mounted) return;
						setState({
							ready: true,
							loading: false,
							error: null,
							snapshot,
						});
					} catch (cause) {
						if (!mounted) return;
						setState((current) => ({
							...current,
							ready: false,
							loading: false,
							error: errorMessage(cause),
						}));
						return;
					}
				} while (mounted && invalidated);
			})().finally(() => {
				running = null;
			});
			return running;
		};

		refreshRef.current = refresh;
		setState({
			ready: false,
			loading: true,
			error: null,
			snapshot: null,
		});
		const unsubscribe = events?.onEvent((event) => {
			if (event.type === "dynamic_runtime_changed") void refresh();
		});
		void refresh();

		return () => {
			mounted = false;
			if (refreshRef.current === refresh) {
				refreshRef.current = async () => undefined;
			}
			unsubscribe?.();
		};
	}, [api, enabled, events, showId]);

	const rows = useMemo(
		() => runningControllerRows(state.snapshot),
		[state.snapshot],
	);
	const off = useCallback(
		async (row: RunningDynamicController) => {
			if (!enabled || !showId || !api || row.releasing || !row.stopMode)
				return false;
			if (stoppingRef.current.has(row.controllerId)) return false;
			stoppingRef.current.add(row.controllerId);
			setStoppingControllerIds(new Set(stoppingRef.current));
			try {
				if (row.stopMode === "playback" && row.stopOwner)
					await api.stopRuntimeLive(row.stopOwner, {
						dynamicId: row.dynamicId,
						instanceId: row.instanceId,
						controllerId: row.controllerId,
					});
				else if (row.stopMode === "programmer")
					await api.offLive(row.controllerId);
				else return false;
				await refreshRef.current();
				return true;
			} catch (cause) {
				setState((current) => ({
					...current,
					error: errorMessage(cause),
				}));
				return false;
			} finally {
				stoppingRef.current.delete(row.controllerId);
				setStoppingControllerIds(new Set(stoppingRef.current));
			}
		},
		[api, enabled, showId],
	);

	return {
		ready: state.ready,
		loading: state.loading,
		error: state.error,
		rows,
		stoppingControllerIds,
		canStop: state.ready && api !== null,
		off,
	};
}

export function runningControllerRows(
	snapshot: RunningDynamicsSnapshot | null,
): RunningDynamicController[] {
	if (!snapshot) return [];
	return snapshot.instances.flatMap((instance) =>
		instance.controllers.map((controller) => ({
			key: `${instance.instance_id}:${controller.controller_id}`,
			instanceId: instance.instance_id,
			dynamicId: instance.dynamic_id,
			poolNumber: instance.pool_number,
			name: instance.name,
			targets: instance.targets,
			pending: instance.pending,
			instancePaused: instance.paused,
			speedSource: instance.speed_source,
			controllerId: controller.controller_id,
			source: controller.source,
			stopOwner: controller.stop_owner,
			stopMode: controller.stop_owner
				? "playback"
				: controller.programmer_id &&
						controller.programmer_id === snapshot.programmer_id
					? "programmer"
					: undefined,
			stopGuidance: controller.stop_owner
				? undefined
				: controller.programmer_id &&
						controller.programmer_id === snapshot.programmer_id
					? "Edits the current Programmer; follows its Preload capture mode."
					: "Stop this source from its owning control; no exact stop owner is available.",
			priority: controller.priority,
			size: controller.size,
			speedMultiplier: controller.speed_multiplier,
			phaseOffsetDegrees: controller.phase_offset_degrees,
			paused: controller.paused,
			winning: controller.winning,
			releasing: controller.releasing,
			activationMix: controller.activation_mix,
		})),
	);
}

function errorMessage(cause: unknown) {
	return cause instanceof Error
		? cause.message
		: "The Dynamic could not be stopped. Refresh its source and try again.";
}
