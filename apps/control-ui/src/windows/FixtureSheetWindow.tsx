import { useMemo, useState } from "react";
import { useServer } from "../api/ServerContext";
import { GroupStrip } from "../components/shared/GroupStrip";
import { SourceLegend } from "../components/shared/SourceLegend";
import { WindowHeader } from "../components/window-kit";
import { useApp } from "../state/AppContext";
import type { FixtureSheetColumn } from "../types";
import {
	DEFAULT_FIXTURE_SHEET_COLUMNS,
	FixtureSheetSettings,
} from "./FixtureSheetSettings";
import { FixtureSheetTable } from "./FixtureSheetTable";
import { fixtureSheetColumns } from "./fixtureSheetColumns";
import {
	useFixtureSheetRows,
	useFixtureSheetVisualizations,
} from "./fixtureSheetProjection";
import { createFixtureStepPresenter } from "./fixtureSheetStep";
import type { WindowProps } from "./windowTypes";
import { useShowObjectView } from "../features/showObjects/ShowObjectsView";

export function FixtureSheetWindow({
	active = true,
	compact,
	showGroupShortcuts,
}: WindowProps) {
	useShowObjectView("group", active);
	const server = useServer();
	const { state } = useApp();
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	const [activeRow, setActiveRow] = useState(0);
	const groupsVisible = compact
		? Boolean(showGroupShortcuts)
		: state.fixtureGroupsVisible;
	const fixtureOrder = compact ? "fixture-id" : state.fixtureSheetOrder;
	const activeOnly = compact ? false : state.fixtureSheetActiveOnly;
	const cueListId =
		compact ||
		!(server.playbacks?.cue_lists ?? []).some(
			(cueList) => cueList.id === state.fixtureSheetCueListId,
		)
			? ""
			: state.fixtureSheetCueListId;
	const visibleColumnIds = compact
		? DEFAULT_FIXTURE_SHEET_COLUMNS
		: state.fixtureSheetColumns;
	const showType = compact || state.fixtureSheetShowType;
	const includedHeads = compact ? "all" : state.fixtureSheetIncludedHeads;
	const { visualization, preloadVisualization } = useFixtureSheetVisualizations(
		state.preload !== "idle",
	);
	const rows = useFixtureSheetRows({
		visualization,
		preloadVisualization,
		fixtureOrder,
		activeOnly,
		cueListId,
		includedHeads,
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
		() => new Set(server.selectedFixtures),
		[server.selectedFixtures],
	);

	return (
		<div className="fixture-window">
			{!compact && (
				<WindowHeader
					title="Fixture Sheet"
					info={{
						primary: `${server.selectedFixtures.length} selected`,
						secondary: <SourceLegend />,
					}}
					settings
					onSettings={(anchor) =>
						setSettingsAnchor(anchor.getBoundingClientRect())
					}
				/>
			)}
			<FixtureSheetTable
				activeRow={activeRow}
				columns={columns}
				onActivate={(fixtureId) =>
					void server.selectionGesture({
						type: "fixture",
						fixture_id: fixtureId,
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
