import {
	Button,
	ColorPickerField,
	CyclingValueToggle,
	FadedDivider,
	FormLayout,
	GroupedSelectionField,
	IconPickerField,
	MultiValueToggle,
	MultiValueToggleField,
	NumberField,
	SelectField,
	SwitchField,
	TextField,
} from "@tosklight/ui";
import {
	EncoderSection,
	type EncoderSectionItem,
	type HardwareEncoderDisplayHandle,
} from "@tosklight/ui/encoders";
import { ModalFrame } from "@tosklight/ui/modals";
import {
	PoolCard,
	PoolGrid,
	type PoolSlotViewModel,
} from "@tosklight/ui/pools";
import {
	WindowHeader,
	WindowScrollArea,
	WindowSettings,
} from "@tosklight/ui/window-kit";
import {
	type CSSProperties,
	type ReactNode,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { createLightApi } from "../api/client/api";
import type {
	DynamicDefinitionProjection,
	DynamicDefinitionStatusProjection,
	DynamicLaneModeProjection,
	DynamicLaneProjection,
	DynamicPeriodicFunctionProjection,
	DynamicPhaseOrderingProjection,
	DynamicRandomGroupProjection,
	DynamicRuntimeSnapshotProjection,
	DynamicScalarSourceProjection,
	DynamicUpdateIntent,
	SpeedGroupId,
} from "../api/generated/light-wire";
import { useCommandLineSurface } from "../components/control/commandLine/useCommandLineSurface";
import { monotonicEpochMillis } from "../components/control/soundToLightAnalyzer";
import { useSoundToLight } from "../components/control/useSoundToLight";
import {
	useActiveShowId,
	useAttributeRegistry,
	useHardwareConnected,
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
import { useStageLayout } from "./stageWindow/useStageLayout";
import type { WindowProps } from "./windowTypes";
import "./DynamicsWindow.css";

type DynamicObject = ShowObject<"dynamic">;
type PresetObject = ShowObject<"preset">;
export type DynamicEditorView = "curves" | "phase" | "speed";

const sourceCurrent: DynamicScalarSourceProjection = { type: "current" };
const sourceZero: DynamicScalarSourceProjection = { type: "value", value: 0 };
const sourceFull: DynamicScalarSourceProjection = { type: "value", value: 1 };
const curveComposerMethods = [
	{ value: "keyframes", label: "Keyframes" },
	{ value: "max_min", label: "Max / min" },
	{ value: "middle_amplitude", label: "Middle / amplitude" },
] as const;

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
	const [runtime, setRuntime] =
		useState<DynamicRuntimeSnapshotProjection | null>(null);
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
	}, [active, refreshRuntime, showId]);
	const run = async (operation: () => Promise<void>) => {
		setBusy(true);
		setError(null);
		try {
			await operation();
		} catch (cause) {
			setError(cause instanceof Error ? cause.message : String(cause));
		} finally {
			setBusy(false);
		}
	};
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
			setChooserSlot(null);
			setSelectedId(outcome.object.id);
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

	if (selected)
		return (
			<DynamicEditor
				dynamic={selected}
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
				onBack={() => {
					closeEditor(selected.id);
					setSelectedId(null);
				}}
				onMutate={mutate}
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
						closeEditor(selected.id);
						setSelectedId(null);
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
						setSelectedId(outcome.object.id);
					})
				}
			/>
		);

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
		card: {
			number: dynamic.body.pool_number,
			primary: dynamic.body.name,
		},
	}));
	return (
		<section className="dynamics-window" aria-busy={busy}>
			{!compact && (
				<WindowHeader
					title="Dynamics"
					info={{ primary: `${dynamics.length} Dynamics` }}
					actions={[]}
				/>
			)}
			{error && (
				<p className="dynamics-error" role="alert">
					{error}
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
					renderSlot={(_, index) => {
						const poolNumber = index + 1;
						const dynamic = occupied.get(poolNumber);
						const running = dynamic
							? runningCount(runtime, dynamic.id) > 0
							: false;
						const count = dynamic ? runningCount(runtime, dynamic.id) : 0;
						const status = dynamic
							? definitionStatus(runtime, dynamic.id)
							: null;
						const validationError = dynamic?.validationError ?? null;
						return (
							<PoolCard
								key={poolNumber}
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
									icon: dynamic?.body.icon ?? "∿",
									iconColor: dynamic?.body.color ?? "#4edcff",
									color: dynamic?.body.color ?? "#4edcff",
									kind: "generic",
									states: [...(!dynamic ? (["empty"] as const) : [])],
								}}
								onClick={(event) => {
									if (deleteArmed) {
										if (!dynamic || !showId) return;
										void run(async () => {
											await api.showObjects.deleteDynamic(
												showId,
												dynamic.id,
												dynamic.revision,
											);
											await commandActions?.reset();
										});
										return;
									}
									if (!dynamic) {
										setChooserSlot(poolNumber);
										return;
									}
									if (validationError) {
										setError(validationError);
										dispatch({ type: "SET_SHIFT_ARMED", value: false });
										return;
									}
									if (event.shiftKey || appState.shiftArmed) {
										openEditor({
											dynamicId: dynamic.id,
											task: "curves",
											encoderPage: 1,
											primaryLaneId: dynamic.body.lanes[0]?.id ?? null,
											primaryKeyframeIndex: 0,
										});
										setSelectedId(dynamic.id);
										dispatch({ type: "SET_SHIFT_ARMED", value: false });
										return;
									}
									if (appState.updateArmed || appState.storeArmed) {
										setError(
											appState.updateArmed
												? "Finish or cancel Update before operating a Dynamic tile."
												: "Finish or cancel Record/Store before operating a Dynamic tile.",
										);
										return;
									}
									if (
										appState.playbackSetArmed ||
										/^SET\s*$/i.test(command.read().text.trim())
									) {
										void command.replace(`SET DYNAMIC ${poolNumber}`);
										dispatch({
											type: "SET_PLAYBACK_SET_ARMED",
											value: false,
										});
										return;
									}
									void toggle(dynamic);
								}}
								onContextMenu={(event) => {
									event.preventDefault();
									if (!dynamic) return;
									openEditor({
										dynamicId: dynamic.id,
										task: "curves",
										encoderPage: 1,
										primaryLaneId: dynamic.body.lanes[0]?.id ?? null,
										primaryKeyframeIndex: 0,
									});
									setSelectedId(dynamic.id);
								}}
								onPressHold={() => {
									if (!dynamic) return;
									openEditor({
										dynamicId: dynamic.id,
										task: "curves",
										encoderPage: 1,
										primaryLaneId: dynamic.body.lanes[0]?.id ?? null,
										primaryKeyframeIndex: 0,
									});
									setSelectedId(dynamic.id);
								}}
							/>
						);
					}}
				/>
			</WindowScrollArea>
			{chooserSlot !== null && (
				<LaneAttributeModal
					id={`select-lane-attribute-create-${chooserSlot}`}
					title="Select lane attribute"
					details={`Create Dynamic ${chooserSlot}`}
					attributes={attributes}
					busy={busy}
					onClose={() => setChooserSlot(null)}
					onChoose={(attribute) => void create(chooserSlot, attribute)}
				/>
			)}
		</section>
	);
}

export interface DynamicEditorProps {
	dynamic: DynamicObject;
	compact: boolean;
	busy: boolean;
	error: string | null;
	attributes: readonly { id: string; label: string; family: string }[];
	presets: readonly PresetObject[];
	runtime: DynamicRuntimeSnapshotProjection | null;
	speedGroupBpms?: Partial<Record<SpeedGroupId, number>>;
	selection: readonly string[];
	selectedGroupId: string | null;
	view?: DynamicEditorView;
	onViewChange?(view: DynamicEditorView): void;
	onBack(): void;
	onMutate(
		dynamic: DynamicObject,
		intent: DynamicUpdateIntent,
		mutationGroup?: string,
	): Promise<void>;
	onSpeedGroupTap?(group: SpeedGroupId): void;
	onDelete(): void;
	onMove(poolNumber: number): void;
	onCopy(poolNumber: number): void;
}

/**
 * The production Dynamic editor composition boundary. The connected window owns
 * persistence and runtime refreshes; deterministic renderers can provide those
 * values and callbacks without creating a second version of the editor UI.
 */
export function DynamicEditor({
	dynamic,
	compact,
	busy,
	error,
	attributes,
	runtime,
	speedGroupBpms,
	selection,
	selectedGroupId,
	view: controlledView,
	onViewChange,
	onBack,
	onMutate,
	onSpeedGroupTap,
}: DynamicEditorProps) {
	const { state: appState, dispatch } = useApp();
	const stageLayout = useStageLayout();
	const {
		session,
		open: openEditor,
		update: updateEditor,
	} = useDynamicEditorSession();
	const view: DynamicEditorView =
		controlledView ??
		(session?.dynamicId === dynamic.id ? session.task : "curves");
	const [primaryLane, setPrimaryLane] = useState(
		dynamic.body.lanes[0]?.id ?? null,
	);
	const [selectedLanes, setSelectedLanes] = useState<Set<string>>(
		new Set(primaryLane ? [primaryLane] : []),
	);
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	const [addingLane, setAddingLane] = useState(false);
	const [previewing, setPreviewing] = useState(false);
	const [previewPhase, setPreviewPhase] = useState(0);
	const encoderPage =
		session?.dynamicId === dynamic.id ? session.encoderPage : 1;
	const primaryKeyframeIndex =
		session?.dynamicId === dynamic.id ? session.primaryKeyframeIndex : 0;
	const setPrimaryKeyframeIndex = (index: number) =>
		updateEditor({ primaryKeyframeIndex: index });
	const lane =
		dynamic.body.lanes.find((candidate) => candidate.id === primaryLane) ??
		dynamic.body.lanes[0];
	const replaceLane = (next: DynamicLaneProjection, group?: string) =>
		onMutate(
			dynamic,
			{ type: "replace_lane", lane_id: next.id, lane: next },
			group,
		);
	const selectLane = (id: string, additive: boolean) => {
		setPrimaryLane(id);
		updateEditor({ primaryLaneId: id, primaryKeyframeIndex: 0 });
		setSelectedLanes((current) => {
			if (!additive) return new Set([id]);
			const next = new Set(current);
			if (next.has(id) && next.size > 1) next.delete(id);
			else next.add(id);
			return next;
		});
		if (appState.shiftArmed)
			dispatch({ type: "SET_SHIFT_ARMED", value: false });
	};
	const running = runningCount(runtime, dynamic.id) > 0;
	const status = definitionStatus(runtime, dynamic.id);
	const previewCycleMillis = dynamicPreviewCycleMillis(
		dynamic.body,
		speedGroupBpms,
	);
	const changeView = (next: DynamicEditorView) => {
		onViewChange?.(next);
		updateEditor({ task: next, encoderPage: 1 });
	};
	const addLane = (attribute: string) => {
		setAddingLane(false);
		void onMutate(dynamic, {
			type: "add_lane",
			lane: createDefaultDynamicLane(attribute),
			index: null,
		});
	};
	const takeSelection = () =>
		onMutate(dynamic, {
			type: "set_target_binding",
			target_binding: selectedGroupId
				? {
						type: "live_group",
						group_id: selectedGroupId,
					}
				: {
						type: "frozen_targets",
						targets: [...selection],
					},
		});
	const clearSelection = () =>
		onMutate(dynamic, {
			type: "set_target_binding",
			target_binding: { type: "targetless" },
		});
	const contentSidebar = (
		<DynamicSelectionPreview
			dynamic={dynamic}
			previewPhase={previewing ? previewPhase : 0}
			selection={selection}
			positions={stageLayout.positions}
			positions3d={stageLayout.positions3d}
		/>
	);
	const contentFooter =
		view === "phase" ? (
			<DynamicPhaseQuickControls
				phase={dynamic.body.phase}
				running={running}
				selectionCount={selection.length}
				targetless={dynamic.body.target_binding.type === "targetless"}
				onPhasePatch={(patch) =>
					void onMutate(dynamic, {
						type: "set_phase",
						phase: { ...dynamic.body.phase, ...patch },
					})
				}
				onTakeSelection={() => void takeSelection()}
				onClearSelection={() => void clearSelection()}
			/>
		) : null;
	useEffect(() => {
		if (
			primaryLane &&
			dynamic.body.lanes.some((candidate) => candidate.id === primaryLane)
		)
			return;
		const nextLane = dynamic.body.lanes[0]?.id ?? null;
		setPrimaryLane(nextLane);
		setSelectedLanes(new Set(nextLane ? [nextLane] : []));
		updateEditor({ primaryLaneId: nextLane, primaryKeyframeIndex: 0 });
	}, [dynamic.body.lanes, primaryLane, updateEditor]);
	useEffect(() => {
		const sessionLane =
			session?.dynamicId === dynamic.id ? session.primaryLaneId : null;
		if (
			!sessionLane ||
			sessionLane === primaryLane ||
			!dynamic.body.lanes.some((candidate) => candidate.id === sessionLane)
		)
			return;
		setPrimaryLane(sessionLane);
		setSelectedLanes(new Set([sessionLane]));
	}, [dynamic.body.lanes, dynamic.id, primaryLane, session]);
	useEffect(() => {
		if (!previewing) return;
		let frame = 0;
		const startedAt = performance.now() - previewPhase * previewCycleMillis;
		const animate = (now: number) => {
			setPreviewPhase(
				((now - startedAt) % previewCycleMillis) / previewCycleMillis,
			);
			frame = requestAnimationFrame(animate);
		};
		frame = requestAnimationFrame(animate);
		return () => cancelAnimationFrame(frame);
	}, [previewing, previewCycleMillis]);
	useEffect(() => {
		openEditor({
			dynamicId: dynamic.id,
			task: view,
			encoderPage,
			primaryLaneId: primaryLane,
			primaryKeyframeIndex,
		});
	}, [
		dynamic.id,
		encoderPage,
		openEditor,
		primaryKeyframeIndex,
		primaryLane,
		view,
	]);

	return (
		<section
			className={`dynamics-window dynamics-editor dynamic-full-discussion-editor ${compact ? "compact" : ""}`}
			aria-busy={busy}
		>
			<WindowHeader
				title={`Dynamic ${dynamic.body.pool_number}`}
				info={{
					primary: dynamic.body.name,
					secondary: `${dynamic.body.lanes.length} ${dynamic.body.lanes.length === 1 ? "lane" : "lanes"}`,
				}}
				actions={[
					view === "curves"
						? [
								{
									id: "add-lane",
									label: "+ Add Lane",
									onClick: () => setAddingLane(true),
								},
							]
						: [],
					[
						{
							id: "curves",
							label: "Lanes",
							active: view === "curves",
							onClick: () => changeView("curves"),
						},
						{
							id: "phase",
							label: "Phase",
							active: view === "phase",
							onClick: () => changeView("phase"),
						},
						{
							id: "speed",
							label: "Speed",
							active: view === "speed",
							onClick: () => changeView("speed"),
						},
					],
					[
						{
							id: "preview",
							label: previewing ? "■ Stop" : "▶ Preview",
							active: previewing,
							variant: previewing ? "danger" : "success",
							className: "dynamic-preview-toggle",
							onClick: () =>
								setPreviewing((current) => {
									if (current) setPreviewPhase(0);
									return !current;
								}),
						},
					],
					[{ id: "back", label: "← Back to Pool", onClick: onBack }],
				]}
				settings
				onSettings={(anchor) =>
					setSettingsAnchor(anchor.getBoundingClientRect())
				}
			/>
			{settingsAnchor && (
				<WindowSettings
					modal={false}
					anchor={settingsAnchor}
					title="Dynamic Settings"
					onClose={() => setSettingsAnchor(null)}
					tabs={[
						{
							id: "general",
							label: "General",
							content: (
								<FormLayout labelPlacement="side">
									<TextField
										key={`name-${dynamic.revision}`}
										label="Name"
										defaultValue={dynamic.body.name}
										maxLength={128}
										onBlur={(event) => {
											const name = event.target.value.trim();
											if (name && name !== dynamic.body.name)
												void onMutate(dynamic, { type: "set_name", name });
										}}
									/>
									<IconPickerField
										label="Icon"
										value={dynamic.body.icon ?? "∿"}
										onChange={(icon) =>
											void onMutate(dynamic, { type: "set_icon", icon })
										}
									/>
									<ColorPickerField
										label="Color"
										value={dynamic.body.color ?? "#4edcff"}
										onChange={(color) =>
											void onMutate(dynamic, { type: "set_color", color })
										}
									/>
								</FormLayout>
							),
						},
						{
							id: "targets",
							label: "Targets",
							content: (
								<section className="dynamic-target-settings">
									<strong>{targetSummary(dynamic.body)}</strong>
									{status && <small>{coverageSummary(status)}</small>}
									{status?.warning && (
										<small className="dynamics-warning">{status.warning}</small>
									)}
									<div>
										<Button
											disabled={running || selection.length === 0}
											title={
												running
													? "Turn every running instance Off before changing targets"
													: selection.length === 0
														? "Select a Group or fixtures first"
														: undefined
											}
											onClick={() => takeSelection()}
										>
											Take Selection
										</Button>
										<Button
											disabled={
												running ||
												dynamic.body.target_binding.type === "targetless"
											}
											title={
												running
													? "Turn every running instance Off before changing targets"
													: undefined
											}
											onClick={() => clearSelection()}
										>
											Clear Selection
										</Button>
									</div>
								</section>
							),
						},
					]}
				/>
			)}
			{addingLane && (
				<LaneAttributeModal
					id={`select-lane-attribute-${dynamic.id}`}
					title="Select lane attribute"
					details="Choose the attribute controlled by the new lane"
					attributes={attributes}
					busy={busy}
					onClose={() => setAddingLane(false)}
					onChoose={addLane}
				/>
			)}
			{error && (
				<p className="dynamics-error" role="alert">
					{error}
				</p>
			)}
			<div className="dynamics-editor-body">
				<main className="dynamic-workspace">
					{view === "curves" &&
						(lane ? (
							<CurvesView
								dynamic={dynamic}
								lane={lane}
								selectedLanes={selectedLanes}
								shiftArmed={appState.shiftArmed}
								attributes={attributes}
								primaryKeyframeIndex={primaryKeyframeIndex}
								previewPhase={previewing ? previewPhase : null}
								contentSidebar={contentSidebar}
								onPrimaryKeyframeIndex={setPrimaryKeyframeIndex}
								onSelect={selectLane}
								onReplace={replaceLane}
								onMutate={onMutate}
							/>
						) : (
							<div className="dynamic-view-with-sidebar">
								<section className="dynamic-empty-lanes">
									<Button onClick={() => setAddingLane(true)}>
										Add first lane
									</Button>
								</section>
								{contentSidebar}
							</div>
						))}
					{view === "phase" && (
						<div className="dynamic-view-with-sidebar">
							<PhaseView
								dynamic={dynamic}
								lane={lane}
								running={running}
								selectionCount={selection.length}
								onSelectLane={(id) => selectLane(id, false)}
								onTakeSelection={takeSelection}
								onClearSelection={clearSelection}
								onMutate={onMutate}
							/>
							{contentSidebar}
						</div>
					)}
					{view === "speed" && (
						<div className="dynamic-view-with-sidebar">
							<SpeedView
								dynamic={dynamic}
								runtime={runtime}
								previewPhase={previewing ? previewPhase : null}
								speedGroupBpms={speedGroupBpms}
								onMutate={onMutate}
								onSpeedGroupTap={onSpeedGroupTap}
							/>
							{contentSidebar}
						</div>
					)}
				</main>
				{contentFooter}
			</div>
		</section>
	);
}

