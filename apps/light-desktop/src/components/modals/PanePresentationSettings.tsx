import { MultiValueToggleField, NumberField, TextField } from "@tosklight/ui";
import { isTiledPane } from "../../features/panePresentation/tiledPaneKinds";
import { useScreensOptional } from "../../features/screens/ScreensContext";
import { useApp } from "../../state/AppContext";
import type { PaneModel } from "../../types";
import { availableWindowChoices } from "./WindowPicker";

export function PaneTitleSettings({ pane }: { pane: PaneModel }) {
	const { dispatch } = useApp();
	const typeName = availableWindowChoices().find(([kind]) => kind === pane.kind)?.[1] ?? pane.kind;
	return (
		<>
			<p>Pane type: <b>{typeName}</b></p>
			<TextField
				label="Pane title"
				value={pane.title}
				maxLength={256}
				onChange={(event) => dispatch({
					type: "SET_PANE_TITLE", id: pane.id, value: event.target.value,
				})}
			/>
		</>
	);
}

function tileLayoutDetail(pane: PaneModel, legacyColumns: boolean, authoredPlayback: boolean) {
	if (legacyColumns)
		return `This saved layout uses ${pane.poolColumns} fixed columns with square tiles. Change rows or columns to fit a grid to the pane, or choose Automatic squares.`;
	if (authoredPlayback)
		return "This Playback pane fits its authored layout. Change rows or columns to use a uniform fixed grid, or choose Automatic squares.";
	return pane.poolTileLayout?.mode === "fixed"
		? "Rows and columns fill the pane. Scroll to reach later entries."
		: "Square tiles follow the Desktop's tile sizes and adapt to the pane width.";
}

export function PaneTileSettings({ pane }: { pane: PaneModel }) {
	const { dispatch } = useApp();
	const screens = useScreensOptional();
	if (!isTiledPane(pane.kind)) return null;
	const legacyColumns = pane.poolTileLayout === undefined && pane.poolColumns !== undefined;
	const authoredPlayback = pane.kind === "playbacks" && pane.poolTileLayout === undefined;
	const authoredRows = screens?.surfaces?.playback_surfaces[pane.surface_id ?? `pane:${pane.id}`]?.layout.rows;
	const mode = pane.poolTileLayout?.mode ?? (legacyColumns || authoredPlayback ? "fixed" : "automatic");
	const fixed = pane.poolTileLayout?.mode === "fixed" ? pane.poolTileLayout : {
		mode: "fixed" as const,
		rows: authoredPlayback ? authoredRows?.length ?? 14 : 4,
		columns: pane.poolColumns ?? (authoredPlayback ? Math.max(1, ...(authoredRows?.map(row => row.count) ?? [20])) : 4),
	};
	const unavailable = pane.kind === "playbacks" && (pane.playbackShowControls || pane.playbackViewMode === "screen");
	const setFixed = (rows: number, columns: number) => dispatch({
		type: "SET_PANE_TILE_LAYOUT", id: pane.id, layout: { mode: "fixed", rows, columns },
	});
	return (
		<section aria-label="Tile layout settings">
			<MultiValueToggleField
				label="Tile layout"
				value={mode}
				disabled={Boolean(unavailable)}
				options={[
					{ value: "automatic", label: "Automatic squares" },
					{ value: "fixed", label: "Fixed grid" },
				]}
				onChange={(value) => value === "automatic"
					? dispatch({ type: "SET_PANE_TILE_LAYOUT", id: pane.id, layout: { mode: "automatic" } })
					: setFixed(fixed.rows, fixed.columns)}
			/>
			{unavailable ? (
				<p>Choose Free region and hide Playback controls to configure tile layout. Screen layouts and fader controls keep their authored rows.</p>
			) : (
				<>
					{mode === "fixed" && <div className="dialog-grid">
						<NumberField
							label="Tile rows" min="1" max="24" value={fixed.rows}
							onChange={(event) => setFixed(Number(event.target.value), fixed.columns)}
						/>
						<NumberField
							label="Tile columns" min="1" max="24" value={fixed.columns}
							onChange={(event) => setFixed(fixed.rows, Number(event.target.value))}
						/>
					</div>}
					<p>{tileLayoutDetail(pane, legacyColumns, authoredPlayback)}</p>
				</>
			)}
		</section>
	);
}
