import {
	Button,
	ColorPickerField,
	FormLayout,
	IconPickerField,
	SelectField,
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
import { WindowHeader, WindowScrollArea } from "@tosklight/ui/window-kit";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ApiRequestError } from "../api/ApiRequestError";
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
} from "../api/generated/light-wire";
import { useCommandLineSurface } from "../components/control/commandLine/useCommandLineSurface";
import {
	useActiveShowId,
	useAttributeRegistry,
	useHardwareConnected,
} from "../features/deskSnapshot/DeskSnapshotState";
import { useDynamicsActions } from "../features/dynamics/DynamicsActionsContext";
import { useDynamicEditorSession } from "../features/dynamics/DynamicEditorSessionContext";
import {
	useProgrammingCommandLineActions,
	useProgrammingDeleteCommandActive,
} from "../features/programmingInteraction/ProgrammingInteractionView";
import type { ShowObject } from "../features/showObjects/contracts";
import {
	useDynamics,
	usePresets,
} from "../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../features/showObjects/ShowObjectsView";
import { useApp } from "../state/AppContext";
import type { WindowProps } from "./windowTypes";
import "./DynamicsWindow.css";

type DynamicObject = ShowObject<"dynamic">;
type PresetObject = ShowObject<"preset">;
export type DynamicEditorView = "curves" | "phase" | "speed";

