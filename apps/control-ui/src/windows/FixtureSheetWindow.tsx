import { useMemo, useState } from "react";
import { useServer } from "../api/ServerContext";
import { GroupStrip } from "../components/shared/GroupStrip";
import { SourceLegend } from "../components/shared/SourceLegend";
import { WindowHeader } from "../components/window-kit";
import {
	useProgrammingSelectionActions,
	useProgrammingSelectionView,
} from "../features/programmingInteraction/ProgrammingInteractionView";
import { useProgrammerPreloadLifecycleView } from "../features/programmerPreloadLifecycle/ProgrammerPreloadLifecycleView";
import { useApp } from "../state/AppContext";
import type { FixtureSheetColumn } from "../types";
import {
	DEFAULT_FIXTURE_SHEET_COLUMNS,
	FixtureSheetSettings,
} from "./FixtureSheetSettings";
import { FixtureSheetTable } from "./FixtureSheetTable";
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
	showGroupShortcuts,
}: WindowProps) {
	const server = useServer();
	const selection = useProgrammingSelectionView(active);
	const selectionActions = useProgrammingSelectionActions(active);
	const preload = useProgrammerPreloadLifecycleView(active);
	const { state } = useApp();
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	const [activeRow, setActiveRow] = useState(0);
	const groupsVisible = compact
		? Boolean(showGroupShortcuts)
		: state.fixtureGroupsVisible;
	const fixtureOrder = compact ? "fixture-id" : state.fixtureSheetOrder;
	const activeOnly = compact ? false : state.fixtureSheetActiveOnly;
	const cuelistFilter = useFixtureSheetCuelistAuthority({
		enabled: active && !compact,
		savedCueListId: state.fixtureSheetCueListId,
	});
	const cueListId = cuelistFilter.selectedCueListId;
	const visibleColumnIds = compact
		? DEFAULT_FIXTURE_SHEET_COLUMNS
		: state.fixtureSheetColumns;
	const showType = compact || state.fixtureSheetShowType;
	const includedHeads = compact ? "all" : state.fixtureSheetIncludedHeads;
	const { visualization, preloadVisualization } = useFixtureSheetVisualizations(
		preload.armed || preload.active,
		active,
	);
	const { rows, activeValuesLoading, groupRuntimeLoading } =
		useFixtureSheetRows({
			visualization,
			preloadVisualization,
			fixtureOrder,
			activeOnly,
			selectedCueList: cuelistFilter.selectedCueList,
			includedHeads,
			active,
		});
	const presentStep = useMemo(
		() => createFixtureStepPresenter(server.highlight),
		[server.highlight],
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

	return (
		<div className="fixture-window">
			{!compact && (
				<WindowHeader
					title="Fixture Sheet"
					info={{
						primary: `${selection?.selected.length ?? 0} selected`,
						secondary: <SourceLegend />,
					}}
					settings
					onSettings={(anchor) =>
						setSettingsAnchor(anchor.getBoundingClientRect())
					}
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
			<FixtureSheetTable
				activeRow={activeRow}
				columns={columns}
				onActivate={(fixtureId) =>
					void selectionActions?.gesture({
						source: { type: "fixture", fixtureId },
						resolvedFixtures: [fixtureId],
					})
				}
				onActiveRowChange={setActiveRow}
				presentStep={presentStep}
				rows={rows}
				selectedFixtureIds={selectedFixtureIds}
			/>
			{groupsVisible && <GroupStrip active={active} />}
			{settingsAnchor && (
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
			)}
		</div>
	);
}
