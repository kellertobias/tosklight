import {
	PoolCard,
	PoolGrid,
	type PoolSlotViewModel,
} from "@tosklight/ui/pools";
import { WindowHeader, WindowScrollArea } from "@tosklight/ui/window-kit";
import {
	useCallback,
	useEffect,
	useMemo,
	useState,
	useSyncExternalStore,
} from "react";
import { ApiRequestError } from "../api/ApiRequestError";
import { createLightApi } from "../api/client/api";
import type {
	DynamicRuntimeSnapshotProjection,
	DynamicSpatialMappingOverrideProjection,
	DynamicUpdateIntent,
} from "../api/types";
import { useCommandLineSurface } from "../components/control/commandLine/useCommandLineSurface";
import { monotonicEpochMillis } from "../components/control/soundToLightAnalyzer";
import { useSoundToLight } from "../components/control/useSoundToLight";
import {
	useActiveShowId,
	useAttributeRegistry,
} from "../features/deskSnapshot/DeskSnapshotState";
import { useDynamicEditorSession } from "../features/dynamics/DynamicEditorSessionContext";
import { DynamicMutationWriter } from "../features/dynamics/DynamicMutationWriter";
import { useDynamicsActions } from "../features/dynamics/DynamicsActionsContext";
import {
	useProgrammingCommandLineActions,
	useProgrammingDeleteCommandActive,
} from "../features/programmingInteraction/ProgrammingInteractionView";
import type { ShowObject } from "../features/showObjects/contracts";
import {
	useDynamics,
	usePresets,
	useShowObjectsStore,
} from "../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../features/showObjects/ShowObjectsView";
import { useSpeedGroupRuntimeView } from "../features/speedGroupRuntime/SpeedGroupRuntimeView";
import { useApp } from "../state/AppContext";
import type { WindowProps } from "./windowTypes";
import "./DynamicsWindow.css";

type DynamicObject = ShowObject<"dynamic">;

export function DynamicsWindow({
	active = true,
	compact = false,
}: WindowProps) {
	useShowObjectView("dynamic", active);
	const showId = useActiveShowId();
	const registry = useAttributeRegistry() ?? [];
	const dynamics = useDynamics(active);
	const presets = usePresets(active);
	const isolatedApi = useMemo(() => createLightApi(), []);
	const api = useDynamicsActions() ?? isolatedApi;
	const showObjectsStore = useShowObjectsStore();
	const mutationWriter = useMemo(
		() => new DynamicMutationWriter(showObjectsStore, api.showObjects),
		[api.showObjects, showObjectsStore],
	);
	const command = useCommandLineSurface({ selection: true, enabled: active });
	const commandActions = useProgrammingCommandLineActions();
	const deleteArmed = useProgrammingDeleteCommandActive(active);
	const speedGroupRuntime = useSpeedGroupRuntimeView(active);
	const soundToLight = useSoundToLight(active);
	const { open: openEditor, close: closeEditor } = useDynamicEditorSession();
	const { state: appState, dispatch } = useApp();
	const [selectedId, setSelectedId] = useState<string | null>(null);
	const [chooserSlot, setChooserSlot] = useState<number | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);
	const selected = dynamics.find((item) => item.id === selectedId) ?? null;
	useEffect(
		() => () => {
			if (selectedId) closeEditor(selectedId);
		},
		[closeEditor, selectedId],
	);
	const attributes = registry.filter(
		(attribute) =>
			attribute.recordable &&
			attribute.value_type === "continuous" &&
			attribute.normalized_min != null &&
			attribute.normalized_max != null,
	);
	const { runtime, refreshRuntime } = useDynamicsRuntime(
		active,
		showId,
		api,
		setError,
	);
	const run = useBusyOperation(setBusy, setError);
	const { toggle, create, mutate } = useDynamicsMutations({
		showId,
		api,
		mutationWriter,
		refreshRuntime,
		run,
		openEditor,
		onCreated: (id) => {
			setChooserSlot(null);
			setSelectedId(id);
		},
	});

	if (selected)
		return (
			<ConnectedDynamicEditor
				selected={selected}
				compact={compact}
				busy={busy}
				error={error}
				attributes={attributes}
				presets={presets}
				runtime={runtime}
				speedGroupBpms={Object.fromEntries(
					(speedGroupRuntime.projection?.groups ?? []).map((group) => [
						group.group,
						group.manualBpm,
					]),
				)}
				selection={command.selected}
				selectedGroupId={command.selectedGroupId}
				showId={showId}
				api={api}
				soundToLight={soundToLight}
				run={run}
				onMutate={mutate}
				onBack={() => {
					closeEditor(selected.id);
					setSelectedId(null);
				}}
				onDeleted={() => {
					closeEditor(selected.id);
					setSelectedId(null);
				}}
				onCopied={setSelectedId}
			/>
		);

	return (
		<DynamicsPool
			dynamics={dynamics}
			runtime={runtime}
			compact={compact}
			busy={busy}
			error={error}
			chooserSlot={chooserSlot}
			attributes={attributes}
			deleteArmed={deleteArmed}
			onChooseSlot={setChooserSlot}
			onError={setError}
			onToggle={toggle}
			onCreate={create}
			onOpen={(dynamic) => {
				openEditor({
					dynamicId: dynamic.id,
					task: "curves",
					encoderPage: 1,
					primaryLaneId: dynamic.body.lanes[0]?.id ?? null,
					primaryKeyframeIndex: 0,
				});
				setSelectedId(dynamic.id);
			}}
			onDelete={(dynamic) =>
				run(async () => {
					if (!showId) return;
					await api.showObjects.deleteDynamic(
						showId,
						dynamic.id,
						dynamic.revision,
					);
					await commandActions?.reset();
				})
			}
			onSet={(poolNumber) => {
				void command.replace(`SET DYNAMIC ${poolNumber}`);
				dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
			}}
			shiftArmed={appState.shiftArmed}
			updateArmed={appState.updateArmed}
			storeArmed={appState.storeArmed}
			setArmed={
				appState.playbackSetArmed ||
				/^SET\\s*$/i.test(command.read().text.trim())
			}
			onClearShift={() => dispatch({ type: "SET_SHIFT_ARMED", value: false })}
		/>
	);
}

