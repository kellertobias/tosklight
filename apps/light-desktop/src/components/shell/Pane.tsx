import { type GridRect, PaneView } from "@tosklight/ui/desktop";
import { useRef, useState } from "react";
import {
	useProgrammingCommandLineActions,
	useProgrammingDeleteCommandActive,
	useProgrammingSelectionView,
} from "../../features/programmingInteraction/ProgrammingInteractionView";
import { useApp } from "../../state/AppContext";
import type { PaneModel } from "../../types";
import { windowRegistry } from "../../windows/WindowRegistry";
import { SourceLegend } from "../shared/SourceLegend";
import { PaneChromeProvider } from "./PaneChromeContext";
import { requestPaneRemoval } from "./paneRemovalGuard";

export function Pane({
	pane,
	active,
	maximized,
	editing,
}: {
	pane: PaneModel;
	active: boolean;
	maximized: boolean;
	editing: boolean;
}) {
	const { dispatch } = useApp();
	const selection = useProgrammingSelectionView(
		active &&
			(pane.kind === "stage" ||
				pane.kind === "fixtures" ||
				pane.kind === "layout"),
	);
	const commandLineActions = useProgrammingCommandLineActions();
	const deleteArmed = useProgrammingDeleteCommandActive();
	const lastFollowToggle = useRef(0);
	const [chromeInfo, setChromeInfo] = useState<HTMLSpanElement | null>(null);
	const [chromeToolbar, setChromeToolbar] = useState<HTMLSpanElement | null>(
		null,
	);
	const stageActions =
		pane.kind === "stage"
			? [
					[
						{
							id: "follow",
							label: "Follow Preload",
							active: Boolean(pane.followPreload),
							onClick: () => {
								const now = performance.now();
								if (now - lastFollowToggle.current < 400) return;
								lastFollowToggle.current = now;
								dispatch({
									type: "SET_PANE_STAGE_OPTION",
									id: pane.id,
									option: "followPreload",
									value: !pane.followPreload,
								});
							},
						},
					],
					[
						{
							id: "groups",
							label: "Groups",
							onClick: () =>
								dispatch({ type: "OPEN_GROUPS_FROM_STAGE", origin: "desk" }),
						},
					],
				]
			: [];
	const gridDimensions = { columns: 24, rows: 18 };
	const removeFromDelete = () => {
		if (!deleteArmed || !requestPaneRemoval(pane.id)) return;
		dispatch({ type: "REMOVE_PANE", id: pane.id });
		void commandLineActions?.reset();
	};
	const updateRect = (candidate: GridRect) => {
		const rect =
			candidate.x !== pane.x || candidate.y !== pane.y
				? { x: candidate.x, y: candidate.y }
				: { width: candidate.width, height: candidate.height };
		dispatch({ type: "SET_PANE_RECT", id: pane.id, rect });
	};
	return (
		<PaneView
			pane={{
				...pane,
				title: pane.kind === "file_manager" ? "File Manager" : pane.title,
				type: pane.kind,
			}}
			active={active}
			maximized={maximized}
			editing={editing}
			dimensions={gridDimensions}
			info={
				pane.kind === "file_manager"
					? {
							primary: "Browse and manage files",
							secondary: (
								<span className="pane-chrome-info-target" ref={setChromeInfo} />
							),
						}
					: pane.kind === "text_editor"
						? {
								primary: (
									<span
										className="pane-chrome-info-target"
										ref={setChromeInfo}
									/>
								),
							}
						: pane.kind === "stage"
							? {
									primary: `${selection?.selected.length ?? 0} selected`,
									secondary: "Tap to select · Shift for range",
								}
							: pane.kind === "fixtures"
								? {
										primary: `${selection?.selected.length ?? 0} selected`,
										secondary: <SourceLegend />,
									}
								: pane.kind === "layout"
									? {
											primary: `${selection?.selected.length ?? 0} selected`,
											secondary: "Live intensity and color",
										}
									: undefined
			}
			toolbar={
				pane.kind === "file_manager" ||
				pane.kind === "text_editor" ||
				pane.kind === "virtual_playbacks" ? (
					<span className="pane-chrome-toolbar-target" ref={setChromeToolbar} />
				) : undefined
			}
			actions={stageActions}
			settings
			onSettings={() => dispatch({ type: "SET_PANE_SETTINGS", id: pane.id })}
			onTitleClick={deleteArmed ? removeFromDelete : undefined}
			titleActionLabel={
				deleteArmed
					? `Remove ${pane.kind === "file_manager" ? "File Manager" : pane.title} pane`
					: undefined
			}
			onRectChange={updateRect}
		>
			<PaneContent
				active={active}
				chromeInfo={chromeInfo}
				chromeToolbar={chromeToolbar}
				pane={pane}
			/>
		</PaneView>
	);
}

function PaneContent({
	active,
	chromeInfo,
	chromeToolbar,
	pane,
}: {
	active: boolean;
	chromeInfo: HTMLSpanElement | null;
	chromeToolbar: HTMLSpanElement | null;
	pane: PaneModel;
}) {
	const { state, dispatch } = useApp();
	const Window = windowRegistry[pane.kind];
	return (
		<PaneChromeProvider value={{ info: chromeInfo, toolbar: chromeToolbar }}>
			<Window
				active={active}
				compact
				paneId={pane.id}
				showGroupShortcuts={Boolean(pane.showGroupShortcuts)}
				fixtureSheetActiveOnly={Boolean(pane.fixtureSheetActiveOnly)}
				showCueSidebar={pane.showCueSidebar ?? true}
				cueListSource={pane.cueListSource ?? "fixed"}
				fixedCueListNumber={pane.fixedCueListNumber}
				stageView={pane.stageView ?? state.stageView}
				followPreload={Boolean(pane.followPreload)}
				showBeamGuides={pane.showBeamGuides ?? true}
				stageRenderQuality={pane.stageRenderQuality ?? "lines_and_beams"}
				layoutGroupId={pane.layoutGroupId}
				presetFamily={pane.presetFamily ?? state.presetFamily}
				presetPoolColors={pane.presetPoolColors ?? true}
				poolColumns={pane.poolColumns}
				schedulerShowList={pane.schedulerShowList ?? true}
				schedulerShowCalendar={pane.schedulerShowCalendar ?? true}
				onSchedulerLayoutChange={({ showList, showCalendar }) =>
					dispatch({
						type: "SET_PANE_SCHEDULER_LAYOUT",
						id: pane.id,
						showList,
						showCalendar,
					})
				}
			/>
		</PaneChromeProvider>
	);
}