function DynamicSelectionPreview({
	dynamic,
	previewPhase,
	selection,
	positions,
	positions3d,
}: {
	dynamic: DynamicObject;
	previewPhase: number;
	selection: readonly string[];
	positions: Record<string, { x: number; y: number; rotation: number }>;
	positions3d: Record<string, { x: number; z: number }>;
}) {
	const selected = selection.length > 0;
	const previewPositions = useMemo(
		() =>
			selected
				? normalizeSelectedPreviewPositions(selection, positions, positions3d)
				: virtualFixturePreviewPositions(),
		[selected, selection, positions, positions3d],
	);
	const phaseOffsets = useMemo(
		() => dynamicPreviewPhaseOffsets(dynamic.body.phase, previewPositions),
		[dynamic.body.phase, previewPositions],
	);
	return (
		<aside
			className="dynamic-face-preview-sidebar dynamic-discussion-preview-sidebar"
			aria-label="Selection preview"
		>
			<header>
				<span>
					<strong>{selected ? "Selected fixtures" : "Virtual fixtures"}</strong>
					<small>
						{selected
							? "Top-down Stage projection"
							: "Front-end-only 20 × 20 grid"}
					</small>
				</span>
				<b>{selected ? selection.length : 400}</b>
			</header>
			<div
				className={`dynamic-face-fixture-field ${selected ? "selected-fixtures" : "virtual-fixtures"}`}
				role="img"
				aria-label={
					selected
						? `Top-down preview of ${selection.length} selected fixtures`
						: "Front-end-only preview of 400 virtual fixtures"
				}
			>
				{previewPositions.map(({ id, left, top }, index) => {
					const values = dynamicPreviewValues(
						dynamic.body,
						moduloOne(previewPhase + (phaseOffsets[index] ?? 0)),
					);
					return (
						<i
							key={id}
							className="dynamic-face-fixture"
							style={
								{
									left: `${left}%`,
									top: `${top}%`,
									"--fixture-color": `rgb(${Math.round(values.red * 255)} ${Math.round(values.green * 255)} ${Math.round(values.blue * 255)})`,
									"--fixture-intensity": values.intensity,
									"--pan": `${(values.pan - 0.5) * 56}%`,
									"--tilt": `${(values.tilt - 0.5) * 56}%`,
								} as CSSProperties
							}
						>
							<span />
						</i>
					);
				})}
			</div>
		</aside>
	);
}

function normalizeSelectedPreviewPositions(
	selection: readonly string[],
	positions: Record<string, { x: number; y: number }>,
	positions3d: Record<string, { x: number; z: number }>,
) {
	const fallback = gridFixturePreviewPositions(selection.length);
	const resolved = selection.map((id, index) => {
		const position3d = positions3d[id];
		if (position3d)
			return { id, x: position3d.x, z: position3d.z, fallback: false };
		const position2d = positions[id];
		if (position2d)
			return { id, x: position2d.x, z: position2d.y, fallback: false };
		return {
			id,
			x: fallback[index]?.left ?? 50,
			z: fallback[index]?.top ?? 50,
			fallback: true,
		};
	});
	const positioned = resolved.filter((position) => !position.fallback);
	if (positioned.length === 0) return fallback;
	const xs = positioned.map(({ x }) => x);
	const zs = positioned.map(({ z }) => z);
	const minX = Math.min(...xs);
	const maxX = Math.max(...xs);
	const minZ = Math.min(...zs);
	const maxZ = Math.max(...zs);
	return resolved.map((position, index) =>
		position.fallback
			? (fallback[index] ?? { id: position.id, left: 50, top: 50 })
			: {
					id: position.id,
					left: normalizePreviewCoordinate(position.x, minX, maxX),
					top: normalizePreviewCoordinate(position.z, minZ, maxZ),
				},
	);
}

function normalizePreviewCoordinate(
	value: number,
	minimum: number,
	maximum: number,
) {
	if (maximum === minimum) return 50;
	return 8 + ((value - minimum) / (maximum - minimum)) * 84;
}

function gridFixturePreviewPositions(count: number) {
	const columns = Math.max(1, Math.ceil(Math.sqrt(count)));
	const rows = Math.max(1, Math.ceil(count / columns));
	return Array.from({ length: count }, (_, index) => ({
		id: `fixture-${index}`,
		left: columns === 1 ? 50 : 8 + (index % columns) * (84 / (columns - 1)),
		top: rows === 1 ? 50 : 8 + Math.floor(index / columns) * (84 / (rows - 1)),
	}));
}

function virtualFixturePreviewPositions() {
	return Array.from({ length: 400 }, (_, index) => ({
		id: `virtual-fixture-${index}`,
		left: 5 + (index % 20) * (90 / 19),
		top: 5 + Math.floor(index / 20) * (90 / 19),
	}));
}

function dynamicPreviewPhaseOffsets(
	phase: DynamicDefinitionProjection["phase"],
	positions: readonly { id: string; left: number; top: number }[],
) {
	const ordered = positions.map((position, index) => ({ position, index }));
	const center = { left: 50, top: 50 };
	switch (phase.ordering.type) {
		case "grid_linear": {
			const radians = (phase.ordering.angle_degrees * Math.PI) / 180;
			ordered.sort(
				(left, right) =>
					left.position.left * Math.cos(radians) +
					left.position.top * Math.sin(radians) -
					(right.position.left * Math.cos(radians) +
						right.position.top * Math.sin(radians)),
			);
			break;
		}
		case "radial_out":
		case "radial_in":
			ordered.sort((left, right) => {
				const distance = (item: (typeof ordered)[number]) =>
					Math.hypot(
						item.position.left - center.left,
						item.position.top - center.top,
					);
				const comparison = distance(left) - distance(right);
				return phase.ordering.type === "radial_in" ? -comparison : comparison;
			});
			break;
		case "axial":
			ordered.sort(
				(left, right) =>
					Math.atan2(
						left.position.top - center.top,
						left.position.left - center.left,
					) -
					Math.atan2(
						right.position.top - center.top,
						right.position.left - center.left,
					),
			);
			break;
		case "random_each_loop": {
			const seed = phase.ordering.seed;
			ordered.sort(
				(left, right) =>
					previewHash(left.position.id, seed) -
					previewHash(right.position.id, seed),
			);
			break;
		}
		case "selection":
			break;
	}
	const blockSize = Math.max(1, Math.round(phase.block_size));
	const rankCount = Math.ceil(ordered.length / blockSize);
	const repeats = Math.min(Math.max(1, Math.round(phase.repeats)), rankCount);
	const offsets = Array.from({ length: positions.length }, () => 0);
	for (const [spatialRank, entry] of ordered.entries()) {
		const rank = Math.floor(spatialRank / blockSize);
		const { local, length } = balancedPreviewRepeat(rank, rankCount, repeats);
		const wingLocal = phase.wings
			? Math.min(local, Math.max(0, length - 1 - local))
			: local;
		const effectiveLength = phase.wings ? Math.ceil(length / 2) : length;
		const distributed =
			phase.anchors_degrees.length >= 2
				? previewAnchorPhase(phase.anchors_degrees, wingLocal, effectiveLength)
				: effectiveLength <= 1
					? 0
					: (wingLocal / effectiveLength) * phase.span_degrees;
		offsets[entry.index] = (phase.offset_degrees + distributed) / 360;
	}
	return offsets;
}

function balancedPreviewRepeat(rank: number, count: number, repeats: number) {
	const base = Math.floor(count / repeats);
	const extras = count % repeats;
	let start = 0;
	for (let repeat = 0; repeat < repeats; repeat += 1) {
		const length = base + (repeat < extras ? 1 : 0);
		if (rank < start + length) return { local: rank - start, length };
		start += length;
	}
	return { local: 0, length: 1 };
}

function previewAnchorPhase(
	anchors: readonly number[],
	local: number,
	length: number,
) {
	if (length <= 1) return anchors[0] ?? 0;
	const progress = local / length;
	const position = progress * (anchors.length - 1);
	const leftIndex = Math.min(Math.floor(position), anchors.length - 2);
	const mix = position - leftIndex;
	return (
		(anchors[leftIndex] ?? 0) * (1 - mix) + (anchors[leftIndex + 1] ?? 0) * mix
	);
}

function previewHash(value: string, seed: number) {
	let hash = seed | 0;
	for (const character of value)
		hash = Math.imul(hash ^ character.charCodeAt(0), 16_777_619);
	return hash >>> 0;
}

function dynamicPreviewValues(
	dynamic: DynamicDefinitionProjection,
	phase: number,
) {
	const values = new Map<string, number>();
	for (const lane of dynamic.lanes)
		values.set(lane.attribute, lanePreviewValue(lane, dynamic, phase));
	const hasColor = [...values.keys()].some((attribute) =>
		attribute.startsWith("color."),
	);
	return {
		intensity: clamp(values.get("intensity") ?? 1, 0, 1),
		red: clamp(values.get("color.red") ?? (hasColor ? 0 : 0.2), 0, 1),
		green: clamp(values.get("color.green") ?? (hasColor ? 0 : 0.78), 0, 1),
		blue: clamp(values.get("color.blue") ?? 1, 0, 1),
		pan: clamp(values.get("pan") ?? 0.5, 0, 1),
		tilt: clamp(values.get("tilt") ?? 0.5, 0, 1),
	};
}