function useDynamicsRuntime(
	active: boolean,
	showId: ReturnType<typeof useActiveShowId>,
	api: NonNullable<ReturnType<typeof useDynamicsActions>>,
	setError: (error: string | null) => void,
) {
	const [runtime, setRuntime] =
		useState<DynamicRuntimeSnapshotProjection | null>(null);
	const refreshRuntime = useCallback(async () => {
		if (!showId) {
			setRuntime(null);
			return;
		}
		setRuntime(await api.dynamics.runtime(showId));
	}, [api, showId]);
	useEffect(() => {
		if (!active || !showId) return;
		void refreshRuntime().catch((cause) =>
			setError(cause instanceof Error ? cause.message : String(cause)),
		);
		const timer = window.setInterval(() => {
			void refreshRuntime().catch(() => undefined);
		}, 750);
		return () => window.clearInterval(timer);
	}, [active, refreshRuntime, setError, showId]);
	return { runtime, refreshRuntime };
}

function useBusyOperation(
	setBusy: (busy: boolean) => void,
	setError: (error: string | null) => void,
) {
	return useCallback(
		async (operation: () => Promise<void>) => {
			setBusy(true);
			setError(null);
			try {
				await operation();
			} catch (cause) {
				setError(cause instanceof Error ? cause.message : String(cause));
			} finally {
				setBusy(false);
			}
		},
		[setBusy, setError],
	);
}

function useDynamicsMutations({
	showId,
	api,
	mutationWriter,
	refreshRuntime,
	run,
	openEditor,
	onCreated,
}: {
	showId: ReturnType<typeof useActiveShowId>;
	api: NonNullable<ReturnType<typeof useDynamicsActions>>;
	mutationWriter: DynamicMutationWriter;
	refreshRuntime(): Promise<void>;
	run(operation: () => Promise<void>): Promise<void>;
	openEditor: ReturnType<typeof useDynamicEditorSession>["open"];
	onCreated(id: string): void;
}) {
	const toggle = (dynamic: DynamicObject) =>
		run(async () => {
			if (!showId) throw new Error("No active show");
			if (dynamic.validationError) throw new Error(dynamic.validationError);
			await api.dynamics.toggle(dynamic.id);
			await refreshRuntime();
		});
	const create = (poolNumber: number, attribute: string) =>
		run(async () => {
			if (!showId) throw new Error("No active show");
			const definition = createDefaultDynamicDefinition(poolNumber, attribute);
			const outcome = await api.showObjects.createDynamic(showId, definition);
			onCreated(outcome.object.id);
			openEditor({
				dynamicId: outcome.object.id,
				task: "curves",
				encoderPage: 1,
				primaryLaneId: definition.lanes[0]?.id ?? null,
				primaryKeyframeIndex: 0,
			});
		});
	const mutate = (
		dynamic: DynamicObject,
		intent: DynamicUpdateIntent,
		mutationGroup?: string,
	) =>
		run(async () => {
			if (!showId) throw new Error("No active show");
			await mutationWriter.update(showId, dynamic.id, intent, mutationGroup);
		});
	return { toggle, create, mutate };
}

