import { GridDesktop } from "@tosklight/ui/desktop";
import { useApp } from "../../state/AppContext";
import {
	type DeskModel,
	GRID_COLUMNS,
	GRID_ROWS,
	type GridRect,
} from "../../types";
import { PaneSettingsModal } from "../modals/PaneSettingsModal";
import { WindowPicker } from "../modals/WindowPicker";
import { Pane } from "./Pane";

/**
 * Rectangles where the desktop grid may still paint without covering a native Stage surface.
 *
 * The native renderer sits beneath the webview, so even the grid painted behind a transparent
 * Stage pane would be composited over the picture. Split the remaining grid into horizontal runs
 * and merge equal runs vertically; this keeps the normal grid everywhere outside Stage panes
 * without putting a web pixel over the native picture.
 */
export function stageGridBackdropRects(desk: DeskModel): GridRect[] {
	const stagePanes = desk.panes.filter((pane) => pane.kind === "stage");
	if (stagePanes.length === 0) return [];

	const rectangles: GridRect[] = [];
	for (let y = 1; y <= GRID_ROWS; y += 1) {
		let x = 1;
		while (x <= GRID_COLUMNS) {
			const covered = stagePanes.some(
				(pane) =>
					x >= pane.x &&
					x < pane.x + pane.width &&
					y >= pane.y &&
					y < pane.y + pane.height,
			);
			if (covered) {
				x += 1;
				continue;
			}

			const start = x;
			do {
				x += 1;
			} while (
				x <= GRID_COLUMNS &&
				!stagePanes.some(
					(pane) =>
						x >= pane.x &&
						x < pane.x + pane.width &&
						y >= pane.y &&
						y < pane.y + pane.height,
				)
			);

			const width = x - start;
			const above = rectangles.find(
				(rectangle) =>
					rectangle.x === start &&
					rectangle.width === width &&
					rectangle.y + rectangle.height === y,
			);
			if (above) above.height += 1;
			else rectangles.push({ x: start, y, width, height: 1 });
		}
	}
	return rectangles;
}

export function DeskGrid({ desk }: { desk: DeskModel }) {
	const { state, dispatch } = useApp();
	const empty = desk.panes.length === 0;
	const maximizedStage = desk.panes.some(
		(pane) => pane.kind === "stage" && pane.id === state.maximizedPaneId,
	);
	const stageBackdrops = maximizedStage ? [] : stageGridBackdropRects(desk);
	const openAt = (rect: GridRect) =>
		dispatch({ type: "OPEN_WINDOW_PICKER", rect });
	return (
		<GridDesktop
			id={desk.id}
			name={desk.name}
			dimensions={{ columns: GRID_COLUMNS, rows: GRID_ROWS }}
			editing={Boolean(state.paneSettingsId)}
			empty={empty}
			onOpen={openAt}
		>
			{stageBackdrops.map((rect) => (
				<span
					aria-hidden="true"
					className="stage-grid-backdrop"
					key={`${rect.x}:${rect.y}:${rect.width}:${rect.height}`}
					style={
						{
							gridColumn: `${rect.x} / span ${rect.width}`,
							gridRow: `${rect.y} / span ${rect.height}`,
							"--stage-grid-backdrop-columns": rect.width,
							"--stage-grid-backdrop-rows": rect.height,
						} as React.CSSProperties
					}
				/>
			))}
			{desk.panes.map((pane) => (
				<Pane
					key={pane.id}
					pane={pane}
					active={
						state.maximizedPaneId == null || state.maximizedPaneId === pane.id
					}
					maximized={state.maximizedPaneId === pane.id}
					editing={state.paneSettingsId === pane.id}
				/>
			))}
			<WindowPicker />
			<PaneSettingsModal />
		</GridDesktop>
	);
}