function lanePreviewValue(
	lane: DynamicLaneProjection,
	dynamic: DynamicDefinitionProjection,
	phase: number,
) {
	const intervalPhase = moduloOne(phase * rationalValue(lane.speed_multiplier));
	const functionName =
		lane.mode === "middle_amplitude"
			? lane.middle_amplitude.function
			: lane.mode === "max_min"
				? lane.max_min.function
				: null;
	const width = functionName === "pwm" ? 1 : clamp(lane.width, 0.05, 1);
	const position = clamp((intervalPhase - (1 - width) / 2) / width, 0, 1);
	if (lane.mode === "keyframes")
		return keyframePreviewValue(lane.keyframes.points, position);
	if (lane.mode === "random") {
		const group = dynamic.random_groups.find(
			(candidate) => candidate.id === lane.random_group_id,
		);
		if (!group) return 0;
		const low = scalarSourceCurveValue(group.low);
		const high = scalarSourceCurveValue(group.high);
		return previewHash(lane.id, Math.floor(position * 1_000_000)) % 2
			? high
			: low;
	}
	const shape = periodicPreviewValue(
		functionName ?? "sinus",
		position,
		lane.mode === "middle_amplitude"
			? lane.middle_amplitude.pwm
			: lane.max_min.pwm,
	);
	if (lane.mode === "middle_amplitude") {
		const middle = scalarSourceCurveValue(lane.middle_amplitude.middle);
		return clamp(
			middle +
				(shape * 2 - 1) *
					lane.middle_amplitude.amplitude *
					lane.middle_amplitude.size,
			0,
			1,
		);
	}
	const minimum = scalarSourceCurveValue(lane.max_min.minimum);
	const maximum = scalarSourceCurveValue(lane.max_min.maximum);
	const middle = (minimum + maximum) / 2;
	return clamp(
		middle +
			(minimum + (maximum - minimum) * shape - middle) * lane.max_min.size,
		0,
		1,
	);
}

function keyframePreviewValue(
	points: readonly {
		position: number;
		source: DynamicScalarSourceProjection;
		interpolation: string;
	}[],
	position: number,
) {
	if (points.length === 0) return 0;
	let leftIndex = 0;
	for (let index = 0; index < points.length; index += 1) {
		if ((points[index]?.position ?? 1) > position) break;
		leftIndex = index;
	}
	const left = points[leftIndex] ?? points[0];
	const right = points[leftIndex + 1] ?? points[0];
	if (!left || !right) return 0;
	const rightPosition = leftIndex + 1 < points.length ? right.position : 1;
	const progress = clamp(
		(position - left.position) /
			Math.max(Number.EPSILON, rightPosition - left.position),
		0,
		1,
	);
	const mix = interpolationPreviewValue(progress, left.interpolation);
	return clamp(
		scalarSourceCurveValue(left.source) * (1 - mix) +
			scalarSourceCurveValue(right.source) * mix,
		0,
		1,
	);
}

function periodicPreviewValue(
	functionName: DynamicPeriodicFunctionProjection,
	position: number,
	pwm: DynamicLaneProjection["max_min"]["pwm"],
) {
	switch (functionName) {
		case "linear_up":
			return position;
		case "linear_down":
			return 1 - position;
		case "cosinus":
			return (Math.cos(position * Math.PI * 2) + 1) / 2;
		case "pwm":
			return pwmPreviewValue(position, pwm);
		case "sinus":
			return (Math.sin(position * Math.PI * 2 - Math.PI / 2) + 1) / 2;
	}
}

function pwmPreviewValue(
	position: number,
	pwm: DynamicLaneProjection["max_min"]["pwm"],
) {
	const total = Math.max(Number.EPSILON, pwm.on + pwm.off);
	const onEnd = pwm.on / total;
	const attackEnd = Math.min(pwm.attack / total, onEnd);
	const decayEnd = Math.min(1, onEnd + pwm.decay / total);
	if (attackEnd > 0 && position < attackEnd)
		return interpolationPreviewValue(
			position / attackEnd,
			pwm.attack_interpolation,
		);
	if (position < onEnd) return 1;
	if (decayEnd > onEnd && position < decayEnd)
		return (
			1 -
			interpolationPreviewValue(
				(position - onEnd) / (decayEnd - onEnd),
				pwm.decay_interpolation,
			)
		);
	return 0;
}

function interpolationPreviewValue(progress: number, interpolation: string) {
	switch (interpolation) {
		case "ease_in":
			return progress * progress;
		case "ease_out":
			return 1 - (1 - progress) * (1 - progress);
		case "ease_in_out":
			return progress * progress * (3 - 2 * progress);
		case "hold":
			return 0;
		case "drop":
			return progress >= 1 ? 1 : 0;
		default:
			return progress;
	}
}

function moduloOne(value: number) {
	return ((value % 1) + 1) % 1;
}

function dynamicPreviewCycleMillis(
	dynamic: DynamicDefinitionProjection,
	speedGroupBpms?: Partial<Record<SpeedGroupId, number>>,
) {
	const baseCycleMillis =
		dynamic.speed.type === "fixed"
			? dynamic.speed.duration_millis
			: (60_000 / Math.max(1, speedGroupBpms?.[dynamic.speed.group] ?? 120)) *
				rationalValue(dynamic.speed.beats_per_cycle);
	return Math.max(
		1,
		baseCycleMillis /
			Math.max(Number.EPSILON, rationalValue(dynamic.overall_speed_multiplier)),
	);
}

function DynamicPhaseQuickControls({
	phase,
	running,
	selectionCount,
	targetless,
	onPhasePatch,
	onTakeSelection,
	onClearSelection,
}: {
	phase: DynamicObject["body"]["phase"];
	running: boolean;
	selectionCount: number;
	targetless: boolean;
	onPhasePatch(patch: Partial<DynamicObject["body"]["phase"]>): void;
	onTakeSelection(): void;
	onClearSelection(): void;
}) {
	const orderingValue =
		phase.ordering.type === "radial_in" ? "radial_out" : phase.ordering.type;
	return (
		<fieldset
			className="dynamic-discussion-phase-controls"
			aria-label="Phase quick controls"
		>
			<div className="dynamic-discussion-phase-ordering">
				<MultiValueToggle
					ariaLabel="Ordering mode"
					value={orderingValue}
					options={[
						{ value: "selection", label: "Linear" },
						{ value: "grid_linear", label: "Grid" },
						{ value: "radial_out", label: "Radial" },
						{ value: "axial", label: "Radar" },
						{ value: "random_each_loop", label: "Random" },
					]}
					onChange={(type) => {
						const center =
							phase.ordering.type === "radial_out" ||
							phase.ordering.type === "radial_in" ||
							phase.ordering.type === "axial"
								? {
										center_x: phase.ordering.center_x,
										center_z: phase.ordering.center_z,
									}
								: { center_x: 0, center_z: 0 };
						onPhasePatch({
							ordering:
								type === "grid_linear"
									? { type, angle_degrees: 90 }
									: type === "radial_out" || type === "axial"
										? { type, ...center }
										: type === "random_each_loop"
											? { type, seed: 201 }
											: { type: "selection" },
						});
					}}
				/>
			</div>
			<div className="dynamic-discussion-phase-actions">
				<Button
					disabled={running || selectionCount === 0}
					onClick={onTakeSelection}
				>
					Take Selection
				</Button>
				<Button disabled={running || targetless} onClick={onClearSelection}>
					Clear Selection
				</Button>
			</div>
		</fieldset>
	);
}

