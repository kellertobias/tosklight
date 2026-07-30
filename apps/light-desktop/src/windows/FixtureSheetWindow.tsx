import { FixtureSheetTableView as FixtureSheetTable } from "@tosklight/ui/tables";
import { WindowHeader } from "@tosklight/ui/window-kit";
import {
	type ReactNode,
	useCallback,
	useDeferredValue,
	useMemo,
	useState,
} from "react";
import { GroupStrip } from "../components/shared/GroupStrip";
import { SourceLegend } from "../components/shared/SourceLegend";
import { useHighlightSnapshot } from "../features/highlight/HighlightState";
import { useProgrammerPreloadLifecycleView } from "../features/programmerPreloadLifecycle/ProgrammerPreloadLifecycleView";
import { useProgrammerPreloadValuesView } from "../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import { useProgrammerValuesView } from "../features/programmerValues/ProgrammerValuesView";
import {
	useProgrammingSelectionActions,
	useProgrammingSelectionView,
} from "../features/programmingInteraction/ProgrammingInteractionView";
import { useApp } from "../state/AppContext";
import type { FixtureSheetColumn } from "../types";
import {
	DEFAULT_FIXTURE_SHEET_COLUMNS,
	FixtureSheetSettings,
} from "./FixtureSheetSettings";
import { fixtureSheetColumns } from "./fixtureSheetColumns";
import { useFixtureSheetCuelistAuthority } from "./fixtureSheetCuelistAuthority";
import {
	useFixtureSheetRows,
	useFixtureSheetVisualizations,
} from "./fixtureSheetProjection";
import { createFixtureStepPresenter } from "./fixtureSheetStep";
import type { WindowProps } from "./windowTypes";

export function FixtureSheetWindow({
	active = true,
	compact,
	viewOnly = false,
	showGroupShortcuts,
	fixtureSheetIncludedHeads,
	fixtureSheetOrder,
	fixtureSheetActiveOnly,
	fixtureSheetCueListId,
	fixtureSheetColumns: forcedColumns,
	fixtureSheetShowType,
}: WindowProps) {
	const highlight = useHighlightSnapshot();
	const interactionActive = active && !viewOnly;
	const selection = useProgrammingSelectionView(interactionActive);
	const selectionActions = useProgrammingSelectionActions(interactionActive);
	const preload = useProgrammerPreloadLifecycleView(active);
	const { state } = useApp();
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	const [activeRow, setActiveRow] = useState(0);
	const [visibleFixtureIds, onVisibleFixtureIdsChange] = useVisibleFixtureIds();
	const groupsVisible = compact
		? Boolean(showGroupShortcuts)
		: state.fixtureGroupsVisible;
	const fixtureOrder = compact
		? (fixtureSheetOrder ?? "fixture-id")
		: state.fixtureSheetOrder;
	const activeOnly = compact
		? (fixtureSheetActiveOnly ?? false)
		: state.fixtureSheetActiveOnly;
	const cuelistFilter = useFixtureSheetCuelistAuthority({
		enabled: active && (!compact || fixtureSheetCueListId != null),
		savedCueListId: compact
			? (fixtureSheetCueListId ?? "")
			: state.fixtureSheetCueListId,
	});
	const cueListId = cuelistFilter.selectedCueListId;
	const visibleColumnIds = compact
		? (forcedColumns ?? DEFAULT_FIXTURE_SHEET_COLUMNS)
		: state.fixtureSheetColumns;
	const showType = compact
		? (fixtureSheetShowType ?? true)
		: state.fixtureSheetShowType;
	const includedHeads = compact
		? (fixtureSheetIncludedHeads ?? "all")
		: state.fixtureSheetIncludedHeads;
	const { visualization, preloadVisualization } = useFixtureSheetVisualizations(
		preload.armed || preload.active,
		active,
		visibleFixtureIds,
	);
	const programmerValues = useProgrammerValuesView(active);
	const preloadProgrammerValues = useProgrammerPreloadValuesView(
		active && (preload.armed || preload.active),
	);
	const deferredVisualization = useDeferredValue(visualization);
	const deferredPreloadVisualization = useDeferredValue(preloadVisualization);
	const deferredProgrammerValues = useDeferredValue(programmerValues);
	const deferredPreloadProgrammerValues = useDeferredValue(
		preloadProgrammerValues,
	);
	const { rows, activeValuesLoading, groupRuntimeLoading } =
		useFixtureSheetRows({
			visualization: deferredVisualization,
			preloadVisualization: deferredPreloadVisualization,
			programmerValues: deferredProgrammerValues,
			preloadProgrammerValues: deferredPreloadProgrammerValues,
			fixtureOrder,
			activeOnly,
			selectedCueList: cuelistFilter.selectedCueList,
			includedHeads,
			active,
		});
	const presentStep = useMemo(
		() => createFixtureStepPresenter(highlight),
		[highlight],
	);
	const columns = useMemo(
		() =>
			fixtureSheetColumns(showType, presentStep).filter((column) =>
				visibleColumnIds.includes(column.id as FixtureSheetColumn),
			),
		[presentStep, showType, visibleColumnIds],
	);
	const selectedFixtureIds = useMemo(
		() => new Set(selection?.selected ?? []),
		[selection?.selected],
	);
	const selectionActionStatus = resolveSelectionActionStatus(selectionActions);

	return (
		<FixtureSheetWindowView
			compact={compact}
			selectionCount={
				selection && selectionActionStatus === "ready"
					? selection.selected.length
					: null
			}
			selectionActionStatus={selectionActionStatus}
			info={<SourceLegend />}
			activeValuesLoading={activeValuesLoading}
			groupRuntimeLoading={groupRuntimeLoading}
			onSettings={(anchor) => setSettingsAnchor(anchor.getBoundingClientRect())}
			table={
				<FixtureSheetTable
					activeRow={activeRow}
					columns={columns}
					onActivate={
						viewOnly
							? undefined
							: (fixtureId) =>
									void selectionActions?.gesture({
										source: { type: "fixture", fixtureId },
										resolvedFixtures: [fixtureId],
									})
					}
					onActiveRowChange={viewOnly ? undefined : setActiveRow}
					onVisibleFixtureIdsChange={onVisibleFixtureIdsChange}
					presentStep={presentStep}
					rows={rows}
					selectedFixtureIds={selectedFixtureIds}
				/>
			}
			groups={
				groupsVisible ? (
					<GroupStrip active={active} viewOnly={viewOnly} />
				) : null
			}
			settings={
				settingsAnchor ? (
					<FixtureSheetSettings
						activeOnly={activeOnly}
						anchor={settingsAnchor}
						cueLists={cuelistFilter.cueLists}
						cueListId={cueListId}
						fixtureOrder={fixtureOrder}
						groupsVisible={groupsVisible}
						includedHeads={includedHeads}
						onClose={() => setSettingsAnchor(null)}
					/>
				) : null
			}
		/>
	);
}