const sourceCurrent: DynamicScalarSourceProjection = { type: "current" };
const sourceZero: DynamicScalarSourceProjection = { type: "value", value: 0 };
const sourceFull: DynamicScalarSourceProjection = { type: "value", value: 1 };

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
	const command = useCommandLineSurface({ selection: true, enabled: active });
	const commandActions = useProgrammingCommandLineActions();
	const deleteArmed = useProgrammingDeleteCommandActive(active);
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
				primaryLaneId: definition.lanes[0]?.id ?? null,
			});
		});
	const mutate = (
		dynamic: DynamicObject,
		intent: DynamicUpdateIntent,
		mutationGroup?: string,
	) =>
		run(async () => {
			if (!showId) throw new Error("No active show");
			try {
				await api.showObjects.updateDynamic(
					showId,
					dynamic.id,
					dynamic.revision,
					intent,
					mutationGroup,
				);
			} catch (cause) {
				if (!(cause instanceof ApiRequestError) || cause.status !== 409)
					throw cause;
				const current =
					await api.showObjects.object<DynamicDefinitionProjection>(
						showId,
						"dynamic",
						dynamic.id,
					);
				await api.showObjects.updateDynamic(
					showId,
					dynamic.id,
					current.revision,
					intent,
					mutationGroup,
				);
			}
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
				selection={command.selected}
				selectedGroupId={command.selectedGroupId}
				onBack={() => {
					closeEditor(selected.id);
					setSelectedId(null);
				}}
				onMutate={mutate}
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
											primaryLaneId: dynamic.body.lanes[0]?.id ?? null,
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
										primaryLaneId: dynamic.body.lanes[0]?.id ?? null,
									});
									setSelectedId(dynamic.id);
								}}
								onPressHold={() => {
									if (!dynamic) return;
									openEditor({
										dynamicId: dynamic.id,
										task: "curves",
										primaryLaneId: dynamic.body.lanes[0]?.id ?? null,
									});
									setSelectedId(dynamic.id);
								}}
							/>
						);
					}}
				/>
			</WindowScrollArea>
			{chooserSlot !== null && (
				<LaneChooser
					slot={chooserSlot}
					attributes={attributes}
					busy={busy}
					onCancel={() => setChooserSlot(null)}
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
	selection: readonly string[];
	selectedGroupId: string | null;
	view?: DynamicEditorView;
	onBack(): void;
	onMutate(
		dynamic: DynamicObject,
		intent: DynamicUpdateIntent,
		mutationGroup?: string,
	): Promise<void>;
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
	presets,
	runtime,
	selection,
	selectedGroupId,
	view: controlledView,
	onBack,
	onMutate,
	onDelete,
	onMove,
	onCopy,
}: DynamicEditorProps) {
	const { state: appState, dispatch } = useApp();
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
	const [destinationPoolNumber, setDestinationPoolNumber] = useState(
		dynamic.body.pool_number,
	);
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
		updateEditor({ primaryLaneId: id });
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
	useEffect(() => {
		openEditor({
			dynamicId: dynamic.id,
			task: view,
			primaryLaneId: primaryLane,
		});
	}, [dynamic.id, openEditor, primaryLane, view]);

	return (
		<section
			className={`dynamics-window dynamics-editor ${compact ? "compact" : ""}`}
			aria-busy={busy}
		>
			<header className="window-toolbar dynamics-toolbar">
				<Button onClick={onBack}>← Back to Pool</Button>
				<h1>
					{dynamic.body.name} <small>Dynamic {dynamic.body.pool_number}</small>
				</h1>
			</header>
			{error && (
				<p className="dynamics-error" role="alert">
					{error}
				</p>
			)}
			<div className="dynamics-editor-body">
				<aside className="dynamic-lane-rail">
					<section className="dynamic-object-metadata">
						<h3>Dynamic settings</h3>
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
					</section>
					<header>
						<b>Lanes</b>
						<small>{selectedLanes.size} selected</small>
					</header>
					{dynamic.body.lanes.map((candidate, index) => (
						<div key={candidate.id} className="dynamic-lane-row">
							<button
								type="button"
								className={`${candidate.id === primaryLane ? "primary" : ""} ${selectedLanes.has(candidate.id) ? "selected" : ""}`}
								onClick={(event) =>
									selectLane(
										candidate.id,
										event.shiftKey || appState.shiftArmed,
									)
								}
							>
								<span>{index + 1}</span>
								<strong>
									{attributes.find((item) => item.id === candidate.attribute)
										?.label ?? candidate.attribute}
								</strong>
								<small>{modeLabel(candidate.mode)}</small>
							</button>
							<span className="dynamic-lane-order">
								<button
									type="button"
									disabled={index === 0}
									aria-label={`Move ${candidate.attribute} lane up`}
									onClick={(event) => {
										event.stopPropagation();
										void onMutate(dynamic, {
											type: "move_lane",
											lane_id: candidate.id,
											index: index - 1,
										});
									}}
								>
									↑
								</button>
								<button
									type="button"
									disabled={index === dynamic.body.lanes.length - 1}
									aria-label={`Move ${candidate.attribute} lane down`}
									onClick={(event) => {
										event.stopPropagation();
										void onMutate(dynamic, {
											type: "move_lane",
											lane_id: candidate.id,
											index: index + 1,
										});
									}}
								>
									↓
								</button>
							</span>
						</div>
					))}
					<div className="dynamic-lane-actions">
						<Button
							onClick={() =>
								onMutate(dynamic, {
									type: "add_lane",
									lane: createDefaultDynamicLane(
										attributes[0]?.id ?? "intensity",
									),
									index: null,
								})
							}
						>
							+ Add Lane
						</Button>
						<Button
							disabled={!lane}
							onClick={() =>
								lane &&
								onMutate(dynamic, {
									type: "add_lane",
									lane: { ...lane, id: crypto.randomUUID() },
									index: null,
								})
							}
						>
							Duplicate
						</Button>
						<Button
							disabled={dynamic.body.lanes.length <= 1 || !lane}
							onClick={() =>
								lane &&
								onMutate(dynamic, {
									type: "delete_lane",
									lane_id: lane.id,
								})
							}
						>
							Delete
						</Button>
					</div>
				</aside>
				<main className="dynamic-workspace">
					{view === "curves" && (
						<div className="dynamic-lane-stack">
							{dynamic.body.lanes.map((candidate) => (
								<section
									key={candidate.id}
									className={`dynamic-lane-workspace-card ${candidate.id === primaryLane ? "primary" : ""}`}
									onPointerDown={() => selectLane(candidate.id, false)}
								>
									<CurvesView
										lane={candidate}
										attributes={attributes}
										presets={presets}
										randomGroups={dynamic.body.random_groups}
										onReplace={replaceLane}
										onAddRandomGroup={(group) =>
											onMutate(dynamic, { type: "add_random_group", group })
										}
										onReplaceRandomGroup={(group) =>
											onMutate(dynamic, {
												type: "replace_random_group",
												group_id: group.id,
												group,
											})
										}
									/>
								</section>
							))}
						</div>
					)}
					{view === "phase" && (
						<PhaseView dynamic={dynamic} onMutate={onMutate} />
					)}
					{view === "speed" && (
						<SpeedView
							dynamic={dynamic}
							runtime={runtime}
							onMutate={onMutate}
						/>
					)}
				</main>
			</div>
			<footer className="dynamic-editor-footer">
				<div>
					<strong>{targetSummary(dynamic.body)}</strong>
					<small>
						{status ? `${coverageSummary(status)} · ` : ""}
						Revision {dynamic.revision} · edits apply immediately
					</small>
					{status?.warning && (
						<small className="dynamics-warning">{status.warning}</small>
					)}
				</div>
				<Button
					disabled={running || selection.length === 0}
					title={
						running
							? "Turn every running instance Off before changing targets"
							: selection.length === 0
								? "Select a Group or fixtures first"
								: undefined
					}
					onClick={() =>
						onMutate(dynamic, {
							type: "set_target_binding",
							target_binding: selectedGroupId
								? { type: "live_group", group_id: selectedGroupId }
								: { type: "frozen_targets", targets: [...selection] },
						})
					}
				>
					Take Selection
				</Button>
				<Button
					disabled={
						running || dynamic.body.target_binding.type === "targetless"
					}
					title={
						running
							? "Turn every running instance Off before changing targets"
							: undefined
					}
					onClick={() =>
						onMutate(dynamic, {
							type: "set_target_binding",
							target_binding: { type: "targetless" },
						})
					}
				>
					Clear Selection
				</Button>
				<label className="dynamic-pool-destination">
					Pool
					<input
						type="number"
						min={1}
						max={9_999}
						value={destinationPoolNumber}
						onChange={(event) =>
							setDestinationPoolNumber(
								Math.max(1, Math.min(9_999, Number(event.target.value) || 1)),
							)
						}
					/>
				</label>
				<Button
					disabled={destinationPoolNumber === dynamic.body.pool_number}
					onClick={() => onMove(destinationPoolNumber)}
				>
					Move
				</Button>
				<Button
					disabled={destinationPoolNumber === dynamic.body.pool_number}
					onClick={() => onCopy(destinationPoolNumber)}
				>
					Copy
				</Button>
				<Button className="danger" onClick={onDelete}>
					Delete Dynamic
				</Button>
			</footer>
		</section>
	);
}