function CurvesView({
	dynamic,
	lane,
	selectedLanes,
	shiftArmed,
	attributes,
	primaryKeyframeIndex,
	previewPhase,
	contentSidebar,
	onPrimaryKeyframeIndex,
	onSelect,
	onReplace,
	onMutate,
}: {
	dynamic: DynamicObject;
	lane: DynamicLaneProjection;
	selectedLanes: ReadonlySet<string>;
	shiftArmed: boolean;
	attributes: readonly { id: string; label: string; family: string }[];
	primaryKeyframeIndex: number;
	previewPhase: number | null;
	contentSidebar?: ReactNode;
	onPrimaryKeyframeIndex(index: number): void;
	onSelect(id: string, additive: boolean): void;
	onReplace(next: DynamicLaneProjection): Promise<void>;
	onMutate(
		dynamic: DynamicObject,
		intent: DynamicUpdateIntent,
		mutationGroup?: string,
	): Promise<void>;
}) {
	const [attributeLaneId, setAttributeLaneId] = useState<string | null>(null);
	const [openLaneMenuId, setOpenLaneMenuId] = useState<string | null>(null);
	const [draggingKeyframe, setDraggingKeyframe] = useState<{
		laneId: string;
		index: number;
		pointerId: number;
		mutationGroup: string;
		grabOffsetX: number;
	} | null>(null);
	const [randomMethod, setRandomMethod] = useState<
		"max_min" | "middle_amplitude"
	>("max_min");
	const setMode = async (mode: DynamicLaneModeProjection) => {
		if (mode !== "random") {
			await onReplace({ ...lane, mode });
			return;
		}
		let groupId = lane.random_group_id;
		if (
			!groupId ||
			!dynamic.body.random_groups.some((group) => group.id === groupId)
		) {
			const group = defaultRandomGroup();
			await onMutate(dynamic, { type: "add_random_group", group });
			groupId = group.id;
		}
		await onReplace({ ...lane, mode, random_group_id: groupId });
	};
	const displayedMethod = lane.mode === "random" ? randomMethod : lane.mode;
	const chooseMethod = (mode: "keyframes" | "max_min" | "middle_amplitude") => {
		if (lane.mode === "random" && mode !== "keyframes") {
			setRandomMethod(mode);
			return;
		}
		void setMode(mode);
	};
	const chooseFunction = (
		functionName: DynamicPeriodicFunctionProjection | "random",
	) => {
		if (functionName === "random") {
			if (displayedMethod !== "keyframes") setRandomMethod(displayedMethod);
			void setMode("random");
			return;
		}
		const method =
			displayedMethod === "middle_amplitude" ? "middle_amplitude" : "max_min";
		void onReplace(
			normalizePwmLane(
				method === "middle_amplitude"
					? {
							...lane,
							mode: method,
							middle_amplitude: {
								...lane.middle_amplitude,
								function: functionName,
							},
						}
					: {
							...lane,
							mode: method,
							max_min: { ...lane.max_min, function: functionName },
						},
			),
		);
	};
	const attributeLane = attributeLaneId
		? dynamic.body.lanes.find((candidate) => candidate.id === attributeLaneId)
		: undefined;
	const keyframeIndex = Math.min(
		primaryKeyframeIndex,
		Math.max(0, lane.keyframes.points.length - 1),
	);
	const selectedFunction =
		lane.mode === "random"
			? "random"
			: lane.mode === "middle_amplitude"
				? lane.middle_amplitude.function
				: lane.max_min.function;
	const moveKeyframe = (
		candidate: DynamicLaneProjection,
		index: number,
		clientX: number,
		timeline: HTMLElement,
		mutationGroup: string,
		repetitions: number,
		grabOffsetX: number,
	) => {
		if (index === 0) return;
		const bounds = timeline.getBoundingClientRect();
		const previous = candidate.keyframes.points[index - 1]?.position ?? 0;
		const next = candidate.keyframes.points[index + 1]?.position ?? 0.999;
		const renderedPercent =
			((clientX - grabOffsetX - bounds.left) / Math.max(1, bounds.width)) * 100;
		const position = clamp(
			((renderedPercent - 0.8) / 98.4) * repetitions,
			previous + 0.01,
			next - 0.01,
		);
		const points = candidate.keyframes.points.map((point, pointIndex) =>
			pointIndex === index ? { ...point, position } : point,
		);
		void onMutate(
			dynamic,
			{
				type: "replace_lane",
				lane_id: candidate.id,
				lane: {
					...candidate,
					keyframes: { ...candidate.keyframes, points },
				},
			},
			mutationGroup,
		);
	};
	return (
		<div
			className={`dynamic-curves-view ${contentSidebar ? "has-content-sidebar" : ""}`}
		>
			<ul className="dynamic-lane-overview-list" aria-label="Dynamic lanes">
				{dynamic.body.lanes.map((candidate, index) => {
					const attribute =
						attributes.find((item) => item.id === candidate.attribute) ?? null;
					const selected = selectedLanes.has(candidate.id);
					const preview = lanePreview(candidate, dynamic.body.lanes);
					return (
						<li
							key={candidate.id}
							aria-current={candidate.id === lane.id}
							className={`dynamic-lane-overview ${candidate.id === lane.id ? "primary" : ""} ${selected ? "selected" : ""}`}
						>
							<div className="dynamic-lane-content">
								<button
									type="button"
									className="dynamic-lane-identity-select"
									aria-label={`Select lane ${index + 1}, ${attribute?.label ?? candidate.attribute}`}
									aria-pressed={selected}
									onClick={(event) =>
										onSelect(candidate.id, event.shiftKey || shiftArmed)
									}
								>
									<span className="dynamic-lane-identity">
										<small>Lane {index + 1}</small>
										<strong>{attribute?.label ?? candidate.attribute}</strong>
										<span>
											{modeLabel(candidate.mode)}
											{laneSpeedLabel(candidate)}
										</span>
									</span>
								</button>
								<div className="dynamic-lane-curve">
									<button
										type="button"
										className="dynamic-lane-curve-select"
										aria-label={`Select ${attribute?.label ?? candidate.attribute} lane from curve`}
										aria-pressed={selected}
										onClick={(event) =>
											onSelect(candidate.id, event.shiftKey || shiftArmed)
										}
									>
										<svg
											viewBox="0 0 1000 200"
											preserveAspectRatio="none"
											role="img"
											aria-label={`${attribute?.label ?? candidate.attribute}: ${modeLabel(candidate.mode)}`}
										>
											<title>{modeLabel(candidate.mode)}</title>
											<path
												className="grid"
												d="M0 50H1000M0 100H1000M0 150H1000M250 0V200M500 0V200M750 0V200"
											/>
											<path className="curve" d={preview.primaryPath} />
											{preview.repeatedPath && (
												<path
													className="curve repeated"
													d={preview.repeatedPath}
												/>
											)}
											{preview.repetitions > 1 && (
												<path
													className="repeat-boundary"
													d={`M${1000 / preview.repetitions} 0V200`}
												/>
											)}
										</svg>
										{preview.repetitions > 1 && (
											<span
												className="dynamic-repeat-label"
												style={{ left: `${100 / preview.repetitions}%` }}
											>
												repeat
											</span>
										)}
										{previewPhase !== null && (
											<i
												className="dynamic-preview-playhead"
												style={{ left: `${previewPhase * 100}%` }}
											/>
										)}
										<span className="dynamic-lane-axis start">0%</span>
										<span className="dynamic-lane-axis middle">50%</span>
										<span className="dynamic-lane-axis end">100%</span>
									</button>
									{candidate.mode === "keyframes" && (
										<span className="dynamic-keyframe-marks">
											{candidate.keyframes.points.map((point, pointIndex) => (
												<Button
													key={`${candidate.id}-${pointIndex}`}
													aria-label={`${attribute?.label ?? candidate.attribute} keyframe ${keyframeName(pointIndex)}`}
													className={
														candidate.id === lane.id &&
														pointIndex === keyframeIndex
															? "selected"
															: ""
													}
													style={{
														left: `${keyframePreviewPercent(point.position, preview.repetitions)}%`,
														top: `${keyframePreviewTop(point.source)}%`,
													}}
													onPointerDown={(event) => {
														event.preventDefault();
														event.stopPropagation();
														onSelect(candidate.id, false);
														onPrimaryKeyframeIndex(pointIndex);
														if (pointIndex === 0) return;
														event.currentTarget.setPointerCapture(
															event.pointerId,
														);
														setDraggingKeyframe({
															laneId: candidate.id,
															index: pointIndex,
															pointerId: event.pointerId,
															mutationGroup: crypto.randomUUID(),
															grabOffsetX:
																event.clientX -
																(event.currentTarget.getBoundingClientRect()
																	.left +
																	event.currentTarget.getBoundingClientRect()
																		.width /
																		2),
														});
													}}
													onPointerMove={(event) => {
														if (
															!draggingKeyframe ||
															draggingKeyframe.laneId !== candidate.id ||
															draggingKeyframe.index !== pointIndex ||
															draggingKeyframe.pointerId !== event.pointerId
														)
															return;
														const timeline = event.currentTarget.parentElement;
														if (timeline)
															moveKeyframe(
																candidate,
																pointIndex,
																event.clientX,
																timeline,
																draggingKeyframe.mutationGroup,
																preview.repetitions,
																draggingKeyframe.grabOffsetX,
															);
													}}
													onPointerUp={(event) => {
														if (draggingKeyframe?.pointerId === event.pointerId)
															setDraggingKeyframe(null);
													}}
													onPointerCancel={() => setDraggingKeyframe(null)}
												>
													<span>{keyframeName(pointIndex)}</span>
												</Button>
											))}
											<i
												className="loop-close"
												style={{
													left: `${keyframePreviewPercent(1, preview.repetitions)}%`,
													top: `${keyframePreviewTop(candidate.keyframes.points[0]?.source)}%`,
												}}
											>
												<span>A′</span>
											</i>
										</span>
									)}
								</div>
							</div>
							<div className="dynamic-lane-row-actions">
								<Button
									iconOnly
									className="dynamic-lane-settings-trigger"
									icon="⚙"
									aria-label={`${attribute?.label ?? candidate.attribute} lane settings`}
									aria-expanded={openLaneMenuId === candidate.id}
									onClick={() =>
										setOpenLaneMenuId((current) =>
											current === candidate.id ? null : candidate.id,
										)
									}
								/>
							</div>
							{openLaneMenuId === candidate.id && (
								<div
									className="dynamic-lane-menu"
									role="menu"
									aria-label={`${attribute?.label ?? candidate.attribute} lane menu`}
								>
									<Button
										role="menuitem"
										onClick={() => {
											setOpenLaneMenuId(null);
											setAttributeLaneId(candidate.id);
										}}
									>
										<span aria-hidden="true">✎</span>
										Change attribute
									</Button>
									<Button
										role="menuitem"
										variant="danger"
										disabled={dynamic.body.lanes.length <= 1}
										title={
											dynamic.body.lanes.length <= 1
												? "A Dynamic requires at least one lane."
												: undefined
										}
										onClick={() => {
											setOpenLaneMenuId(null);
											void onMutate(dynamic, {
												type: "delete_lane",
												lane_id: candidate.id,
											});
										}}
									>
										<span aria-hidden="true">⌫</span>
										Delete lane
									</Button>
									<Button
										iconOnly
										className="dynamic-lane-menu-close"
										icon="×"
										aria-label="Close lane settings"
										onClick={() => setOpenLaneMenuId(null)}
									/>
								</div>
							)}
						</li>
					);
				})}
			</ul>
			{contentSidebar}
			{attributeLane && (
				<LaneAttributeModal
					id={`change-lane-attribute-${attributeLane.id}`}
					title="Change lane attribute"
					details="Choose the attribute controlled by this lane"
					currentAttribute={attributeLane.attribute}
					attributes={attributes}
					onClose={() => setAttributeLaneId(null)}
					onChoose={(nextAttribute) => {
						const target = dynamic.body.lanes.find(
							(candidate) => candidate.id === attributeLaneId,
						);
						if (!target) return;
						setAttributeLaneId(null);
						void onMutate(dynamic, {
							type: "replace_lane",
							lane_id: target.id,
							lane: { ...target, attribute: nextAttribute },
						});
					}}
				/>
			)}
			<section
				className="dynamic-lane-bottom-editor"
				aria-label="Curve Composer"
			>
				<CyclingValueToggle
					className="dynamic-curve-method-cycle"
					ariaLabel="Curve method"
					value={displayedMethod}
					options={curveComposerMethods}
					onChange={chooseMethod}
				/>
				<FadedDivider
					orientation="vertical"
					className="dynamic-curve-composer-divider"
				/>
				{displayedMethod === "keyframes" ? (
					<div
						className="dynamic-keyframe-choice-list"
						role="group"
						aria-label="Selected keyframe"
					>
						{lane.keyframes.points.map((point, index) => {
							const name = keyframeName(index);
							const position = Math.round(point.position * 100);
							const source = scalarSourceEncoderDisplay(point.source);
							return (
								<Button
									key={`${lane.id}-${index}`}
									className="dynamic-keyframe-choice"
									active={index === keyframeIndex}
									aria-pressed={index === keyframeIndex}
									aria-label={`${name}, ${position}%, ${source}`}
									onClick={() => onPrimaryKeyframeIndex(index)}
								>
									<b aria-hidden="true">{name}</b>
									<span aria-hidden="true">{position}%</span>
									<small aria-hidden="true">{source}</small>
								</Button>
							);
						})}
						<span
							className="dynamic-keyframe-choice loop-close"
							aria-label="A prime, 100%, alias of A"
							role="note"
						>
							<b aria-hidden="true">A′</b>
							<span aria-hidden="true">100%</span>
							<small aria-hidden="true">Alias of A</small>
						</span>
					</div>
				) : (
					<GroupedSelectionField
						className="dynamic-composer-choice"
						ariaLabel={`Curve function: ${laneShapeLabel(lane)}`}
						dialogTitle="Choose curve function"
						value={selectedFunction}
						groups={curveFunctionSelectionGroups()}
						onChange={chooseFunction}
					/>
				)}
				{displayedMethod === "keyframes" && (
					<>
						<Button
							size="compact"
							variant="danger"
							disabled={
								keyframeIndex === 0 || lane.keyframes.points.length <= 2
							}
							aria-label={`Delete keyframe ${keyframeName(keyframeIndex)}`}
							title={
								keyframeIndex === 0
									? "The first keyframe also closes the loop and cannot be deleted."
									: lane.keyframes.points.length <= 2
										? "A curve requires at least two keyframes."
										: undefined
							}
							onClick={() => {
								const next = deleteKeyframeFromLane(lane, keyframeIndex);
								void onReplace(next);
								onPrimaryKeyframeIndex(Math.max(0, keyframeIndex - 1));
							}}
						>
							Delete Keyframe
						</Button>
						<Button
							size="compact"
							onClick={() => {
								const previousPositions = new Set(
									lane.keyframes.points.map((point) => point.position),
								);
								const next = addKeyframeToLane(lane);
								const nextIndex = next.keyframes.points.findIndex(
									(point) => !previousPositions.has(point.position),
								);
								void onReplace(next);
								if (nextIndex >= 0) onPrimaryKeyframeIndex(nextIndex);
							}}
						>
							+ Keyframe
						</Button>
					</>
				)}
			</section>
		</div>
	);
}

function addKeyframeToLane(lane: DynamicLaneProjection): DynamicLaneProjection {
	const points = [...lane.keyframes.points];
	const position = largestKeyframeGapMidpoint(points);
	points.push({
		position,
		source: sourceCurrent,
		interpolation: "ease_in_out",
	});
	points.sort((left, right) => left.position - right.position);
	return {
		...lane,
		keyframes: { ...lane.keyframes, points },
	};
}

function deleteKeyframeFromLane(
	lane: DynamicLaneProjection,
	index: number,
): DynamicLaneProjection {
	if (index <= 0 || lane.keyframes.points.length <= 2) return lane;
	return {
		...lane,
		keyframes: {
			...lane.keyframes,
			points: lane.keyframes.points.filter(
				(_, pointIndex) => pointIndex !== index,
			),
		},
	};
}

function PhaseView({
	dynamic,
	lane,
	running,
	selectionCount,
	onSelectLane,
	onTakeSelection,
	onClearSelection,
	onMutate,
}: {
	dynamic: DynamicObject;
	lane?: DynamicLaneProjection;
	running: boolean;
	selectionCount: number;
	onSelectLane(id: string): void;
	onTakeSelection(): void;
	onClearSelection(): void;
	onMutate(dynamic: DynamicObject, intent: DynamicUpdateIntent): void;
}) {
	const phaseMode = dynamic.body.phase_mode;
	const phase =
		phaseMode === "per_lane"
			? (lane?.phase ?? dynamic.body.phase)
			: dynamic.body.phase;
	const spatialOrdering = isSpatialOrdering(phase.ordering)
		? phase.ordering
		: null;
	const update = (patch: Partial<typeof phase>) => {
		const nextPhase = { ...phase, ...patch };
		if (phaseMode === "per_lane" && lane) {
			onMutate(dynamic, {
				type: "replace_lane",
				lane_id: lane.id,
				lane: { ...lane, phase: nextPhase },
			});
			return;
		}
		onMutate(dynamic, { type: "set_phase", phase: nextPhase });
	};
	return (
		<div className="dynamic-phase-view">
			<section className="dynamic-phase-controls">
				{phaseMode === "per_lane" && lane && (
					<SelectField
						className="dynamic-phase-lane"
						label="Lane"
						ariaLabel="Dynamic phase lane"
						value={lane.id}
						options={dynamic.body.lanes.map((candidate, index) => ({
							value: candidate.id,
							label: `Lane ${index + 1} · ${candidate.attribute}`,
						}))}
						onChange={onSelectLane}
					/>
				)}
				<FormLayout labelPlacement="top">
					<div className="dynamic-phase-ordering-field">
						<MultiValueToggleField
							className="dynamic-phase-ordering"
							label="Ordering mode"
							value={
								phase.ordering.type === "radial_in"
									? "radial_out"
									: phase.ordering.type
							}
							options={[
								{ value: "selection", label: "Linear" },
								{ value: "grid_linear", label: "Grid" },
								{ value: "radial_out", label: "Radial" },
								{ value: "axial", label: "Radar" },
								{ value: "random_each_loop", label: "Random" },
							]}
							onChange={(ordering) =>
								update({ ordering: orderingFor(ordering, phase.ordering) })
							}
						/>
					</div>
					{phase.ordering.type === "grid_linear" && (
						<div className="dynamic-phase-field dynamic-phase-direction-field">
							<NumberField
								label="Direction"
								value={phase.ordering.angle_degrees}
								allowDecimal
								unit="°"
								onValueChange={(angle_degrees) =>
									update({
										ordering: {
											type: "grid_linear",
											angle_degrees: Number(angle_degrees),
										},
									})
								}
							/>
						</div>
					)}
					{spatialOrdering && (
						<>
							<div className="dynamic-phase-field dynamic-phase-center-x-field">
								<NumberField
									label="Center X"
									value={spatialOrdering.center_x}
									allowDecimal
									onValueChange={(center_x) =>
										update({
											ordering: {
												type: spatialOrdering.type,
												center_x: Number(center_x),
												center_z: spatialOrdering.center_z,
											},
										})
									}
								/>
							</div>
							<div className="dynamic-phase-field dynamic-phase-center-z-field">
								<NumberField
									label="Center Z"
									value={spatialOrdering.center_z}
									allowDecimal
									onValueChange={(center_z) =>
										update({
											ordering: {
												type: spatialOrdering.type,
												center_x: spatialOrdering.center_x,
												center_z: Number(center_z),
											},
										})
									}
								/>
							</div>
						</>
					)}
					<div className="dynamic-phase-shared-fields">
						<div className="dynamic-phase-field dynamic-phase-offset-field">
							<NumberField
								label="Offset"
								value={phase.offset_degrees}
								allowDecimal
								unit="°"
								onValueChange={(offset_degrees) =>
									update({ offset_degrees: Number(offset_degrees) })
								}
							/>
						</div>
						<div className="dynamic-phase-field dynamic-phase-span-field">
							<NumberField
								label="Span"
								value={phase.span_degrees}
								allowDecimal
								unit="°"
								onValueChange={(span_degrees) =>
									update({ span_degrees: Number(span_degrees) })
								}
							/>
						</div>
						<div className="dynamic-phase-field dynamic-phase-blocks-field">
							<NumberField
								label="Blocks"
								value={phase.block_size}
								min={1}
								onValueChange={(block_size) =>
									update({ block_size: Math.max(1, Number(block_size)) })
								}
							/>
						</div>
						<div className="dynamic-phase-field dynamic-phase-repeats-field">
							<NumberField
								label="Repeats"
								value={phase.repeats}
								min={1}
								onValueChange={(repeats) =>
									update({ repeats: Math.max(1, Number(repeats)) })
								}
							/>
						</div>
						<div className="dynamic-phase-field dynamic-phase-wings-field">
							<SwitchField
								label="Wings"
								checked={phase.wings}
								onChange={(event) => update({ wings: event.target.checked })}
							/>
						</div>
						<div className="dynamic-phase-field dynamic-phase-anchors-field">
							<TextField
								className="dynamic-phase-anchors"
								key={phase.anchors_degrees.join(",")}
								label="Explicit anchors"
								defaultValue={phase.anchors_degrees.join(" THRU ")}
								placeholder="Automatic"
								onBlur={(event) => {
									const text = event.target.value.trim();
									if (!text) {
										update({ anchors_degrees: [] });
										return;
									}
									const anchors = text.split(/\s*(?:THRU|,)\s*/i).map(Number);
									if (anchors.every(Number.isFinite))
										update({ anchors_degrees: anchors });
								}}
							/>
						</div>
					</div>
				</FormLayout>
				<footer className="dynamic-phase-footer">
					<div className="dynamic-phase-target-actions">
						<Button
							disabled={running || selectionCount === 0}
							onClick={onTakeSelection}
						>
							Take Selection
						</Button>
						<Button
							disabled={
								running || dynamic.body.target_binding.type === "targetless"
							}
							onClick={onClearSelection}
						>
							Clear Selection
						</Button>
					</div>
					<fieldset className="button-group" aria-label="Phase span presets">
						{[180, 360, 720].map((span) => (
							<Button
								key={span}
								active={phase.span_degrees === span}
								onClick={() => update({ span_degrees: span })}
							>
								{span}°
							</Button>
						))}
					</fieldset>
				</footer>
			</section>
		</div>
	);
}