function ConnectedDynamicEditor({
	selected,
	showId,
	api,
	soundToLight,
	run,
	onBack,
	onDeleted,
	onCopied,
	onMutate,
	...view
}: Omit<
	DynamicEditorProps,
	"dynamic" | "onBack" | "onDelete" | "onMove" | "onCopy" | "onSpeedGroupTap"
> & {
	selected: DynamicObject;
	showId: ReturnType<typeof useActiveShowId>;
	api: NonNullable<ReturnType<typeof useDynamicsActions>>;
	soundToLight: ReturnType<typeof useSoundToLight>;
	run(operation: () => Promise<void>): Promise<void>;
	onBack(): void;
	onDeleted(): void;
	onCopied(id: string): void;
}) {
	const showObjectsStore = useShowObjectsStore();
	const showRevision = useSyncExternalStore(
		showObjectsStore.subscribe,
		() => showObjectsStore.getSnapshot().showRevision,
	);
	const loadSpatialPreview = useCallback(
		async (spatialMapping: DynamicSpatialMappingOverrideProjection) => {
			if (!showId || showRevision == null)
				throw new Error("No authoritative show revision");
			return api.showObjects.previewDynamicSpatialMapping(showId, selected.id, {
				expected_dynamic_revision: selected.revision,
				expected_show_revision: showRevision,
				spatial_mapping: spatialMapping,
			});
		},
		[api.showObjects, selected.id, selected.revision, showId, showRevision],
	);
	const applySpatialMapping = useCallback(
		async (spatialMapping: DynamicSpatialMappingOverrideProjection) => {
			if (!showId) throw new Error("No active show");
			try {
				const outcome = await api.showObjects.updateDynamic(
					showId,
					selected.id,
					selected.revision,
					{ type: "set_spatial_mapping", spatial_mapping: spatialMapping },
				);
				showObjectsStore.installObjects(
					showId,
					[
						{
							kind: "dynamic",
							objectId: selected.id,
							object: outcome.object as DynamicObject,
						},
					],
					outcome.event_sequence,
					outcome.show_revision,
				);
				return "applied" as const;
			} catch (error) {
				if (!(error instanceof ApiRequestError) || error.status !== 409)
					throw error;
				const snapshot = await api.showObjects.collectionSnapshot<
					DynamicObject["body"]
				>(showId, "dynamic");
				const authoritative = snapshot.objects.find(
					(candidate) => candidate.id === selected.id,
				);
				if (authoritative)
					showObjectsStore.installObjects(
						showId,
						[
							{
								kind: "dynamic",
								objectId: selected.id,
								object: authoritative as DynamicObject,
							},
						],
						null,
						snapshot.showRevision,
					);
				return "conflict" as const;
			}
		},
		[api.showObjects, selected.id, selected.revision, showId, showObjectsStore],
	);
	return (
		<DynamicEditor
			{...view}
			dynamic={selected}
			onBack={onBack}
			onMutate={onMutate}
			onLoadSpatialPreview={loadSpatialPreview}
			onApplySpatialMapping={applySpatialMapping}
			onSpeedGroupTap={(group) =>
				run(async () => {
					await soundToLight.action(group, {
						action: "learn",
						captured_at_millis: monotonicEpochMillis(),
					});
				})
			}
			onDelete={() =>
				run(async () => {
					if (!showId) throw new Error("No active show");
					await api.showObjects.deleteDynamic(
						showId,
						selected.id,
						selected.revision,
					);
					onDeleted();
				})
			}
			onMove={(poolNumber) =>
				run(async () => {
					if (!showId) throw new Error("No active show");
					await api.showObjects.moveDynamic(
						showId,
						selected.id,
						selected.revision,
						poolNumber,
					);
				})
			}
			onCopy={(poolNumber) =>
				run(async () => {
					if (!showId) throw new Error("No active show");
					const outcome = await api.showObjects.copyDynamic(
						showId,
						selected.id,
						selected.revision,
						poolNumber,
					);
					onCopied(outcome.object.id);
				})
			}
		/>
	);
}

interface DynamicsPoolProps {
	dynamics: readonly DynamicObject[];
	runtime: DynamicRuntimeSnapshotProjection | null;
	compact: boolean;
	busy: boolean;
	error: string | null;
	chooserSlot: number | null;
	attributes: readonly { id: string; label: string; family: string }[];
	deleteArmed: boolean;
	shiftArmed: boolean;
	updateArmed: boolean;
	storeArmed: boolean;
	setArmed: boolean;
	onChooseSlot(slot: number | null): void;
	onError(error: string): void;
	onToggle(dynamic: DynamicObject): void;
	onCreate(poolNumber: number, attribute: string): void;
	onOpen(dynamic: DynamicObject): void;
	onDelete(dynamic: DynamicObject): void;
	onSet(poolNumber: number): void;
	onClearShift(): void;
}

