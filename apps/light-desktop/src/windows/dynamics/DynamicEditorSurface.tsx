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
import { createLightApi } from "../../api/client/api";
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
} from "../../api/types";
import { useCommandLineSurface } from "../../components/control/commandLine/useCommandLineSurface";
import { monotonicEpochMillis } from "../../components/control/soundToLightAnalyzer";
import { useSoundToLight } from "../../components/control/useSoundToLight";
import {
	useActiveShowId,
	useAttributeRegistry,
	useHardwareConnected,
} from "../../features/deskSnapshot/DeskSnapshotState";
import { useDynamicEditorSession } from "../../features/dynamics/DynamicEditorSessionContext";
import { DynamicMutationWriter } from "../../features/dynamics/DynamicMutationWriter";
import { useDynamicsActions } from "../../features/dynamics/DynamicsActionsContext";
import {
	useProgrammingCommandLineActions,
	useProgrammingDeleteCommandActive,
} from "../../features/programmingInteraction/ProgrammingInteractionView";
import type { ShowObject } from "../../features/showObjects/contracts";
import {
	useDynamics,
	usePresets,
	useShowObjectsStore,
} from "../../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../../features/showObjects/ShowObjectsView";
import { useSpeedGroupRuntimeView } from "../../features/speedGroupRuntime/SpeedGroupRuntimeView";
import { useApp } from "../../state/AppContext";
import { useStageLayout } from "../stageWindow/useStageLayout";

import type { DynamicEditorProps, DynamicEditorView } from "./DynamicsEditor";
import {
	coverageSummary,
	LaneAttributeModal,
	targetSummary,
} from "./DynamicsEditor";
import { CurvesView } from "./CurvesView";
import { PhaseView, SpeedView } from "./PhaseSpeedViews";

type DynamicObject = ShowObject<"dynamic">;

interface DynamicEditorSurfaceProps {
	dynamic: DynamicObject;
	compact: boolean;
	busy: boolean;
	error: string | null;
	attributes: readonly { id: string; label: string; family: string }[];
	runtime: DynamicRuntimeSnapshotProjection | null;
	speedGroupBpms?: Partial<Record<SpeedGroupId, number>>;
	selection: readonly string[];
	view: DynamicEditorView;
	lane: DynamicLaneProjection | undefined;
	selectedLanes: ReadonlySet<string>;
	shiftArmed: boolean;
	primaryKeyframeIndex: number;
	previewing: boolean;
	previewPhase: number;
	settingsAnchor: DOMRect | null;
	addingLane: boolean;
	running: boolean;
	status: DynamicDefinitionStatusProjection | null;
	contentSidebar: ReactNode;
	contentFooter: ReactNode;
	onBack(): void;
	onChangeView(view: DynamicEditorView): void;
	onPreviewing(update: (current: boolean) => boolean): void;
	onPreviewPhase(value: number): void;
	onSettingsAnchor(anchor: DOMRect | null): void;
	onAddingLane(value: boolean): void;
	onAddLane(attribute: string): void;
	onTakeSelection(): void;
	onClearSelection(): void;
	onPrimaryKeyframeIndex(index: number): void;
	onSelectLane(id: string, additive: boolean): void;
	onReplaceLane(lane: DynamicLaneProjection, group?: string): Promise<void>;
	onMutate: DynamicEditorProps["onMutate"];
	onSpeedGroupTap?: DynamicEditorProps["onSpeedGroupTap"];
}

export function DynamicEditorSurface(props: DynamicEditorSurfaceProps) {
	return (
		<section
			className={`dynamics-window dynamics-editor dynamic-full-discussion-editor ${props.compact ? "compact" : ""}`}
			aria-busy={props.busy}
		>
			<DynamicEditorHeader {...props} />
			<DynamicEditorSettings {...props} />
			{props.addingLane && (
				<LaneAttributeModal
					id={`select-lane-attribute-${props.dynamic.id}`}
					title="Select lane attribute"
					details="Choose the attribute controlled by the new lane"
					attributes={props.attributes}
					busy={props.busy}
					onClose={() => props.onAddingLane(false)}
					onChoose={props.onAddLane}
				/>
			)}
			{props.error && (
				<p className="dynamics-error" role="alert">
					{props.error}
				</p>
			)}
			<DynamicEditorWorkspace {...props} />
		</section>
	);
}

function DynamicEditorHeader({
	dynamic,
	view,
	previewing,
	onBack,
	onChangeView,
	onPreviewing,
	onPreviewPhase,
	onSettingsAnchor,
	onAddingLane,
}: DynamicEditorSurfaceProps) {
	return (
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
								onClick: () => onAddingLane(true),
							},
						]
					: [],
				[
					{
						id: "curves",
						label: "Lanes",
						active: view === "curves",
						onClick: () => onChangeView("curves"),
					},
					{
						id: "phase",
						label: "Phase",
						active: view === "phase",
						onClick: () => onChangeView("phase"),
					},
					{
						id: "speed",
						label: "Speed",
						active: view === "speed",
						onClick: () => onChangeView("speed"),
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
							onPreviewing((current) => {
								if (current) onPreviewPhase(0);
								return !current;
							}),
					},
				],
				[{ id: "back", label: "← Back to Pool", onClick: onBack }],
			]}
			settings
			onSettings={(anchor) => onSettingsAnchor(anchor.getBoundingClientRect())}
		/>
	);
}