function SpeedView({
	dynamic,
	runtime,
	previewPhase,
	speedGroupBpms,
	onMutate,
	onSpeedGroupTap,
}: {
	dynamic: DynamicObject;
	runtime: DynamicRuntimeSnapshotProjection | null;
	previewPhase: number | null;
	speedGroupBpms?: Partial<Record<SpeedGroupId, number>>;
	onMutate(dynamic: DynamicObject, intent: DynamicUpdateIntent): void;
	onSpeedGroupTap?(group: SpeedGroupId): void;
}) {
	const speed = dynamic.body.speed;
	const instances =
		runtime?.instances.filter(
			(instance) => instance.dynamic_id === dynamic.id,
		) ?? [];
	const primaryRuntime =
		instances.find((instance) =>
			instance.controllers.some((controller) => controller.winning),
		) ?? instances[0];
	const runtimeState = primaryRuntime
		? primaryRuntime.pending
			? "Pending"
			: primaryRuntime.paused || !primaryRuntime.phase_advancing
				? "Paused"
				: "Running"
		: "Off";
	const fixedBpm =
		speed.type === "fixed"
			? Math.max(1, Math.round(60_000 / speed.duration_millis))
			: null;
	const beatPhase = previewPhase ?? primaryRuntime?.beat_phase ?? 0;
	const displayedBpm =
		speed.type === "fixed"
			? (fixedBpm ?? 120)
			: (speedGroupBpms?.[speed.group] ?? primaryRuntime?.effective_bpm ?? 120);
	const tapTimes = useRef<number[]>([]);
	const tapTempo = () => {
		if (speed.type === "speed_group") {
			onSpeedGroupTap?.(speed.group);
			return;
		}
		const now = performance.now();
		const previous = tapTimes.current.at(-1);
		if (previous == null || now - previous > 2_000) tapTimes.current = [now];
		else tapTimes.current = [...tapTimes.current.slice(-4), now];
		if (tapTimes.current.length < 2) return;
		const intervals = tapTimes.current
			.slice(1)
			.map((time, index) => time - tapTimes.current[index]);
		const average =
			intervals.reduce((sum, interval) => sum + interval, 0) / intervals.length;
		onMutate(dynamic, {
			type: "set_speed",
			speed: {
				type: "fixed",
				duration_millis: Math.max(1, Math.round(average)),
			},
		});
	};
	return (
		<div className="dynamic-speed-view">
			<section className="dynamic-speed-transport">
				<div
					className="dynamic-beat-grid"
					role="img"
					aria-label={`Speed transport beat grid, phase ${Math.round(clamp(beatPhase, 0, 1) * 100)}%`}
				>
					<i style={{ left: `${clamp(beatPhase, 0, 1) * 100}%` }} />
					<b className="dynamic-beat-phase">
						Phase {Math.round(clamp(beatPhase, 0, 1) * 100)}%
					</b>
					{Array.from({ length: 16 }, (_, index) => (
						<span key={index} className={index % 4 === 0 ? "bar-start" : ""}>
							{(index % 4) + 1}
						</span>
					))}
				</div>
				<MultiValueToggle
					ariaLabel="Speed source"
					value={speed.type}
					options={[
						{ value: "fixed", label: "Fixed BPM" },
						{ value: "speed_group", label: "Speed Group" },
					]}
					onChange={(type) =>
						onMutate(dynamic, {
							type: "set_speed",
							speed:
								type === "fixed"
									? { type: "fixed", duration_millis: 500 }
									: {
											type: "speed_group",
											group: "A",
											beats_per_cycle: { numerator: 4, denominator: 1 },
										},
						})
					}
				/>
				{speed.type === "fixed" ? (
					<div className="dynamic-fixed-speed-fields">
						<NumberField
							label="Tempo"
							value={fixedBpm ?? 120}
							min={1}
							max={999}
							unit="BPM"
							onValueChange={(bpm) =>
								onMutate(dynamic, {
									type: "set_speed",
									speed: {
										type: "fixed",
										duration_millis: Math.max(
											1,
											Math.round(60_000 / Math.max(1, Number(bpm))),
										),
									},
								})
							}
						/>
					</div>
				) : (
					<div className="dynamic-speed-source-fields">
						<SelectField
							label="Speed Group"
							value={speed.group}
							options={["A", "B", "C", "D", "E"].map((group) => ({
								value: group,
								label: `Speed Group ${group}`,
							}))}
							onChange={(group) =>
								onMutate(dynamic, {
									type: "set_speed",
									speed: { ...speed, group },
								})
							}
						/>
						<NumberField
							label="Beats per cycle"
							value={speed.beats_per_cycle.numerator}
							min={1}
							onValueChange={(numerator) =>
								onMutate(dynamic, {
									type: "set_speed",
									speed: {
										...speed,
										beats_per_cycle: {
											numerator: Math.max(1, Math.round(Number(numerator))),
											denominator: 1,
										},
									},
								})
							}
						/>
					</div>
				)}
				<Button
					className="dynamic-tap-tempo"
					aria-label={
						speed.type === "fixed"
							? `Tap fixed tempo, ${Math.round(displayedBpm)} BPM`
							: `Tap Speed Group ${speed.group} tempo, ${Math.round(displayedBpm)} BPM`
					}
					onClick={tapTempo}
				>
					<strong className="speed-group-value">
						{Math.round(displayedBpm)} BPM
					</strong>
					<span className="speed-group-label">
						{speed.type === "fixed" ? "TAP" : `TAP GROUP ${speed.group}`}
					</span>
				</Button>
				<footer>
					<span>
						{runtimeState} · transport {Math.round(beatPhase * 100)}%
					</span>
					{primaryRuntime?.aliasing_warning && (
						<small>{primaryRuntime.aliasing_warning}</small>
					)}
				</footer>
			</section>
			<section className="dynamic-speed-controls">
				<FormLayout labelPlacement="top">
					<NumberField
						label="Multiplier"
						value={rationalValue(dynamic.body.overall_speed_multiplier)}
						min={0.0625}
						max={16}
						step={0.25}
						allowDecimal
						unit="×"
						onValueChange={(multiplier) =>
							onMutate(dynamic, {
								type: "set_overall_speed_multiplier",
								multiplier: rationalFromNumber(Number(multiplier)),
							})
						}
					/>
					<SelectField
						label="Run mode"
						description="Loop repeats continuously. One-shot stops after one complete cycle."
						value={dynamic.body.run_mode}
						options={[
							{ value: "loop", label: "Loop" },
							{ value: "one_shot", label: "One-shot" },
						]}
						onChange={(run_mode) =>
							onMutate(dynamic, { type: "set_run_mode", run_mode })
						}
					/>
					<SelectField
						label="Activation"
						description="Chooses when a newly started Dynamic instance enters its first cycle."
						value={dynamic.body.default_activation}
						options={[
							{ value: "start_now", label: "Start now" },
							{
								value: "join_sync_now",
								label: "Join sync now",
								disabled: speed.type !== "speed_group",
							},
							{
								value: "next_boundary",
								label: "Next boundary",
								disabled: speed.type !== "speed_group",
							},
						]}
						onChange={(activation) =>
							onMutate(dynamic, { type: "set_activation", activation })
						}
					/>
					<SelectField
						label="Boundary"
						description="For Next boundary, wait for the next beat or the next four-beat bar."
						value={dynamic.body.activation_boundary}
						disabled={
							speed.type !== "speed_group" ||
							dynamic.body.default_activation !== "next_boundary"
						}
						options={[
							{ value: "beat", label: "Next beat" },
							{ value: "bar", label: "Next bar (4 beats)" },
						]}
						onChange={(boundary) =>
							onMutate(dynamic, {
								type: "set_activation_boundary",
								boundary,
							})
						}
					/>
				</FormLayout>
				<small>
					{instances.length} active{" "}
					{instances.length === 1 ? "instance" : "instances"} ·{" "}
					{primaryRuntime?.speed_source ??
						(speed.type === "speed_group"
							? `Speed Group ${speed.group}`
							: `${fixedBpm} BPM`)}
				</small>
			</section>
		</div>
	);
}

export function DynamicEncoderDeck({
	view,
	page = 1,
	lane,
	dynamic,
	presets = [],
	keyframeIndex = 0,
	onKeyframeIndex = () => undefined,
	onLaneChange,
	onMutate,
}: {
	view: DynamicEditorView;
	page?: number;
	lane?: DynamicLaneProjection;
	dynamic: DynamicDefinitionProjection;
	presets?: readonly PresetObject[];
	keyframeIndex?: number;
	onKeyframeIndex?(index: number): void;
	onLaneChange(
		update: (lane: DynamicLaneProjection) => DynamicLaneProjection,
		mutationGroup?: string,
	): Promise<void>;
	onMutate(intent: DynamicUpdateIntent, mutationGroup?: string): Promise<void>;
}) {
	const { state: appState } = useApp();
	const hardwareAttached = useHardwareConnected();
	const hardwareConnected = Boolean(hardwareAttached || appState.midiProfile);
	const hardwareDisplays = useRef<Array<HardwareEncoderDisplayHandle | null>>(
		[],
	);
	const gesture = useRef<{
		key: string;
		id: string;
		lastSampleAt: number;
	} | null>(null);
	const accumulated = useRef(
		new Map<string, { observedBase: number; value: number }>(),
	);
	const phase =
		dynamic.phase_mode === "per_lane"
			? (lane?.phase ?? dynamic.phase)
			: dynamic.phase;
	const applyPhase = (
		nextPhase: DynamicDefinitionProjection["phase"],
		group?: string,
	) =>
		dynamic.phase_mode === "per_lane" && lane
			? onLaneChange((current) => ({ ...current, phase: nextPhase }), group)
			: onMutate({ type: "set_phase", phase: nextPhase }, group);
	const slots: DynamicEncoderSlot[] =
		view === "curves"
			? curveEditorEncoderSlots(
					lane,
					dynamic,
					keyframeIndex,
					onKeyframeIndex,
					onLaneChange,
					presets,
				)
			: view === "phase"
				? [
						{
							id: "offset",
							label: "Offset",
							display: `${phase.offset_degrees}°`,
							value: phase.offset_degrees,
							minimum: -10_000,
							maximum: 10_000,
							inputScale: 1,
							fineStep: 5,
							coarseStep: 45,
							apply: (value, group) =>
								applyPhase({ ...phase, offset_degrees: value }, group),
							applyRange: (values, group) =>
								applyPhase(phaseWithExplicitRange(phase, values), group),
						},
						{
							id: "span",
							label: "Span",
							display: `${phase.span_degrees}°`,
							value: phase.span_degrees,
							minimum: -10_000,
							maximum: 10_000,
							inputScale: 1,
							fineStep: 5,
							coarseStep: 45,
							apply: (value, group) =>
								applyPhase(
									{ ...phase, span_degrees: value, anchors_degrees: [] },
									group,
								),
							applyRange: (values, group) =>
								applyPhase(phaseWithExplicitRange(phase, values), group),
						},
						{
							id: "blocks",
							label: "Blocks",
							display: String(phase.block_size),
							value: phase.block_size,
							minimum: 1,
							maximum: 10_000,
							inputScale: 1,
							fineStep: 1,
							coarseStep: 5,
							apply: (value, group) =>
								applyPhase(
									{ ...phase, block_size: Math.max(1, Math.round(value)) },
									group,
								),
						},
						{
							id: "repeats",
							label: "Repeats",
							display: String(phase.repeats),
							value: phase.repeats,
							minimum: 1,
							maximum: 10_000,
							inputScale: 1,
							fineStep: 1,
							coarseStep: 5,
							apply: (value, group) =>
								applyPhase(
									{ ...phase, repeats: Math.max(1, Math.round(value)) },
									group,
								),
						},
						{
							id: "wings",
							label: "Wings",
							display: phase.wings ? "On" : "Off",
							value: phase.wings ? 1 : 0,
							minimum: 0,
							maximum: 1,
							inputScale: 1,
							fineStep: 1,
							coarseStep: 1,
							choices: encoderChoices("Wings", phase.wings ? 1 : 0, [
								{ label: "Off" },
								{ label: "On" },
							]),
							apply: (value, group) =>
								applyPhase({ ...phase, wings: value >= 0.5 }, group),
						},
						phaseDirectionSlot(phase, applyPhase),
					]
				: speedEncoderSlots(dynamic, onMutate);
	const items: EncoderSectionItem[] = slots.map((slot, index) => ({
		id: slot.id,
		slot: index + 1,
		target: {
			label: slot.label,
			display: slot.display,
		},
		value: slot.value,
		minimum: slot.minimum,
		maximum: slot.maximum,
		inputScale: slot.inputScale,
		slowStep: slot.fineStep,
		fastStep: slot.coarseStep,
		repeatSeconds: 0.08,
		disabled: slot.disabled,
		presets: slot.choices ?? slot.presets,
		range: Boolean(slot.applyRange),
		touchInteraction: slot.choices ? "choices" : undefined,
	}));
	const slotsRef = useRef(slots);
	slotsRef.current = slots;
	const groupFor = useCallback((key: string) => {
		const now = performance.now();
		if (
			!gesture.current ||
			gesture.current.key !== key ||
			now - gesture.current.lastSampleAt > 250
		)
			gesture.current = {
				key,
				id: crypto.randomUUID(),
				lastSampleAt: now,
			};
		else gesture.current.lastSampleAt = now;
		return gesture.current.id;
	}, []);
	const applyRelative = useCallback(
		(id: string, delta: number, undoGroup?: string | null) => {
			const slot = slotsRef.current.find((candidate) => candidate.id === id);
			if (!slot || slot.disabled) return;
			const key = `${view}:${id}`;
			const current = accumulated.current.get(key);
			const externalChange =
				current &&
				slot.value !== current.observedBase &&
				slot.value !== current.value;
			const base = !current || externalChange ? slot.value : current.value;
			const next = clamp(base + delta, slot.minimum, slot.maximum);
			accumulated.current.set(key, {
				observedBase: slot.value,
				value: next,
			});
			const group = undoGroup ?? groupFor(id);
			void slot.apply(next, group);
		},
		[groupFor, view],
	);
	const applyAbsolute = useCallback(
		(id: string, value: number) => {
			const slot = slotsRef.current.find((candidate) => candidate.id === id);
			if (!slot || slot.disabled) return;
			const next = clamp(value, slot.minimum, slot.maximum);
			accumulated.current.set(`${view}:${id}`, {
				observedBase: slot.value,
				value: next,
			});
			void slot.apply(next, crypto.randomUUID());
		},
		[view],
	);
	const applyRange = useCallback((id: string, points: number[]) => {
		const slot = slotsRef.current.find((candidate) => candidate.id === id);
		if (!slot?.applyRange || slot.disabled) return;
		const scale = slot.inputScale || 1;
		void slot.applyRange(
			points.map((point) => clamp(point / scale, slot.minimum, slot.maximum)),
			crypto.randomUUID(),
		);
	}, []);
	const selectPreset = useCallback((id: string, value: string) => {
		const slot = slotsRef.current.find((candidate) => candidate.id === id);
		if (!slot?.selectPreset || slot.disabled) return;
		void slot.selectPreset(value, crypto.randomUUID());
	}, []);
	useEffect(() => {
		accumulated.current.clear();
		gesture.current = null;
	}, [dynamic.id]);
	useEffect(() => {
		if (!hardwareConnected) return;
		const handleEncoder = (event: Event) => {
			const detail = (event as CustomEvent<{ control: string; value?: string }>)
				.detail;
			const slotNumber = Number(detail.control.split("/")[1]);
			const slot = slotsRef.current[slotNumber - 1];
			if (!slot || slot.disabled) return;
			if (detail.value === "press") {
				hardwareDisplays.current[slotNumber - 1]?.activate();
				return;
			}
			const direction =
				detail.value === "up" || detail.value === "right"
					? 1
					: detail.value === "down" || detail.value === "left"
						? -1
						: 0;
			if (!direction) return;
			const coarse = detail.value === "left" || detail.value === "right";
			applyRelative(
				slot.id,
				direction * (coarse ? slot.coarseStep : slot.fineStep),
				groupFor(slot.id),
			);
		};
		window.addEventListener("light:encoder-action", handleEncoder);
		return () =>
			window.removeEventListener("light:encoder-action", handleEncoder);
	}, [applyRelative, groupFor, hardwareConnected]);
	return (
		<div className="dynamic-encoder-deck">
			<EncoderSection
				showHeader={false}
				model={{
					id: `dynamics-${view}-${page}`,
					label: `${view === "curves" ? "Lanes" : view === "phase" ? "Phase" : "Speed"} encoders`,
					description: "Turn fine · press-turn coarse · center Set Value",
					encoders: items,
				}}
				surface={hardwareConnected ? "hardware" : "touch"}
				callbacks={{
					onRelativeChange: applyRelative,
					onAbsoluteChange: applyAbsolute,
					onRangeChange: applyRange,
					onPresetSelect: selectPreset,
					onHardwareDisplayRef: (slot, handle) => {
						hardwareDisplays.current[slot - 1] = handle;
					},
				}}
			/>
		</div>
	);
}