function CurvesView({
	lane,
	attributes,
	presets,
	randomGroups,
	onReplace,
	onAddRandomGroup,
	onReplaceRandomGroup,
}: {
	lane: DynamicLaneProjection;
	attributes: readonly { id: string; label: string; family: string }[];
	presets: readonly PresetObject[];
	randomGroups: readonly DynamicRandomGroupProjection[];
	onReplace(next: DynamicLaneProjection): Promise<void>;
	onAddRandomGroup(group: DynamicRandomGroupProjection): Promise<void>;
	onReplaceRandomGroup(group: DynamicRandomGroupProjection): Promise<void>;
}) {
	const setMode = async (mode: DynamicLaneModeProjection) => {
		if (mode !== "random") {
			await onReplace({ ...lane, mode });
			return;
		}
		let groupId = lane.random_group_id;
		if (!groupId || !randomGroups.some((group) => group.id === groupId)) {
			const group = defaultRandomGroup();
			await onAddRandomGroup(group);
			groupId = group.id;
		}
		await onReplace({ ...lane, mode, random_group_id: groupId });
	};
	const randomGroup =
		randomGroups.find((group) => group.id === lane.random_group_id) ?? null;
	return (
		<div className="dynamic-curves-view">
			<header className="dynamic-control-row">
				<label>
					Attribute
					<select
						value={lane.attribute}
						onChange={(event) =>
							onReplace({ ...lane, attribute: event.target.value })
						}
					>
						{attributes.map((attribute) => (
							<option key={attribute.id} value={attribute.id}>
								{attribute.family} · {attribute.label}
							</option>
						))}
					</select>
				</label>
				<fieldset className="button-group" aria-label="Lane mode">
					<Button
						className={lane.mode === "keyframes" ? "active" : ""}
						onClick={() => void setMode("keyframes")}
					>
						Keyframes
					</Button>
					<Button
						className={lane.mode === "max_min" ? "active" : ""}
						onClick={() => void setMode("max_min")}
					>
						Max / min
					</Button>
					<Button
						className={lane.mode === "middle_amplitude" ? "active" : ""}
						onClick={() => void setMode("middle_amplitude")}
					>
						Middle / amplitude
					</Button>
					<Button
						className={lane.mode === "random" ? "active" : ""}
						onClick={() => void setMode("random")}
					>
						Random
					</Button>
				</fieldset>
			</header>
			<div className="dynamic-curve-canvas">
				<svg
					viewBox="0 0 1000 260"
					role="img"
					aria-label={modeLabel(lane.mode)}
				>
					<title>{modeLabel(lane.mode)}</title>
					<path
						className="grid"
						d="M0 65H1000M0 130H1000M0 195H1000M250 0V260M500 0V260M750 0V260"
					/>
					<path className="curve" d={curvePath(lane)} />
				</svg>
				<span>0%</span>
				<span>25%</span>
				<span>50%</span>
				<span>75%</span>
				<span>100%</span>
			</div>
			{lane.mode === "keyframes" ? (
				<KeyframeControls lane={lane} presets={presets} onReplace={onReplace} />
			) : lane.mode === "random" && randomGroup ? (
				<RandomControls
					lane={lane}
					group={randomGroup}
					groups={randomGroups}
					presets={presets}
					onReplaceLane={onReplace}
					onReplaceGroup={onReplaceRandomGroup}
				/>
			) : (
				<FunctionControls lane={lane} presets={presets} onReplace={onReplace} />
			)}
		</div>
	);
}

function KeyframeControls({
	lane,
	presets,
	onReplace,
}: {
	lane: DynamicLaneProjection;
	presets: readonly PresetObject[];
	onReplace(next: DynamicLaneProjection): Promise<void>;
}) {
	return (
		<section className="dynamic-source-cards">
			{lane.keyframes.points.map((point, index) => (
				<article key={`${point.position}-${index}`}>
					<header>
						<b>
							{index === 0 ? "A · Loop start / close" : `Point ${index + 1}`}
						</b>
						{index === 0 ? (
							<span>0% / 100%</span>
						) : (
							<input
								aria-label={`Point ${index + 1} position`}
								type="number"
								min={1}
								max={99}
								value={Math.round(point.position * 100)}
								onChange={(event) => {
									const position = clamp(
										Number(event.target.value) / 100,
										0.01,
										0.99,
									);
									const points = lane.keyframes.points
										.map((candidate, target) =>
											target === index ? { ...candidate, position } : candidate,
										)
										.sort((left, right) => left.position - right.position);
									void onReplace({
										...lane,
										keyframes: { ...lane.keyframes, points },
									});
								}}
							/>
						)}
					</header>
					<ScalarSourceControl
						source={point.source}
						attribute={lane.attribute}
						presets={presets}
						onChange={(source) => {
							const points = lane.keyframes.points.map((candidate, target) =>
								target === index ? { ...candidate, source } : candidate,
							);
							onReplace({
								...lane,
								keyframes: { ...lane.keyframes, points },
							});
						}}
					/>
					<select
						value={point.interpolation}
						onChange={(event) => {
							const points = lane.keyframes.points.map((candidate, target) =>
								target === index
									? {
											...candidate,
											interpolation: event.target
												.value as typeof candidate.interpolation,
										}
									: candidate,
							);
							onReplace({
								...lane,
								keyframes: { ...lane.keyframes, points },
							});
						}}
					>
						<option value="linear">Linear</option>
						<option value="ease_in">Ease in</option>
						<option value="ease_out">Ease out</option>
						<option value="ease_in_out">Ease in + out</option>
						<option value="hold">Hold</option>
						<option value="drop">Drop</option>
					</select>
					{index > 0 && lane.keyframes.points.length > 2 && (
						<Button
							onClick={() =>
								void onReplace({
									...lane,
									keyframes: {
										...lane.keyframes,
										points: lane.keyframes.points.filter(
											(_, target) => target !== index,
										),
									},
								})
							}
						>
							Remove point
						</Button>
					)}
				</article>
			))}
			<Button
				onClick={() => {
					const points = [...lane.keyframes.points];
					const position = largestKeyframeGapMidpoint(points);
					points.push({
						position,
						source: sourceCurrent,
						interpolation: "ease_in_out",
					});
					points.sort((left, right) => left.position - right.position);
					void onReplace({
						...lane,
						keyframes: { ...lane.keyframes, points },
					});
				}}
			>
				+ Add keyframe
			</Button>
		</section>
	);
}