function DynamicEditorSettings({
	dynamic,
	settingsAnchor,
	status,
	running,
	selection,
	onSettingsAnchor,
	onTakeSelection,
	onClearSelection,
	onMutate,
}: DynamicEditorSurfaceProps) {
	if (!settingsAnchor) return null;
	return (
		<WindowSettings
			modal={false}
			anchor={settingsAnchor}
			title="Dynamic Settings"
			onClose={() => onSettingsAnchor(null)}
			tabs={[
				{
					id: "general",
					label: "General",
					content: (
						<DynamicGeneralSettings dynamic={dynamic} onMutate={onMutate} />
					),
				},
				{
					id: "targets",
					label: "Targets",
					content: (
						<DynamicTargetSettings
							dynamic={dynamic}
							status={status}
							running={running}
							selectionCount={selection.length}
							onTakeSelection={onTakeSelection}
							onClearSelection={onClearSelection}
						/>
					),
				},
			]}
		/>
	);
}

function DynamicGeneralSettings({
	dynamic,
	onMutate,
}: Pick<DynamicEditorSurfaceProps, "dynamic" | "onMutate">) {
	return (
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
				value={dynamic.body.icon ?? ""}
				onChange={(icon) => void onMutate(dynamic, { type: "set_icon", icon })}
			/>
			<ColorPickerField
				label="Color"
				value={dynamic.body.color ?? "#4edcff"}
				onChange={(color) =>
					void onMutate(dynamic, { type: "set_color", color })
				}
			/>
		</FormLayout>
	);
}

function DynamicTargetSettings({
	dynamic,
	status,
	running,
	selectionCount,
	onTakeSelection,
	onClearSelection,
}: Pick<
	DynamicEditorSurfaceProps,
	"dynamic" | "status" | "running" | "onTakeSelection" | "onClearSelection"
> & { selectionCount: number }) {
	return (
		<section className="dynamic-target-settings">
			<strong>{targetSummary(dynamic.body)}</strong>
			{status && <small>{coverageSummary(status)}</small>}
			{status?.warning && (
				<small className="dynamics-warning">{status.warning}</small>
			)}
			<div>
				<Button
					disabled={running || selectionCount === 0}
					title={
						running
							? "Turn every running instance Off before changing targets"
							: selectionCount === 0
								? "Select a Group or fixtures first"
								: undefined
					}
					onClick={onTakeSelection}
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
					onClick={onClearSelection}
				>
					Clear Selection
				</Button>
			</div>
		</section>
	);
}

function DynamicEditorWorkspace(props: DynamicEditorSurfaceProps) {
	return (
		<div className="dynamics-editor-body">
			<main className="dynamic-workspace">
				{props.view === "curves" && <CurvesWorkspace {...props} />}
				{props.view === "phase" && <PhaseWorkspace {...props} />}
				{props.view === "speed" && <SpeedWorkspace {...props} />}
			</main>
			{props.contentFooter}
		</div>
	);
}

function CurvesWorkspace(props: DynamicEditorSurfaceProps) {
	if (!props.lane)
		return (
			<div className="dynamic-view-with-sidebar">
				<section className="dynamic-empty-lanes">
					<Button onClick={() => props.onAddingLane(true)}>
						Add first lane
					</Button>
				</section>
				{props.contentSidebar}
			</div>
		);
	return (
		<CurvesView
			dynamic={props.dynamic}
			lane={props.lane}
			selectedLanes={props.selectedLanes}
			shiftArmed={props.shiftArmed}
			attributes={props.attributes}
			primaryKeyframeIndex={props.primaryKeyframeIndex}
			previewPhase={props.previewing ? props.previewPhase : null}
			contentSidebar={props.contentSidebar}
			onPrimaryKeyframeIndex={props.onPrimaryKeyframeIndex}
			onSelect={props.onSelectLane}
			onReplace={props.onReplaceLane}
			onMutate={props.onMutate}
		/>
	);
}

function PhaseWorkspace(props: DynamicEditorSurfaceProps) {
	return (
		<div className="dynamic-view-with-sidebar">
			<PhaseView
				dynamic={props.dynamic}
				lane={props.lane}
				running={props.running}
				selectionCount={props.selection.length}
				onSelectLane={(id) => props.onSelectLane(id, false)}
				onTakeSelection={props.onTakeSelection}
				onClearSelection={props.onClearSelection}
				onMutate={props.onMutate}
			/>
			{props.contentSidebar}
		</div>
	);
}

function SpeedWorkspace(props: DynamicEditorSurfaceProps) {
	return (
		<div className="dynamic-view-with-sidebar">
			<SpeedView
				dynamic={props.dynamic}
				runtime={props.runtime}
				previewPhase={props.previewing ? props.previewPhase : null}
				speedGroupBpms={props.speedGroupBpms}
				onMutate={props.onMutate}
				onSpeedGroupTap={props.onSpeedGroupTap}
			/>
			{props.contentSidebar}
		</div>
	);
}