interface DynamicEncoderSlot {
	id: string;
	label: string;
	display: string;
	value: number;
	minimum: number;
	maximum: number;
	inputScale: number;
	fineStep: number;
	coarseStep: number;
	disabled?: boolean;
	choices?: NonNullable<EncoderSectionItem["presets"]>;
	presets?: NonNullable<EncoderSectionItem["presets"]>;
	apply(value: number, mutationGroup: string): Promise<void>;
	applyRange?(values: number[], mutationGroup: string): Promise<void>;
	selectPreset?(value: string, mutationGroup: string): Promise<void>;
}

function encoderChoices(
	groupLabel: string,
	selectedValue: number,
	options: readonly {
		label: string;
		description?: string;
		disabled?: boolean;
	}[],
): NonNullable<EncoderSectionItem["presets"]> {
	return {
		selectedValue: String(selectedValue),
		groups: [
			{
				label: groupLabel,
				options: options.map((option, index) => ({
					value: String(index),
					label: option.label,
					description: option.description,
					disabled: option.disabled,
				})),
			},
		],
	};
}

function curveEditorEncoderSlots(
	lane: DynamicLaneProjection | undefined,
	dynamic: DynamicDefinitionProjection,
	keyframeIndex: number,
	onKeyframeIndex: (index: number) => void,
	onLaneChange: (
		update: (lane: DynamicLaneProjection) => DynamicLaneProjection,
		mutationGroup?: string,
	) => Promise<void>,
	presets: readonly PresetObject[],
): DynamicEncoderSlot[] {
	const disabled = !lane;
	const unassigned = (id: string): DynamicEncoderSlot => ({
		id,
		label: "Unassigned",
		display: "—",
		value: 0,
		minimum: 0,
		maximum: 1,
		inputScale: 1,
		fineStep: 0.01,
		coarseStep: 0.1,
		disabled: true,
		apply: async () => undefined,
	});
	const sourceSlot = (
		id: string,
		label: string,
		source: DynamicScalarSourceProjection | undefined,
		replace: (
			lane: DynamicLaneProjection,
			source: DynamicScalarSourceProjection,
		) => DynamicLaneProjection,
	): DynamicEncoderSlot => ({
		id,
		label,
		display: scalarSourceEncoderDisplay(source),
		value: scalarSourceEncoderValue(source),
		minimum: 0,
		maximum: 1,
		inputScale: 100,
		fineStep: 0.01,
		coarseStep: 0.1,
		disabled,
		presets: source
			? scalarSourcePresetChoices(presets, lane?.attribute ?? "", source)
			: undefined,
		apply: (value, group) =>
			onLaneChange(
				(item) =>
					normalizePwmLane(
						replace(item, { type: "value", value: clamp(value, 0, 1) }),
					),
				group,
			),
		selectPreset: (value, group) =>
			onLaneChange(
				(item) =>
					normalizePwmLane(
						replace(item, scalarSourceFromPresetChoice(value, item.attribute)),
					),
				group,
			),
	});
	const speedSlot: DynamicEncoderSlot = {
		id: "lane-speed",
		label: "Speed",
		display: lane
			? `${lane.speed_multiplier.numerator}/${lane.speed_multiplier.denominator}`
			: "—",
		value: lane ? rationalValue(lane.speed_multiplier) : 1,
		minimum: 0.0625,
		maximum: 16,
		inputScale: 1,
		fineStep: 0.0625,
		coarseStep: 0.5,
		disabled,
		apply: (value, group) =>
			onLaneChange(
				(item) => ({
					...item,
					speed_multiplier: rationalFromNumber(value),
				}),
				group,
			),
	};
	const widthSlot: DynamicEncoderSlot = {
		id: "curve-width",
		label: "Curve width",
		display: lane ? `${Math.round(lane.width * 100)}%` : "—",
		value: lane?.width ?? 1,
		minimum: 0.05,
		maximum: 1,
		inputScale: 100,
		fineStep: 0.01,
		coarseStep: 0.1,
		disabled,
		apply: (value, group) =>
			onLaneChange(
				(item) => ({ ...item, width: clamp(value, 0.05, 1) }),
				group,
			),
	};
	if (!lane || lane.mode === "keyframes") {
		const resolvedIndex = Math.min(
			keyframeIndex,
			Math.max(0, (lane?.keyframes.points.length ?? 1) - 1),
		);
		const point = lane?.keyframes.points[resolvedIndex];
		return [
			{
				id: "keyframe",
				label: "Keyframe",
				display: point
					? `${keyframeName(resolvedIndex)} · ${resolvedIndex + 1}/${lane?.keyframes.points.length ?? 1}`
					: "—",
				value: resolvedIndex,
				minimum: 0,
				maximum: Math.max(0, (lane?.keyframes.points.length ?? 1) - 1),
				inputScale: 1,
				fineStep: 1,
				coarseStep: 1,
				disabled,
				choices: encoderChoices(
					"Keyframe",
					resolvedIndex,
					(lane?.keyframes.points ?? []).map((_, index) => ({
						label: keyframeName(index),
						description: `${index + 1}/${lane?.keyframes.points.length ?? 1}`,
					})),
				),
				apply: async (value) =>
					onKeyframeIndex(
						wrappedIndex(value, lane?.keyframes.points.length ?? 1),
					),
			},
			sourceSlot(
				"keyframe-value",
				point?.source.type === "value" ? "Value" : "Source value",
				point?.source,
				(item, source) => ({
					...item,
					keyframes: {
						...item.keyframes,
						points: item.keyframes.points.map((candidate, index) =>
							index === resolvedIndex ? { ...candidate, source } : candidate,
						),
					},
				}),
			),
			{
				id: "keyframe-time",
				label: "Keyframe time",
				display: point ? `${Math.round(point.position * 100)}%` : "—",
				value: point?.position ?? 0,
				minimum: 0,
				maximum: 1,
				inputScale: 100,
				fineStep: 0.01,
				coarseStep: 0.1,
				disabled: disabled || resolvedIndex === 0,
				apply: (value, group) =>
					onLaneChange((item) => {
						const previous =
							item.keyframes.points[resolvedIndex - 1]?.position ?? 0;
						const next =
							item.keyframes.points[resolvedIndex + 1]?.position ?? 0.999;
						return {
							...item,
							keyframes: {
								...item.keyframes,
								points: item.keyframes.points.map((candidate, index) =>
									index === resolvedIndex
										? {
												...candidate,
												position: clamp(value, previous + 0.01, next - 0.01),
											}
										: candidate,
								),
							},
						};
					}, group),
			},
			{
				id: "interpolation",
				label: "Interpolation",
				display: lane ? primaryInterpolationLabel(lane) : "—",
				value: lane ? primaryInterpolationIndex(lane) : 0,
				minimum: 0,
				maximum: interpolations.length - 1,
				inputScale: 1,
				fineStep: 1,
				coarseStep: 1,
				disabled,
				choices: encoderChoices(
					"Interpolation",
					lane ? primaryInterpolationIndex(lane) : 0,
					[
						{ label: "Linear" },
						{ label: "Ease in" },
						{ label: "Ease out" },
						{ label: "Ease in + out" },
						{ label: "Hold" },
						{ label: "Drop" },
					],
				),
				apply: (value, group) =>
					onLaneChange((item) => setPrimaryInterpolation(item, value), group),
			},
			widthSlot,
			speedSlot,
		];
	}
	if (lane.mode === "middle_amplitude") {
		const valueSlots: DynamicEncoderSlot[] = [
			sourceSlot(
				"middle",
				"Middle",
				lane.middle_amplitude.middle,
				(item, middle) => ({
					...item,
					middle_amplitude: { ...item.middle_amplitude, middle },
				}),
			),
			{
				id: "amplitude",
				label: "Amplitude",
				display: `${Math.round(lane.middle_amplitude.amplitude * 100)}%`,
				value: lane.middle_amplitude.amplitude,
				minimum: 0,
				maximum: 1,
				inputScale: 100,
				fineStep: 0.01,
				coarseStep: 0.1,
				apply: (value, group) =>
					onLaneChange(
						(item) => ({
							...item,
							middle_amplitude: {
								...item.middle_amplitude,
								amplitude: clamp(value, 0, 1),
							},
						}),
						group,
					),
			},
		];
		return lane.middle_amplitude.function === "pwm"
			? [
					...middleAmplitudePwmValueSlots(lane, onLaneChange),
					...pwmEncoderSlots(lane, onLaneChange),
					speedSlot,
				]
			: [
					...valueSlots,
					unassigned("middle-unassigned-1"),
					unassigned("middle-unassigned-2"),
					widthSlot,
					speedSlot,
				];
	}
	if (lane.mode === "random") {
		const group = dynamic.random_groups.find(
			(candidate) => candidate.id === lane.random_group_id,
		);
		return [
			{
				...sourceSlot("random-low", "Low", group?.low, (item) => item),
				disabled: true,
			},
			{
				...sourceSlot("random-high", "High", group?.high, (item) => item),
				disabled: true,
			},
			unassigned("random-unassigned-1"),
			unassigned("random-unassigned-2"),
			widthSlot,
			speedSlot,
		];
	}
	const valueSlots: DynamicEncoderSlot[] = [
		sourceSlot("maximum", "Top", lane.max_min.maximum, (item, maximum) => ({
			...item,
			max_min: { ...item.max_min, maximum },
		})),
		sourceSlot("minimum", "Bottom", lane.max_min.minimum, (item, minimum) => ({
			...item,
			max_min: { ...item.max_min, minimum },
		})),
	];
	return lane.max_min.function === "pwm"
		? [...valueSlots, ...pwmEncoderSlots(lane, onLaneChange), speedSlot]
		: [
				...valueSlots,
				unassigned("bounds-unassigned-1"),
				unassigned("bounds-unassigned-2"),
				widthSlot,
				speedSlot,
			];
}

function middleAmplitudePwmValueSlots(
	lane: DynamicLaneProjection,
	onLaneChange: (
		update: (lane: DynamicLaneProjection) => DynamicLaneProjection,
		mutationGroup?: string,
	) => Promise<void>,
): DynamicEncoderSlot[] {
	const middle = scalarSourceEncoderValue(lane.middle_amplitude.middle);
	const top = clamp(middle + lane.middle_amplitude.amplitude, 0, 1);
	const bottom = clamp(middle - lane.middle_amplitude.amplitude, 0, 1);
	const slot = (
		field: "top" | "bottom",
		value: number,
	): DynamicEncoderSlot => ({
		id: `pwm-${field}`,
		label: field === "top" ? "Top" : "Bottom",
		display: `${Math.round(value * 100)}%`,
		value,
		minimum: 0,
		maximum: 1,
		inputScale: 100,
		fineStep: 0.01,
		coarseStep: 0.1,
		apply: (nextValue, group) =>
			onLaneChange((item) => {
				const currentMiddle = scalarSourceEncoderValue(
					item.middle_amplitude.middle,
				);
				const currentTop = clamp(
					currentMiddle + item.middle_amplitude.amplitude,
					0,
					1,
				);
				const currentBottom = clamp(
					currentMiddle - item.middle_amplitude.amplitude,
					0,
					1,
				);
				const nextTop =
					field === "top" ? clamp(nextValue, currentBottom, 1) : currentTop;
				const nextBottom =
					field === "bottom" ? clamp(nextValue, 0, currentTop) : currentBottom;
				return normalizePwmLane({
					...item,
					middle_amplitude: {
						...item.middle_amplitude,
						middle: {
							type: "value",
							value: (nextTop + nextBottom) / 2,
						},
						amplitude: (nextTop - nextBottom) / 2,
					},
				});
			}, group),
	});
	return [slot("top", top), slot("bottom", bottom)];
}