function FunctionControls({
	lane,
	presets,
	onReplace,
}: {
	lane: DynamicLaneProjection;
	presets: readonly PresetObject[];
	onReplace(next: DynamicLaneProjection): Promise<void>;
}) {
	const config =
		lane.mode === "middle_amplitude" ? lane.middle_amplitude : lane.max_min;
	return (
		<section className="dynamic-function-controls">
			<label>
				Function
				<select
					value={config.function}
					onChange={(event) => {
						const functionName = event.target
							.value as DynamicPeriodicFunctionProjection;
						onReplace(
							lane.mode === "middle_amplitude"
								? {
										...lane,
										middle_amplitude: {
											...lane.middle_amplitude,
											function: functionName,
										},
									}
								: {
										...lane,
										max_min: { ...lane.max_min, function: functionName },
									},
						);
					}}
				>
					<option value="sinus">Sinus</option>
					<option value="cosinus">Cosinus</option>
					<option value="linear_up">Linear +</option>
					<option value="linear_down">Linear −</option>
					<option value="pwm">PWM</option>
				</select>
			</label>
			{lane.mode === "middle_amplitude" ? (
				<>
					<ScalarSourceControl
						label="Middle"
						source={lane.middle_amplitude.middle}
						attribute={lane.attribute}
						presets={presets}
						onChange={(middle) =>
							onReplace({
								...lane,
								middle_amplitude: { ...lane.middle_amplitude, middle },
							})
						}
					/>
					<NumberControl
						label="Amplitude"
						value={lane.middle_amplitude.amplitude}
						onChange={(amplitude) =>
							onReplace({
								...lane,
								middle_amplitude: {
									...lane.middle_amplitude,
									amplitude,
								},
							})
						}
					/>
				</>
			) : (
				<>
					<ScalarSourceControl
						label="Top"
						source={lane.max_min.maximum}
						attribute={lane.attribute}
						presets={presets}
						onChange={(maximum) =>
							onReplace({
								...lane,
								max_min: { ...lane.max_min, maximum },
							})
						}
					/>
					<ScalarSourceControl
						label="Bottom"
						source={lane.max_min.minimum}
						attribute={lane.attribute}
						presets={presets}
						onChange={(minimum) =>
							onReplace({
								...lane,
								max_min: { ...lane.max_min, minimum },
							})
						}
					/>
				</>
			)}
			{config.function === "pwm" && (
				<div className="dynamic-pwm-grid">
					{(["attack", "on", "decay", "off"] as const).map((field) => (
						<NumberControl
							key={field}
							label={`${field[0].toUpperCase()}${field.slice(1)}`}
							value={config.pwm[field]}
							onChange={(value) => {
								const pwm = { ...config.pwm, [field]: clamp(value, 0, 1) };
								void onReplace(
									lane.mode === "middle_amplitude"
										? {
												...lane,
												middle_amplitude: {
													...lane.middle_amplitude,
													pwm,
												},
											}
										: {
												...lane,
												max_min: { ...lane.max_min, pwm },
											},
								);
							}}
						/>
					))}
				</div>
			)}
		</section>
	);
}

function RandomControls({
	lane,
	group,
	groups,
	presets,
	onReplaceLane,
	onReplaceGroup,
}: {
	lane: DynamicLaneProjection;
	group: DynamicRandomGroupProjection;
	groups: readonly DynamicRandomGroupProjection[];
	presets: readonly PresetObject[];
	onReplaceLane(next: DynamicLaneProjection): Promise<void>;
	onReplaceGroup(next: DynamicRandomGroupProjection): Promise<void>;
}) {
	const update = (patch: Partial<DynamicRandomGroupProjection>) =>
		void onReplaceGroup({ ...group, ...patch });
	return (
		<section className="dynamic-function-controls dynamic-random-controls">
			<label>
				Random group
				<select
					value={group.id}
					onChange={(event) =>
						void onReplaceLane({
							...lane,
							random_group_id: event.target.value,
						})
					}
				>
					{groups.map((candidate, index) => (
						<option key={candidate.id} value={candidate.id}>
							Group {index + 1}
						</option>
					))}
				</select>
			</label>
			<ScalarSourceControl
				label="Low"
				source={group.low}
				attribute={lane.attribute}
				presets={presets}
				onChange={(low) => update({ low })}
			/>
			<ScalarSourceControl
				label="High"
				source={group.high}
				attribute={lane.attribute}
				presets={presets}
				onChange={(high) => update({ high })}
			/>
			<NumberControl
				label="Decision interval"
				value={group.decision_interval_millis}
				suffix="ms"
				onChange={(decision_interval_millis) =>
					update({
						decision_interval_millis: Math.max(1, decision_interval_millis),
					})
				}
			/>
			<NumberControl
				label="Start probability"
				value={group.start_probability * 100}
				suffix="%"
				onChange={(value) =>
					update({ start_probability: clamp(value / 100, 0, 1) })
				}
			/>
			<NumberControl
				label="Mean pulse"
				value={group.mean_duration_millis}
				suffix="ms"
				onChange={(mean_duration_millis) =>
					update({ mean_duration_millis: Math.max(1, mean_duration_millis) })
				}
			/>
			<NumberControl
				label="Duration spread"
				value={group.duration_spread_millis}
				suffix="ms"
				onChange={(duration_spread_millis) =>
					update({
						duration_spread_millis: Math.max(0, duration_spread_millis),
					})
				}
			/>
			<NumberControl
				label="Attack"
				value={group.attack_ratio * 100}
				suffix="%"
				onChange={(value) =>
					update({
						attack_ratio: clamp(value / 100, 0, 1 - group.decay_ratio),
					})
				}
			/>
			<NumberControl
				label="Decay"
				value={group.decay_ratio * 100}
				suffix="%"
				onChange={(value) =>
					update({
						decay_ratio: clamp(value / 100, 0, 1 - group.attack_ratio),
					})
				}
			/>
			<Button
				onClick={() =>
					update({
						seed: crypto.getRandomValues(new Uint32Array(1))[0] ?? 0,
					})
				}
			>
				Generate Seed
			</Button>
		</section>
	);
}

