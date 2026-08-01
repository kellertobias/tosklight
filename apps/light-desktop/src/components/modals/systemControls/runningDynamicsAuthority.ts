import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { DynamicsApiClient } from "../../../api/client/dynamics";
import type { RuntimeCapabilityEvent } from "../../../api/types";

export type RunningDynamicsApi = Pick<DynamicsApiClient, "runtime" | "offLive">;
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
			if (!enabled || !showId || !api || row.releasing) return false;
			let alreadyStopping = false;
			setStoppingControllerIds((current) => {
				if (current.has(row.controllerId)) {
					alreadyStopping = true;
					return current;
				}
				const next = new Set(current);
				next.add(row.controllerId);
				return next;
			});
			if (alreadyStopping) return false;
			try {
				await api.offLive(row.controllerId);
				await refreshRef.current();
				return true;
			} catch (cause) {
				setState((current) => ({
					...current,
					error: errorMessage(cause),
				}));
				return false;
			} finally {
				setStoppingControllerIds((current) => {
					const next = new Set(current);
					next.delete(row.controllerId);
					return next;
				});
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
	return cause instanceof Error ? cause.message : String(cause);
}