function useVisibleFixtureIds() {
	const [fixtureIds, setFixtureIds] = useState<readonly string[]>([]);
	const onChange = useCallback((next: readonly string[]) => {
		setFixtureIds((current) =>
			current.length === next.length &&
			current.every((fixtureId, index) => fixtureId === next[index])
				? current
				: [...next],
		);
	}, []);
	return [fixtureIds, onChange] as const;
}

function resolveSelectionActionStatus(
	actions: ReturnType<typeof useProgrammingSelectionActions>,
) {
	if (typeof actions?.status === "function") return actions.status();
	return actions ? ("ready" as const) : ("loading" as const);
}

export function FixtureSheetWindowView({
	compact,
	selectionCount,
	selectionActionStatus,
	info,
	activeValuesLoading,
	groupRuntimeLoading,
	onSettings,
	table,
	groups,
	settings,
}: {
	compact?: boolean;
	selectionCount: number | null;
	selectionActionStatus?: "ready" | "loading" | "scope-mismatch" | "stopped";
	info?: ReactNode;
	activeValuesLoading?: boolean;
	groupRuntimeLoading?: boolean;
	onSettings?: (anchor: HTMLElement) => void;
	table: ReactNode;
	groups?: ReactNode;
	settings?: ReactNode;
}) {
	return (
		<div
			className="fixture-window"
			data-selection-action-status={selectionActionStatus}
		>
			{!compact && (
				<WindowHeader
					title="Fixture Sheet"
					info={{
						primary:
							selectionCount == null
								? "Selection loading…"
								: `${selectionCount} selected`,
						secondary: info,
					}}
					settings
					onSettings={onSettings}
				/>
			)}
			{activeValuesLoading && (
				<p className="fixture-sheet-loading" role="status">
					Programmer values loading…
				</p>
			)}
			{groupRuntimeLoading && (
				<p className="fixture-sheet-loading" role="status">
					Group runtime loading…
				</p>
			)}
			{table}
			{groups}
			{settings}
		</div>
	);
}