function PhaseView({
	dynamic,
	onMutate,
}: {
	dynamic: DynamicObject;
	onMutate(dynamic: DynamicObject, intent: DynamicUpdateIntent): void;
}) {
	const phase = dynamic.body.phase;
	const spatialOrdering = isSpatialOrdering(phase.ordering)
		? phase.ordering
		: null;
	const update = (patch: Partial<typeof phase>) =>
		onMutate(dynamic, { type: "set_phase", phase: { ...phase, ...patch } });
	return (
		<div className="dynamic-phase-view">
			<section className="phase-map">
				<div className="phase-rings" aria-hidden="true">
					{Array.from({ length: 12 }, (_, index) => (
						<i
							key={index}
							style={{
								transform: `rotate(${index * 30}deg) translateY(-7rem)`,
							}}
						/>
					))}
				</div>
				<strong>{phase.span_degrees}°</strong>
				<small>Shared target phase projection</small>
			</section>
			<section className="dynamic-settings-grid">
				<label>
					Ordering
					<select
						value={phase.ordering.type}
						onChange={(event) =>
							update({ ordering: orderingFor(event.target.value) })
						}
					>
						<option value="selection">Selection / Group order</option>
						<option value="grid_linear">Grid linear</option>
						<option value="radial_out">Radial out</option>
						<option value="radial_in">Radial in</option>
						<option value="axial">Axial / Radar</option>
						<option value="random_each_loop">Random each loop</option>
					</select>
				</label>
				{phase.ordering.type === "grid_linear" && (
					<NumberControl
						label="Direction"
						value={phase.ordering.angle_degrees}
						suffix="°"
						onChange={(angle_degrees) =>
							update({ ordering: { type: "grid_linear", angle_degrees } })
						}
					/>
				)}
				{spatialOrdering && (
					<>
						<NumberControl
							label="Center X"
							value={spatialOrdering.center_x}
							onChange={(center_x) =>
								update({
									ordering: {
										type: spatialOrdering.type,
										center_x,
										center_z: spatialOrdering.center_z,
									},
								})
							}
						/>
						<NumberControl
							label="Center Z"
							value={spatialOrdering.center_z}
							onChange={(center_z) =>
								update({
									ordering: {
										type: spatialOrdering.type,
										center_x: spatialOrdering.center_x,
										center_z,
									},
								})
							}
						/>
					</>
				)}
				<NumberControl
					label="Offset"
					value={phase.offset_degrees}
					suffix="°"
					onChange={(offset_degrees) => update({ offset_degrees })}
				/>
				<NumberControl
					label="Span"
					value={phase.span_degrees}
					suffix="°"
					onChange={(span_degrees) => update({ span_degrees })}
				/>
				<NumberControl
					label="Blocks"
					value={phase.block_size}
					onChange={(block_size) => update({ block_size })}
				/>
				<NumberControl
					label="Repeats"
					value={phase.repeats}
					onChange={(repeats) => update({ repeats })}
				/>
				<label className="dynamic-check">
					<input
						type="checkbox"
						checked={phase.wings}
						onChange={(event) => update({ wings: event.target.checked })}
					/>
					Wings
				</label>
				<label>
					Explicit phase anchors
					<input
						value={phase.anchors_degrees.join(" THRU ")}
						placeholder="Automatic"
						onChange={(event) => {
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
				</label>
				<fieldset className="button-group" aria-label="Phase span presets">
					{[180, 360, 720].map((span) => (
						<Button key={span} onClick={() => update({ span_degrees: span })}>
							{span}°
						</Button>
					))}
				</fieldset>
			</section>
		</div>
	);
}

function SpeedView({
	dynamic,
	runtime,
	onMutate,
}: {
	dynamic: DynamicObject;
	runtime: DynamicRuntimeSnapshotProjection | null;
	onMutate(dynamic: DynamicObject, intent: DynamicUpdateIntent): void;
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
	return (
		<div className="dynamic-speed-view">
			<section className="speed-source-card">
				<span>Authoritative source</span>
				<strong>
					{primaryRuntime?.speed_source ??
						(speed.type === "fixed"
							? `${(speed.duration_millis / 1000).toFixed(2)} s`
							: `Speed Group ${speed.group}`)}
				</strong>
				<small>
					{runtimeState}
					{instances.length > 0
						? ` · ${instances.length} ${instances.length === 1 ? "instance" : "instances"}`
						: ""}
				</small>
				{primaryRuntime && (
					<small>
						Effective cycle{" "}
						{primaryRuntime.effective_cycle_millis > 0
							? `${(Number(primaryRuntime.effective_cycle_millis) / 1000).toFixed(2)} s`
							: "stopped"}
						{primaryRuntime.effective_bpm != null
							? ` · ${primaryRuntime.effective_bpm.toFixed(1)} BPM`
							: ""}
						{` · ${primaryRuntime.activation_boundary} boundary`}
					</small>
				)}
				{primaryRuntime?.beat_phase != null && (
					<small>
						Transport {(primaryRuntime.beat_phase * 100).toFixed(0)}%
						{primaryRuntime.pending_until_millis != null
							? ` · boundary ${primaryRuntime.pending_until_millis}`
							: ""}
					</small>
				)}
				<small>
					Overall ×{dynamic.body.overall_speed_multiplier.numerator}/
					{dynamic.body.overall_speed_multiplier.denominator}
				</small>
				{primaryRuntime?.aliasing_warning && (
					<small className="warning">{primaryRuntime.aliasing_warning}</small>
				)}
				<small>No editor preview · output transport only</small>
			</section>
			<div className="dynamic-settings-grid">
				<div className="button-group">
					<Button
						className={speed.type === "fixed" ? "active" : ""}
						onClick={() =>
							onMutate(dynamic, {
								type: "set_speed",
								speed: { type: "fixed", duration_millis: 4000 },
							})
						}
					>
						Fixed duration
					</Button>
					<Button
						className={speed.type === "speed_group" ? "active" : ""}
						onClick={() =>
							onMutate(dynamic, {
								type: "set_speed",
								speed: {
									type: "speed_group",
									group: "A",
									beats_per_cycle: { numerator: 4, denominator: 1 },
								},
							})
						}
					>
						Speed Group
					</Button>
				</div>
				{speed.type === "fixed" ? (
					<NumberControl
						label="Complete cycle"
						value={speed.duration_millis / 1000}
						suffix="s"
						onChange={(seconds) =>
							onMutate(dynamic, {
								type: "set_speed",
								speed: {
									type: "fixed",
									duration_millis: Math.max(1, Math.round(seconds * 1000)),
								},
							})
						}
					/>
				) : (
					<>
						<label>
							Speed Group
							<select
								value={speed.group}
								onChange={(event) =>
									onMutate(dynamic, {
										type: "set_speed",
										speed: { ...speed, group: event.target.value },
									})
								}
							>
								{["A", "B", "C", "D", "E"].map((group) => (
									<option key={group}>{group}</option>
								))}
							</select>
						</label>
						<NumberControl
							label="Beats per cycle"
							value={speed.beats_per_cycle.numerator}
							onChange={(numerator) =>
								onMutate(dynamic, {
									type: "set_speed",
									speed: {
										...speed,
										beats_per_cycle: {
											numerator: Math.max(1, Math.round(numerator)),
											denominator: 1,
										},
									},
								})
							}
						/>
					</>
				)}
				<label>
					Run mode
					<select
						value={dynamic.body.run_mode}
						onChange={(event) =>
							onMutate(dynamic, {
								type: "set_run_mode",
								run_mode: event.target.value as "loop" | "one_shot",
							})
						}
					>
						<option value="loop">Loop</option>
						<option value="one_shot">One-shot</option>
					</select>
				</label>
				<label>
					Activation
					<select
						value={dynamic.body.default_activation}
						onChange={(event) =>
							onMutate(dynamic, {
								type: "set_activation",
								activation: event.target.value,
							})
						}
					>
						<option value="start_now">Start now</option>
						<option
							value="join_sync_now"
							disabled={speed.type !== "speed_group"}
						>
							Join sync now
						</option>
						<option
							value="next_boundary"
							disabled={speed.type !== "speed_group"}
						>
							Next boundary
						</option>
					</select>
				</label>
				<label>
					Boundary
					<select
						value={dynamic.body.activation_boundary}
						disabled={
							speed.type !== "speed_group" ||
							dynamic.body.default_activation !== "next_boundary"
						}
						onChange={(event) =>
							onMutate(dynamic, {
								type: "set_activation_boundary",
								boundary: event.target.value as "beat" | "bar",
							})
						}
					>
						<option value="beat">Next beat</option>
						<option value="bar">Next bar (4 beats)</option>
					</select>
				</label>
				<div className="dynamic-speed-multipliers">
					<span>Overall multiplier</span>
					{[2, 3, 4].map((factor) => (
						<div className="button-group" key={factor}>
							<Button
								onClick={() =>
									onMutate(dynamic, {
										type: "set_overall_speed_multiplier",
										multiplier: scaleRational(
											dynamic.body.overall_speed_multiplier,
											factor,
										),
									})
								}
							>
								×{factor}
							</Button>
							<Button
								onClick={() =>
									onMutate(dynamic, {
										type: "set_overall_speed_multiplier",
										multiplier: scaleRational(
											dynamic.body.overall_speed_multiplier,
											1 / factor,
										),
									})
								}
							>
								÷{factor}
							</Button>
						</div>
					))}
				</div>
			</div>
		</div>
	);
}

export function DynamicEncoderDeck({
	view,
	lane,
	dynamic,
	onLaneChange,
	onMutate,
}: {
	view: DynamicEditorView;
	lane?: DynamicLaneProjection;
	dynamic: DynamicDefinitionProjection;
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
	const slots: DynamicEncoderSlot[] =
		view === "curves"
			? [
					{
						id: "size",
						label: "Size",
						display: lane ? `${Math.round(laneConfigSize(lane) * 100)}%` : "—",
						value: lane ? laneConfigSize(lane) : 0,
						minimum: 0,
						maximum: 2,
						inputScale: 100,
						fineStep: 0.01,
						coarseStep: 0.1,
						disabled: !lane,
						apply: (value, group) =>
							onLaneChange((item) => setLaneSizeValue(item, value), group),
					},
					{
						id: "width",
						label: "Width",
						display: lane ? `${Math.round(lane.width * 100)}%` : "—",
						value: lane?.width ?? 0,
						minimum: 0,
						maximum: 1,
						inputScale: 100,
						fineStep: 0.01,
						coarseStep: 0.1,
						disabled: !lane,
						apply: (value, group) =>
							onLaneChange(
								(item) => ({
									...item,
									width: clamp(value, 0, 1),
								}),
								group,
							),
					},
					{
						id: "lane-speed",
						label: "Lane speed",
						display: lane
							? `${lane.speed_multiplier.numerator}/${lane.speed_multiplier.denominator}`
							: "—",
						value: lane ? rationalValue(lane.speed_multiplier) : 1,
						minimum: 0.0625,
						maximum: 16,
						inputScale: 1,
						fineStep: 0.0625,
						coarseStep: 0.5,
						disabled: !lane,
						apply: (value, group) =>
							onLaneChange(
								(item) => ({
									...item,
									speed_multiplier: rationalFromNumber(value),
								}),
								group,
							),
					},
					{
						id: "mode",
						label: "Mode",
						display: lane ? modeLabel(lane.mode) : "—",
						value: lane ? laneModes.indexOf(lane.mode) : 0,
						minimum: 0,
						maximum: laneModes.length - 1,
						inputScale: 1,
						fineStep: 1,
						coarseStep: 1,
						disabled: !lane,
						apply: (value, group) =>
							onLaneChange(
								(item) => ({
									...item,
									mode: laneModes[wrappedIndex(value, laneModes.length)],
								}),
								group,
							),
					},
					{
						id: "shape",
						label: "Shape",
						display: lane ? laneShapeLabel(lane) : "—",
						value: lane ? periodicFunctionIndex(lane) : 0,
						minimum: 0,
						maximum: periodicFunctions.length - 1,
						inputScale: 1,
						fineStep: 1,
						coarseStep: 1,
						disabled:
							!lane || lane.mode === "keyframes" || lane.mode === "random",
						apply: (value, group) =>
							onLaneChange(
								(item) => setLanePeriodicFunction(item, value),
								group,
							),
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
						disabled: !lane || lane.mode !== "keyframes",
						apply: (value, group) =>
							onLaneChange(
								(item) => setPrimaryInterpolation(item, value),
								group,
							),
					},
				]
			: view === "phase"
				? [
						{
							id: "offset",
							label: "Offset",
							display: `${dynamic.phase.offset_degrees}°`,
							value: dynamic.phase.offset_degrees,
							minimum: -360,
							maximum: 360,
							inputScale: 1,
							fineStep: 5,
							coarseStep: 45,
							apply: (value, group) =>
								onMutate(
									{
										type: "set_phase",
										phase: {
											...dynamic.phase,
											offset_degrees: value,
										},
									},
									group,
								),
						},
						{
							id: "span",
							label: "Span",
							display: `${dynamic.phase.span_degrees}°`,
							value: dynamic.phase.span_degrees,
							minimum: 0,
							maximum: 720,
							inputScale: 1,
							fineStep: 5,
							coarseStep: 45,
							apply: (value, group) =>
								onMutate(
									{
										type: "set_phase",
										phase: {
											...dynamic.phase,
											span_degrees: value,
										},
									},
									group,
								),
						},
						{
							id: "blocks",
							label: "Blocks",
							display: String(dynamic.phase.block_size),
							value: dynamic.phase.block_size,
							minimum: 1,
							maximum: 10_000,
							inputScale: 1,
							fineStep: 1,
							coarseStep: 5,
							apply: (value, group) =>
								onMutate(
									{
										type: "set_phase",
										phase: {
											...dynamic.phase,
											block_size: Math.max(1, Math.round(value)),
										},
									},
									group,
								),
						},
						{
							id: "repeats",
							label: "Repeats",
							display: String(dynamic.phase.repeats),
							value: dynamic.phase.repeats,
							minimum: 1,
							maximum: 10_000,
							inputScale: 1,
							fineStep: 1,
							coarseStep: 5,
							apply: (value, group) =>
								onMutate(
									{
										type: "set_phase",
										phase: {
											...dynamic.phase,
											repeats: Math.max(1, Math.round(value)),
										},
									},
									group,
								),
						},
						{
							id: "wings",
							label: "Wings",
							display: dynamic.phase.wings ? "On" : "Off",
							value: dynamic.phase.wings ? 1 : 0,
							minimum: 0,
							maximum: 1,
							inputScale: 1,
							fineStep: 1,
							coarseStep: 1,
							apply: (value, group) =>
								onMutate(
									{
										type: "set_phase",
										phase: {
											...dynamic.phase,
											wings: value >= 0.5,
										},
									},
									group,
								),
						},
						phaseDirectionSlot(dynamic, onMutate),
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
				model={{
					id: `dynamics-${view}`,
					label: `${view === "curves" ? "Curves" : view === "phase" ? "Phase Spread" : "Speed"} encoders`,
					description: "Turn fine · press-turn coarse · center Set Value",
					encoders: items,
				}}
				surface={hardwareConnected ? "hardware" : "touch"}
				callbacks={{
					onRelativeChange: applyRelative,
					onAbsoluteChange: applyAbsolute,
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
	apply(value: number, mutationGroup: string): Promise<void>;
}

const laneModes: readonly DynamicLaneModeProjection[] = [
	"keyframes",
	"max_min",
	"middle_amplitude",
	"random",
];
const periodicFunctions: readonly DynamicPeriodicFunctionProjection[] = [
	"sinus",
	"cosinus",
	"linear_up",
	"linear_down",
	"pwm",
];
const interpolations = [
	"linear",
	"ease_in",
	"ease_out",
	"ease_in_out",
	"hold",
	"drop",
] as const;

function phaseDirectionSlot(
	dynamic: DynamicDefinitionProjection,
	onMutate: (
		intent: DynamicUpdateIntent,
		mutationGroup?: string,
	) => Promise<void>,
): DynamicEncoderSlot {
	const ordering = dynamic.phase.ordering;
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
			onMutate(
				{
					type: "set_phase",
					phase: {
						...dynamic.phase,
						ordering: direction
							? { ...ordering, angle_degrees: normalizeDegrees(next) }
							: spatial
								? { ...ordering, center_x: next }
								: ordering,
					},
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

function LaneChooser({
	slot,
	attributes,
	busy,
	onCancel,
	onChoose,
}: {
	slot: number;
	attributes: readonly { id: string; label: string; family: string }[];
	busy: boolean;
	onCancel(): void;
	onChoose(attribute: string): void;
}) {
	const [attribute, setAttribute] = useState(attributes[0]?.id ?? "");
	return (
		<ModalFrame
			id={`create-dynamic-${slot}`}
			ariaLabel={`Create Dynamic ${slot}`}
			title={`Create Dynamic ${slot}`}
			details="Choose the first lane"
			onClose={onCancel}
		>
			<section className="dynamic-create-form">
				<p>
					The Dynamic is created only after a valid scalar lane is selected.
				</p>
				<FormLayout labelPlacement="side">
					<SelectField
						label="First lane"
						value={attribute}
						options={attributes.map((candidate) => ({
							value: candidate.id,
							label: `${candidate.family} · ${candidate.label}`,
						}))}
						onChange={setAttribute}
					/>
				</FormLayout>
				{attributes.length === 0 && (
					<p role="alert">No continuous scalar attributes are available.</p>
				)}
				<footer>
					<Button onClick={onCancel}>Cancel</Button>
					<Button
						variant="primary"
						disabled={busy || !attribute}
						onClick={() => onChoose(attribute)}
					>
						{busy ? "Creating…" : "Create and edit"}
					</Button>
				</footer>
			</section>
		</ModalFrame>
	);
}

function ScalarSourceControl({
	label = "Source",
	source,
	attribute,
	presets,
	onChange,
}: {
	label?: string;
	source: DynamicScalarSourceProjection;
	attribute: string;
	presets: readonly PresetObject[];
	onChange?(source: DynamicScalarSourceProjection): void;
}) {
	const matchingPresets = presets.filter((preset) =>
		Object.values(preset.body.values).some((values) =>
			Object.hasOwn(values, attribute),
		),
	);
	return (
		<label>
			{label}
			<select
				value={source.type}
				onChange={(event) =>
					onChange?.(
						event.target.value === "current"
							? sourceCurrent
							: event.target.value === "value"
								? sourceZero
								: {
										type: "preset",
										preset_id: matchingPresets[0]?.id ?? "",
										attribute,
										last_valid_by_target: [],
									},
					)
				}
			>
				<option value="current">Current</option>
				<option value="value">Value</option>
				<option value="preset" disabled={matchingPresets.length === 0}>
					Preset
				</option>
			</select>
			{source.type === "value" && (
				<input
					type="number"
					min={0}
					max={1}
					step={0.01}
					value={source.value}
					onChange={(event) =>
						onChange?.({
							type: "value",
							value: Number(event.target.value),
						})
					}
				/>
			)}
			{source.type === "preset" && (
				<select
					aria-label={`${label} Preset`}
					value={source.preset_id}
					onChange={(event) =>
						onChange?.({
							type: "preset",
							preset_id: event.target.value,
							attribute,
							last_valid_by_target: source.last_valid_by_target,
						})
					}
				>
					{matchingPresets.length === 0 && (
						<option value="">No matching Preset</option>
					)}
					{matchingPresets.map((preset) => (
						<option key={preset.id} value={preset.id}>
							{preset.body.number} · {preset.body.name}
						</option>
					))}
				</select>
			)}
		</label>
	);
}

function NumberControl({
	label,
	value,
	suffix,
	onChange,
}: {
	label: string;
	value: number;
	suffix?: string;
	onChange?(value: number): void;
}) {
	return (
		<label>
			{label}
			<span className="dynamic-number-control">
				<input
					type="number"
					value={value}
					onChange={(event) => onChange?.(Number(event.target.value))}
					disabled={!onChange}
				/>
				{suffix && <small>{suffix}</small>}
			</span>
		</label>
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

function scaleRational(
	value: { numerator: number; denominator: number },
	factor: number,
) {
	const numerator =
		factor >= 1 ? value.numerator * Math.round(factor) : value.numerator;
	const denominator =
		factor >= 1
			? value.denominator
			: value.denominator * Math.round(1 / factor);
	const divisor = greatestCommonDivisor(numerator, denominator);
	return {
		numerator: Math.min(1_000_000, numerator / divisor),
		denominator: Math.min(1_000_000, denominator / divisor),
	};
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

function curvePath(lane: DynamicLaneProjection) {
	if (lane.mode === "keyframes")
		return "M0 220 C180 220 300 40 500 40 S820 220 1000 220";
	const functionName =
		lane.mode === "middle_amplitude"
			? lane.middle_amplitude.function
			: lane.max_min.function;
	if (functionName === "linear_up") return "M0 220 L1000 35";
	if (functionName === "linear_down") return "M0 35 L1000 220";
	if (functionName === "pwm") return "M0 220 L100 35 H500 L600 220 H1000";
	return "M0 130 C120 15 380 15 500 130 S880 245 1000 130";
}

function laneConfigSize(lane: DynamicLaneProjection) {
	if (lane.mode === "keyframes") return lane.keyframes.size;
	if (lane.mode === "middle_amplitude") return lane.middle_amplitude.size;
	return lane.max_min.size;
}

function setLaneSizeValue(lane: DynamicLaneProjection, value: number) {
	const adjusted = clamp(value, 0, 2);
	if (lane.mode === "keyframes")
		return {
			...lane,
			keyframes: { ...lane.keyframes, size: adjusted },
		};
	if (lane.mode === "middle_amplitude")
		return {
			...lane,
			middle_amplitude: {
				...lane.middle_amplitude,
				size: adjusted,
			},
		};
	return {
		...lane,
		max_min: { ...lane.max_min, size: adjusted },
	};
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

function periodicFunctionIndex(lane: DynamicLaneProjection) {
	if (lane.mode === "middle_amplitude")
		return periodicFunctions.indexOf(lane.middle_amplitude.function);
	return periodicFunctions.indexOf(lane.max_min.function);
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

function setLanePeriodicFunction(
	lane: DynamicLaneProjection,
	value: number,
): DynamicLaneProjection {
	const functionName =
		periodicFunctions[wrappedIndex(value, periodicFunctions.length)];
	if (lane.mode === "middle_amplitude")
		return {
			...lane,
			middle_amplitude: { ...lane.middle_amplitude, function: functionName },
		};
	return {
		...lane,
		max_min: { ...lane.max_min, function: functionName },
	};
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

function orderingFor(type: string): DynamicPhaseOrderingProjection {
	switch (type) {
		case "grid_linear":
			return { type, angle_degrees: 90 };
		case "radial_out":
		case "radial_in":
		case "axial":
			return { type, center_x: 0, center_z: 0 };
		case "random_each_loop":
			return { type, seed: Date.now() };
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