function DynamicsPool(props: DynamicsPoolProps) {
	const { dynamics, runtime } = props;
	const occupied = new Map(
		dynamics.map((dynamic) => [dynamic.body.pool_number, dynamic]),
	);
	const slotCount = Math.max(
		200,
		...dynamics.map((dynamic) => dynamic.body.pool_number),
	);
	const slots: PoolSlotViewModel<number>[] = dynamics.map((dynamic) => ({
		id: dynamic.body.pool_number,
		position: dynamic.body.pool_number - 1,
		card: { number: dynamic.body.pool_number, primary: dynamic.body.name },
	}));
	return (
		<section className="dynamics-window" aria-busy={props.busy}>
			{!props.compact && (
				<WindowHeader
					title="Dynamics"
					info={{ primary: `${dynamics.length} Dynamics` }}
					actions={[]}
				/>
			)}
			{props.error && (
				<p className="dynamics-error" role="alert">
					{props.error}
				</p>
			)}
			<WindowScrollArea>
				<PoolGrid
					slots={slots}
					slotCount={slotCount}
					emptySlot={(index) => ({
						id: index + 1,
						position: index,
						card: { number: index + 1, primary: "Empty", states: ["empty"] },
					})}
					renderSlot={(_, index) => (
						<DynamicPoolTile
							key={index}
							{...props}
							poolNumber={index + 1}
							dynamic={occupied.get(index + 1)}
							runtime={runtime}
						/>
					)}
				/>
			</WindowScrollArea>
			{props.chooserSlot !== null && (
				<LaneAttributeModal
					id={`select-lane-attribute-create-${props.chooserSlot}`}
					title="Select lane attribute"
					details={`Create Dynamic ${props.chooserSlot}`}
					attributes={props.attributes}
					busy={props.busy}
					onClose={() => props.onChooseSlot(null)}
					onChoose={(attribute) =>
						props.onCreate(props.chooserSlot as number, attribute)
					}
				/>
			)}
		</section>
	);
}

function DynamicPoolTile({
	poolNumber,
	dynamic,
	runtime,
	...actions
}: DynamicsPoolProps & {
	poolNumber: number;
	dynamic: DynamicObject | undefined;
}) {
	const count = dynamic ? runningCount(runtime, dynamic.id) : 0;
	const running = count > 0;
	const status = dynamic ? definitionStatus(runtime, dynamic.id) : null;
	const validationError = dynamic?.validationError ?? null;
	const open = () => {
		if (dynamic) actions.onOpen(dynamic);
	};
	return (
		<PoolCard
			className={`dynamic-pool-card ${running ? "running" : ""} ${validationError ? "invalid" : ""}`}
			aria-pressed={running}
			model={{
				number: poolNumber,
				primary: dynamic?.body.name ?? "Empty",
				secondary: dynamic
					? targetSummary(dynamic.body)
					: "Tap to choose first lane",
				details: dynamic
					? [
							`${dynamic.body.lanes.length} ${dynamic.body.lanes.length === 1 ? "lane" : "lanes"}`,
							...(validationError ? [validationError] : []),
							...(status ? [coverageSummary(status)] : []),
							validationError
								? "Blocked until repaired or deleted"
								: running
									? `${count} running ${count === 1 ? "instance" : "instances"}`
									: "Ready",
						]
					: [],
				status: validationError || status?.warning ? "⚠" : undefined,
				icon: dynamic?.body.icon,
				iconColor: dynamic?.body.color ?? "#4edcff",
				color: dynamic?.body.color ?? "#4edcff",
				kind: "generic",
				states: [...(!dynamic ? (["empty"] as const) : [])],
			}}
			onClick={(event) => {
				if (actions.deleteArmed) {
					if (dynamic) actions.onDelete(dynamic);
				} else if (!dynamic) actions.onChooseSlot(poolNumber);
				else if (validationError) {
					actions.onError(validationError);
					actions.onClearShift();
				} else if (event.shiftKey || actions.shiftArmed) {
					open();
					actions.onClearShift();
				} else if (actions.updateArmed || actions.storeArmed)
					actions.onError(
						actions.updateArmed
							? "Finish or cancel Update before operating a Dynamic tile."
							: "Finish or cancel Record/Store before operating a Dynamic tile.",
					);
				else if (actions.setArmed) actions.onSet(poolNumber);
				else actions.onToggle(dynamic);
			}}
			onContextMenu={(event) => {
				event.preventDefault();
				open();
			}}
			onPressHold={open}
		/>
	);
}

export {
	createDefaultDynamicDefinition,
	createDefaultDynamicLane,
	DynamicEditor,
	type DynamicEditorProps,
	type DynamicEditorView,
	DynamicEncoderDeck,
} from "./dynamics/DynamicsEditor";

import {
	coverageSummary,
	createDefaultDynamicDefinition,
	DynamicEditor,
	type DynamicEditorProps,
	definitionStatus,
	LaneAttributeModal,
	runningCount,
	targetSummary,
} from "./dynamics/DynamicsEditor";