function normalizePwmLane(lane: DynamicLaneProjection) {
	const functionName =
		lane.mode === "middle_amplitude"
			? lane.middle_amplitude.function
			: lane.mode === "max_min"
				? lane.max_min.function
				: null;
	if (functionName !== "pwm") return lane;
	const pwm =
		lane.mode === "middle_amplitude"
			? lane.middle_amplitude.pwm
			: lane.max_min.pwm;
	const on = clamp(pwm.on, 0, 1);
	const normalizedPwm = {
		...pwm,
		attack: clamp(pwm.attack, 0, on),
		on,
		decay: clamp(pwm.decay, 0, 1 - on),
		off: 1 - on,
	};
	return lane.mode === "middle_amplitude"
		? {
				...lane,
				width: 1,
				middle_amplitude: {
					...lane.middle_amplitude,
					pwm: normalizedPwm,
				},
			}
		: {
				...lane,
				width: 1,
				max_min: { ...lane.max_min, pwm: normalizedPwm },
			};
}

function pwmEncoderSlots(
	lane: DynamicLaneProjection,
	onLaneChange: (
		update: (lane: DynamicLaneProjection) => DynamicLaneProjection,
		mutationGroup?: string,
	) => Promise<void>,
): DynamicEncoderSlot[] {
	const pwm =
		lane.mode === "middle_amplitude"
			? lane.middle_amplitude.pwm
			: lane.max_min.pwm;
	const slot = (
		field: "attack" | "on" | "decay",
		label: string,
	): DynamicEncoderSlot => ({
		id: `pwm-${field}`,
		label,
		display: `${Math.round(pwm[field] * 100)}%`,
		value: pwm[field],
		minimum: 0,
		maximum: 1,
		inputScale: 100,
		fineStep: 0.01,
		coarseStep: 0.1,
		apply: (value, group) =>
			onLaneChange((item) => setLanePwmValue(item, field, value), group),
	});
	return [slot("attack", "Attack"), slot("on", "On"), slot("decay", "Decay")];
}

function setLanePwmValue(
	lane: DynamicLaneProjection,
	field: "attack" | "on" | "decay",
	value: number,
): DynamicLaneProjection {
	const pwm =
		lane.mode === "middle_amplitude"
			? lane.middle_amplitude.pwm
			: lane.max_min.pwm;
	const nextPwm = { ...pwm };
	if (field === "attack") nextPwm.attack = clamp(value, 0, nextPwm.on);
	if (field === "on") {
		nextPwm.on = clamp(value, nextPwm.attack, 1 - nextPwm.decay);
		nextPwm.off = 1 - nextPwm.on;
	}
	if (field === "decay") nextPwm.decay = clamp(value, 0, nextPwm.off);
	return lane.mode === "middle_amplitude"
		? {
				...lane,
				width: 1,
				middle_amplitude: { ...lane.middle_amplitude, pwm: nextPwm },
			}
		: {
				...lane,
				width: 1,
				max_min: { ...lane.max_min, pwm: nextPwm },
			};
}

function scalarSourceEncoderValue(
	source: DynamicScalarSourceProjection | undefined,
) {
	return source?.type === "value" ? source.value : 0;
}

function scalarSourceEncoderDisplay(
	source: DynamicScalarSourceProjection | undefined,
) {
	if (!source) return "—";
	if (source.type === "current") return "Current";
	if (source.type === "preset") return "Preset";
	return `${Math.round(source.value * 100)}%`;
}

function scalarSourcePresetChoices(
	presets: readonly PresetObject[],
	attribute: string,
	source: DynamicScalarSourceProjection,
): NonNullable<EncoderSectionItem["presets"]> {
	const available = [...presets]
		.filter((preset) => presetContainsAttribute(preset, attribute))
		.sort((left, right) => {
			const family = presetFamilyLabel(left).localeCompare(
				presetFamilyLabel(right),
			);
			return family || left.body.number - right.body.number;
		});
	const families = new Map<string, PresetObject[]>();
	for (const preset of available) {
		const family = presetFamilyLabel(preset);
		families.set(family, [...(families.get(family) ?? []), preset]);
	}
	return {
		selectedValue:
			source.type === "current"
				? "current"
				: source.type === "preset"
					? `preset:${source.preset_id}`
					: undefined,
		groups: [
			{
				label: "Source",
				options: [
					{
						value: "current",
						label: "Current",
						description: "Use the current value for this attribute.",
					},
				],
			},
			...[...families].map(([family, items]) => ({
				label: family,
				options: items.map((preset) => ({
					value: `preset:${preset.id}`,
					label: preset.body.name,
					description: `${family} ${preset.body.number}`,
				})),
			})),
		],
	};
}

function presetContainsAttribute(preset: PresetObject, attribute: string) {
	const fixtureValues = Object.values(preset.body.values);
	const groupValues = Object.values(preset.body.group_values ?? {});
	return [...fixtureValues, ...groupValues].some((values) =>
		Object.hasOwn(values, attribute),
	);
}

function presetFamilyLabel(preset: PresetObject) {
	const family = preset.body.family;
	return !family || family === "All" ? "Mixed" : family;
}

function scalarSourceFromPresetChoice(
	value: string,
	attribute: string,
): DynamicScalarSourceProjection {
	if (value === "current") return sourceCurrent;
	if (value.startsWith("preset:"))
		return {
			type: "preset",
			preset_id: value.slice("preset:".length),
			attribute,
			last_valid_by_target: [],
		};
	return sourceCurrent;
}

const curveFunctionOptions: Array<{
	value: DynamicPeriodicFunctionProjection | "random";
	label: string;
	description: string;
}> = [
	{
		value: "sinus",
		label: "Sinus",
		description:
			"Smooth wave · Top and Bottom or Middle and Amplitude, Curve width, Speed.",
	},
	{
		value: "cosinus",
		label: "Cosinus",
		description:
			"Smooth wave starting at maximum · Top and Bottom or Middle and Amplitude, Curve width, Speed.",
	},
	{
		value: "linear_up",
		label: "Linear +",
		description:
			"Steady rise · Top and Bottom or Middle and Amplitude, Curve width, Speed.",
	},
	{
		value: "linear_down",
		label: "Linear −",
		description:
			"Steady fall · Top and Bottom or Middle and Amplitude, Curve width, Speed.",
	},
	{
		value: "pwm",
		label: "PWM",
		description:
			"Shaped pulse · Top and Bottom or Middle and Amplitude, Attack, On, Decay, Off.",
	},
	{
		value: "random",
		label: "Random",
		description:
			"Seeded gate values and timing · configuration comes from the linked Random group.",
	},
];

function curveFunctionSelectionGroups() {
	return [
		{
			label: "Periodic functions",
			options: curveFunctionOptions.slice(0, 5).map((option) => ({
				...option,
				icon: <CurveFunctionIcon functionName={option.value} />,
			})),
		},
		{
			label: "Random function",
			options: curveFunctionOptions.slice(5).map((option) => ({
				...option,
				icon: <CurveFunctionIcon functionName={option.value} />,
			})),
		},
	];
}

function CurveFunctionIcon({
	functionName,
}: {
	functionName: DynamicPeriodicFunctionProjection | "random";
}) {
	const path =
		functionName === "sinus"
			? "M1 12C4 3 8 3 12 12s8 9 11 0"
			: functionName === "cosinus"
				? "M1 4c5 0 6 16 11 16S18 4 23 4"
				: functionName === "linear_up"
					? "M2 21 22 3"
					: functionName === "linear_down"
						? "M2 3 22 21"
						: functionName === "pwm"
							? "M2 20V4h10v16h10"
							: "M2 16 6 7l4 10 4-12 4 11 4-7";
	return (
		<svg
			className="dynamic-function-icon"
			viewBox="0 0 24 24"
			aria-hidden="true"
		>
			<path d={path} />
		</svg>
	);
}
const interpolations = [
	"linear",
	"ease_in",
	"ease_out",
	"ease_in_out",
	"hold",
	"drop",
] as const;

function phaseWithExplicitRange(
	phase: DynamicDefinitionProjection["phase"],
	values: number[],
): DynamicDefinitionProjection["phase"] {
	if (values.length < 2 || values.some((value) => !Number.isFinite(value)))
		return phase;
	const offset = values[0];
	const anchors = values.map((value) => value - offset);
	return {
		...phase,
		offset_degrees: offset,
		span_degrees: anchors.at(-1) ?? phase.span_degrees,
		anchors_degrees: anchors,
	};
}

function phaseDirectionSlot(
	phase: DynamicDefinitionProjection["phase"],
	applyPhase: (
		phase: DynamicDefinitionProjection["phase"],
		mutationGroup?: string,
	) => Promise<void>,
): DynamicEncoderSlot {
	const ordering = phase.ordering;
	const direction = ordering.type === "grid_linear";
	const spatial = isSpatialOrdering(ordering);
	const value = direction
		? ordering.angle_degrees
		: spatial
			? ordering.center_x
			: 0;
	return {
		id: "direction-center",
		label: direction
			? "Direction"
			: spatial
				? "Center X"
				: "Direction / center",
		display: direction ? `${value}°` : spatial ? String(value) : "Unavailable",
		value,
		minimum: direction ? 0 : -10_000,
		maximum: direction ? 359 : 10_000,
		inputScale: 1,
		fineStep: direction ? 5 : 0.1,
		coarseStep: direction ? 45 : 1,
		disabled: !direction && !spatial,
		apply: (next, group) =>
			applyPhase(
				{
					...phase,
					ordering: direction
						? { ...ordering, angle_degrees: normalizeDegrees(next) }
						: spatial
							? { ...ordering, center_x: next }
							: ordering,
				},
				group,
			),
	};
}

function speedEncoderSlots(
	dynamic: DynamicDefinitionProjection,
	onMutate: (
		intent: DynamicUpdateIntent,
		mutationGroup?: string,
	) => Promise<void>,
): DynamicEncoderSlot[] {
	const speedGroups = ["A", "B", "C", "D", "E"] as const;
	const speed = dynamic.speed;
	const speedSource =
		speed.type === "fixed" ? 0 : speedGroups.indexOf(speed.group) + 1;
	const activationModes = [
		"start_now",
		"join_sync_now",
		"next_boundary",
	] as const;
	const activationIndex = activationModes.indexOf(dynamic.default_activation);
	const fixed = speed.type === "fixed";
	const cycleValue =
		speed.type === "fixed"
			? speed.duration_millis / 1000
			: rationalValue(speed.beats_per_cycle);
	const cycleDisplay =
		speed.type === "fixed"
			? `${(speed.duration_millis / 1000).toFixed(2)} s`
			: `${speed.beats_per_cycle.numerator}/${speed.beats_per_cycle.denominator}`;
	return [
		{
			id: "speed-source",
			label: "Speed source",
			display: speed.type === "fixed" ? "Fixed" : `Group ${speed.group}`,
			value: speedSource,
			minimum: 0,
			maximum: speedGroups.length,
			inputScale: 1,
			fineStep: 1,
			coarseStep: 1,
			choices: encoderChoices("Speed source", speedSource, [
				{ label: "Fixed" },
				...speedGroups.map((group) => ({ label: `Group ${group}` })),
			]),
			apply: (value, group) =>
				onMutate(
					{
						type: "set_speed",
						speed:
							wrappedIndex(value, speedGroups.length + 1) === 0
								? { type: "fixed", duration_millis: 4000 }
								: {
										type: "speed_group",
										group:
											speedGroups[
												wrappedIndex(value, speedGroups.length + 1) - 1
											],
										beats_per_cycle: { numerator: 4, denominator: 1 },
									},
					},
					group,
				),
		},
		{
			id: fixed ? "duration" : "beats-cycle",
			label: fixed ? "Duration" : "Beats / cycle",
			display: cycleDisplay,
			value: cycleValue,
			minimum: fixed ? 0.001 : 0.0625,
			maximum: fixed ? 3600 : 64,
			inputScale: 1,
			fineStep: fixed ? 0.1 : 0.25,
			coarseStep: fixed ? 1 : 1,
			apply: (value, group) =>
				onMutate(
					{
						type: "set_speed",
						speed:
							speed.type === "fixed"
								? {
										type: "fixed",
										duration_millis: Math.max(1, Math.round(value * 1000)),
									}
								: {
										...speed,
										beats_per_cycle: rationalFromNumber(value),
									},
					},
					group,
				),
		},
		{
			id: "overall-speed",
			label: "Overall speed",
			display: `${dynamic.overall_speed_multiplier.numerator}/${dynamic.overall_speed_multiplier.denominator}`,
			value: rationalValue(dynamic.overall_speed_multiplier),
			minimum: 0.0625,
			maximum: 16,
			inputScale: 1,
			fineStep: 0.0625,
			coarseStep: 0.5,
			apply: (value, group) =>
				onMutate(
					{
						type: "set_overall_speed_multiplier",
						multiplier: rationalFromNumber(value),
					},
					group,
				),
		},
		{
			id: "activation",
			label: "Activation",
			display: dynamic.default_activation.replaceAll("_", " "),
			value: activationIndex,
			minimum: 0,
			maximum: activationModes.length - 1,
			inputScale: 1,
			fineStep: 1,
			coarseStep: 1,
			choices: encoderChoices("Activation", activationIndex, [
				{ label: "Start now" },
				{
					label: "Join sync now",
					disabled: fixed,
				},
				{
					label: "Next boundary",
					disabled: fixed,
				},
			]),
			apply: (value, group) =>
				onMutate(
					{
						type: "set_activation",
						activation:
							activationModes[
								fixed ? 0 : wrappedIndex(value, activationModes.length)
							],
					},
					group,
				),
		},
		{
			id: "boundary",
			label: "Boundary",
			display:
				dynamic.default_activation === "next_boundary"
					? dynamic.activation_boundary
					: "Unavailable",
			value: dynamic.activation_boundary === "bar" ? 1 : 0,
			minimum: 0,
			maximum: 1,
			inputScale: 1,
			fineStep: 1,
			coarseStep: 1,
			disabled: dynamic.default_activation !== "next_boundary",
			choices: encoderChoices(
				"Boundary",
				dynamic.activation_boundary === "bar" ? 1 : 0,
				[
					{ label: "Next beat" },
					{ label: "Next bar", description: "Four beats" },
				],
			),
			apply: (value, group) =>
				onMutate(
					{
						type: "set_activation_boundary",
						boundary: value >= 0.5 ? "bar" : "beat",
					},
					group,
				),
		},
		{
			id: "run-mode",
			label: "Run mode",
			display: dynamic.run_mode === "one_shot" ? "One-shot" : "Loop",
			value: dynamic.run_mode === "one_shot" ? 1 : 0,
			minimum: 0,
			maximum: 1,
			inputScale: 1,
			fineStep: 1,
			coarseStep: 1,
			choices: encoderChoices(
				"Run mode",
				dynamic.run_mode === "one_shot" ? 1 : 0,
				[{ label: "Loop" }, { label: "One-shot" }],
			),
			apply: (value, group) =>
				onMutate(
					{
						type: "set_run_mode",
						run_mode: value >= 0.5 ? "one_shot" : "loop",
					},
					group,
				),
		},
	];
}

function LaneAttributeModal({
	id,
	title,
	details,
	currentAttribute,
	attributes,
	busy = false,
	onClose,
	onChoose,
}: {
	id: string;
	title: string;
	details: string;
	currentAttribute?: string;
	attributes: readonly { id: string; label: string; family: string }[];
	busy?: boolean;
	onClose(): void;
	onChoose(attribute: string): void;
}) {
	const groups = attributes.reduce<
		Array<{
			family: string;
			attributes: Array<{ id: string; label: string; family: string }>;
		}>
	>((grouped, attribute) => {
		const family = attribute.family || "Other";
		const group = grouped.find((candidate) => candidate.family === family);
		if (group) group.attributes.push(attribute);
		else grouped.push({ family, attributes: [attribute] });
		return grouped;
	}, []);
	return (
		<ModalFrame
			id={id}
			ariaLabel={title}
			title={title}
			details={details}
			dialogClassName="dynamic-attribute-choice-modal"
			onClose={onClose}
		>
			<div className="dynamic-attribute-choice-scroll">
				<div className="ui-grouped-selection-groups dynamic-attribute-choice-groups">
					{groups.map((group) => (
						<section key={group.family}>
							<h3>{group.family}</h3>
							<div className="ui-grouped-selection-options">
								{group.attributes.map((attribute) => {
									const selected = attribute.id === currentAttribute;
									return (
										<Button
											key={attribute.id}
											active={selected}
											aria-pressed={selected}
											disabled={busy}
											contentAlign="left"
											onClick={() =>
												selected ? onClose() : onChoose(attribute.id)
											}
										>
											<span className="ui-grouped-selection-option has-no-icon">
												<span className="ui-grouped-selection-copy">
													<b>{attribute.label}</b>
												</span>
											</span>
										</Button>
									);
								})}
							</div>
						</section>
					))}
				</div>
				{groups.length === 0 && (
					<p className="dynamic-attribute-choice-empty" role="alert">
						No continuous scalar attributes are available.
					</p>
				)}
			</div>
		</ModalFrame>
	);
}

export function createDefaultDynamicDefinition(
	poolNumber: number,
	attribute: string,
	ids: { definition?: string; lane?: string } = {},
): DynamicDefinitionProjection {
	return {
		id: ids.definition ?? crypto.randomUUID(),
		pool_number: poolNumber,
		revision: 0,
		name: `Dynamic ${poolNumber}`,
		color: "#4edcff",
		icon: "∿",
		target_binding: { type: "targetless" },
		lanes: [createDefaultDynamicLane(attribute, ids.lane)],
		random_groups: [],
		phase_mode: "uniform",
		phase: {
			ordering: { type: "selection" },
			offset_degrees: 0,
			span_degrees: 360,
			block_size: 1,
			repeats: 1,
			wings: false,
			anchors_degrees: [],
		},
		speed: { type: "fixed", duration_millis: 4000 },
		overall_speed_multiplier: { numerator: 1, denominator: 1 },
		run_mode: "loop",
		default_activation: "start_now",
		activation_boundary: "beat",
	};
}

export function createDefaultDynamicLane(
	attribute: string,
	id: string = crypto.randomUUID(),
): DynamicLaneProjection {
	return {
		id,
		attribute,
		mode: "max_min",
		keyframes: {
			points: [
				{
					position: 0,
					source: sourceZero,
					interpolation: "ease_in_out",
				},
				{
					position: 0.5,
					source: sourceFull,
					interpolation: "ease_in_out",
				},
			],
			size: 1,
		},
		max_min: {
			minimum: sourceZero,
			maximum: sourceFull,
			function: "sinus",
			size: 1,
			pwm: defaultPwm(),
		},
		middle_amplitude: {
			middle: { type: "value", value: 0.5 },
			amplitude: 0.5,
			function: "sinus",
			size: 1,
			pwm: defaultPwm(),
		},
		speed_multiplier: { numerator: 1, denominator: 1 },
		width: 1,
		random_group_id: null,
		phase: null,
	};
}

function defaultPwm() {
	return {
		attack: 0,
		on: 0.5,
		decay: 0,
		off: 0.5,
		attack_interpolation: "linear" as const,
		decay_interpolation: "linear" as const,
	};
}

function defaultRandomGroup(): DynamicRandomGroupProjection {
	return {
		id: crypto.randomUUID(),
		seed: crypto.getRandomValues(new Uint32Array(1))[0] ?? 0,
		low: sourceZero,
		high: sourceFull,
		decision_interval_millis: 250,
		start_probability: 0.25,
		mean_duration_millis: 500,
		duration_spread_millis: 100,
		attack_ratio: 0.1,
		decay_ratio: 0.1,
	};
}

function largestKeyframeGapMidpoint(
	points: readonly { position: number }[],
): number {
	const positions = [...points.map((point) => point.position), 1].sort(
		(left, right) => left - right,
	);
	let midpoint = 0.5;
	let largest = -1;
	for (let index = 1; index < positions.length; index += 1) {
		const gap = positions[index] - positions[index - 1];
		if (gap > largest) {
			largest = gap;
			midpoint = positions[index - 1] + gap / 2;
		}
	}
	return clamp(midpoint, 0.01, 0.99);
}

function greatestCommonDivisor(left: number, right: number) {
	let a = Math.abs(Math.round(left));
	let b = Math.abs(Math.round(right));
	while (b !== 0) [a, b] = [b, a % b];
	return Math.max(1, a);
}

function targetSummary(definition: DynamicDefinitionProjection) {
	switch (definition.target_binding.type) {
		case "live_group":
			return `Live Group · ${definition.target_binding.group_id}`;
		case "frozen_targets":
			return `${definition.target_binding.targets.length} frozen targets`;
		case "targetless":
			return "Targetless · resolves at start";
	}
}

function definitionStatus(
	runtime: DynamicRuntimeSnapshotProjection | null,
	dynamicId: string,
) {
	return (
		runtime?.definitions.find((status) => status.dynamic_id === dynamicId) ??
		null
	);
}

function coverageSummary(status: DynamicDefinitionStatusProjection) {
	const addresses = status.target_count * status.lane_count;
	return `${status.compatible_target_count}/${status.target_count} compatible targets · ${status.supported_address_count}/${addresses} lane addresses · ${status.unpatched_target_count} unpatched · ${status.missing_target_count} missing`;
}

function modeLabel(mode: DynamicLaneModeProjection) {
	switch (mode) {
		case "keyframes":
			return "Keyframes";
		case "max_min":
			return "Max / min";
		case "middle_amplitude":
			return "Middle / amplitude";
		case "random":
			return "Random";
	}
}

function lanePreview(
	lane: DynamicLaneProjection,
	lanes: readonly DynamicLaneProjection[],
) {
	const slowest = Math.min(
		...lanes.map((candidate) =>
			Math.max(0.0001, rationalValue(candidate.speed_multiplier)),
		),
	);
	const repetitions = clamp(
		rationalValue(lane.speed_multiplier) / slowest,
		1,
		16,
	);
	if (lane.mode === "keyframes") {
		const cyclePath = (cycle: number) => {
			const points = [
				...lane.keyframes.points,
				{
					position: 1,
					source: lane.keyframes.points[0]?.source ?? sourceZero,
				},
			]
				.map((point) => ({
					...point,
					timelinePosition: (cycle + point.position) / repetitions,
				}))
				.filter((point) => point.timelinePosition <= 1.0001);
			return points
				.map(
					(point, index) =>
						`${index === 0 ? "M" : "L"}${Math.round(8 + point.timelinePosition * 984)} ${keyframeY(point.source)}`,
				)
				.join(" ");
		};
		return {
			repetitions,
			primaryPath: cyclePath(0),
			repeatedPath: Array.from(
				{ length: Math.max(0, Math.ceil(repetitions) - 1) },
				(_, index) => cyclePath(index + 1),
			)
				.filter(Boolean)
				.join(" "),
		};
	}
	const functionName =
		lane.mode === "middle_amplitude"
			? lane.middle_amplitude.function
			: lane.max_min.function;
	const path = (start: number, end: number) =>
		Array.from({ length: 121 }, (_, index) => {
			const progress = start + ((end - start) * index) / 120;
			const intervalPhase = (progress * repetitions) % 1;
			const width = functionName === "pwm" ? 1 : clamp(lane.width, 0.05, 1);
			const phase = clamp((intervalPhase - (1 - width) / 2) / width, 0, 1);
			const shape = periodicPreviewValue(
				functionName,
				phase,
				lane.mode === "middle_amplitude"
					? lane.middle_amplitude.pwm
					: lane.max_min.pwm,
			);
			const minimum =
				lane.mode === "middle_amplitude"
					? scalarSourceCurveValue(lane.middle_amplitude.middle) -
						lane.middle_amplitude.amplitude
					: scalarSourceCurveValue(lane.max_min.minimum);
			const maximum =
				lane.mode === "middle_amplitude"
					? scalarSourceCurveValue(lane.middle_amplitude.middle) +
						lane.middle_amplitude.amplitude
					: scalarSourceCurveValue(lane.max_min.maximum);
			const value = clamp(minimum + (maximum - minimum) * shape, 0, 1);
			return `${index === 0 ? "M" : "L"}${Math.round(progress * 1000)} ${Math.round(190 - value * 180)}`;
		}).join(" ");
	const firstEnd = 1 / repetitions;
	return {
		repetitions,
		primaryPath: path(0, firstEnd),
		repeatedPath: repetitions > 1 ? path(firstEnd, 1) : "",
	};
}

function laneSpeedLabel(lane: DynamicLaneProjection) {
	const value = rationalValue(lane.speed_multiplier);
	if (Math.abs(value - 1) < 0.0001) return "";
	return ` · ${lane.speed_multiplier.numerator}/${lane.speed_multiplier.denominator} speed`;
}

function keyframePreviewPercent(position: number, repetitions: number) {
	return 0.8 + (clamp(position, 0, 1) * 98.4) / repetitions;
}

function keyframePreviewTop(source: DynamicScalarSourceProjection | undefined) {
	return 9 + (1 - scalarSourceCurveValue(source)) * 70;
}

function scalarSourceCurveValue(
	source: DynamicScalarSourceProjection | undefined,
) {
	return source?.type === "value" ? source.value : 0.5;
}

function keyframeName(index: number) {
	return String.fromCharCode(65 + (index % 26));
}

function keyframeY(source: DynamicScalarSourceProjection | undefined) {
	return Math.round(190 - scalarSourceCurveValue(source) * 180);
}

function rationalValue(value: { numerator: number; denominator: number }) {
	return value.numerator / value.denominator;
}

function rationalFromNumber(value: number) {
	const denominator = 10_000;
	const numerator = Math.max(1, Math.round(value * denominator));
	const divisor = greatestCommonDivisor(numerator, denominator);
	return {
		numerator: numerator / divisor,
		denominator: denominator / divisor,
	};
}

function wrappedIndex(value: number, length: number) {
	const rounded = Math.round(value);
	return ((rounded % length) + length) % length;
}

function laneShapeLabel(lane: DynamicLaneProjection) {
	if (lane.mode === "keyframes") return "Keyframes";
	if (lane.mode === "random") return "Random";
	const value =
		lane.mode === "middle_amplitude"
			? lane.middle_amplitude.function
			: lane.max_min.function;
	switch (value) {
		case "sinus":
			return "Sinus";
		case "cosinus":
			return "Cosinus";
		case "linear_up":
			return "Linear +";
		case "linear_down":
			return "Linear −";
		case "pwm":
			return "PWM";
	}
}

function primaryInterpolationIndex(lane: DynamicLaneProjection) {
	return interpolations.indexOf(
		lane.keyframes.points[0]?.interpolation ?? "ease_in_out",
	);
}

function primaryInterpolationLabel(lane: DynamicLaneProjection) {
	if (lane.mode !== "keyframes") return "Unavailable";
	const value = lane.keyframes.points[0]?.interpolation ?? "ease_in_out";
	switch (value) {
		case "linear":
			return "Linear";
		case "ease_in":
			return "Ease in";
		case "ease_out":
			return "Ease out";
		case "ease_in_out":
			return "Ease in + out";
		case "hold":
			return "Hold";
		case "drop":
			return "Drop";
	}
}

function setPrimaryInterpolation(
	lane: DynamicLaneProjection,
	value: number,
): DynamicLaneProjection {
	if (lane.mode !== "keyframes") return lane;
	const interpolation =
		interpolations[wrappedIndex(value, interpolations.length)];
	return {
		...lane,
		keyframes: {
			...lane.keyframes,
			points: lane.keyframes.points.map((point, index) =>
				index === 0 ? { ...point, interpolation } : point,
			),
		},
	};
}

function normalizeDegrees(value: number) {
	return ((value % 360) + 360) % 360;
}

function orderingFor(
	type: string,
	current?: DynamicPhaseOrderingProjection,
): DynamicPhaseOrderingProjection {
	switch (type) {
		case "grid_linear":
			return {
				type,
				angle_degrees:
					current?.type === "grid_linear" ? current.angle_degrees : 90,
			};
		case "radial_out":
		case "radial_in":
		case "axial":
			return {
				type,
				center_x: current && isSpatialOrdering(current) ? current.center_x : 0,
				center_z: current && isSpatialOrdering(current) ? current.center_z : 0,
			};
		case "random_each_loop":
			return {
				type,
				seed: current?.type === "random_each_loop" ? current.seed : Date.now(),
			};
		default:
			return { type: "selection" };
	}
}

function isSpatialOrdering(
	ordering: DynamicPhaseOrderingProjection,
): ordering is Extract<
	DynamicPhaseOrderingProjection,
	{ type: "radial_out" | "radial_in" | "axial" }
> {
	return (
		ordering.type === "radial_out" ||
		ordering.type === "radial_in" ||
		ordering.type === "axial"
	);
}

function clamp(value: number, minimum: number, maximum: number) {
	return Math.max(minimum, Math.min(maximum, value));
}

function runningCount(
	runtime: DynamicRuntimeSnapshotProjection | null,
	dynamicId: string,
) {
	return (
		runtime?.instances.filter((instance) => instance.dynamic_id === dynamicId)
			.length ?? 0
	);
}
