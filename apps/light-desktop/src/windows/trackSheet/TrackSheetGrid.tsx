import { Button } from "@tosklight/ui";
import { useLayoutEffect, useRef, useState } from "react";
import type { Cue } from "../../api/types/playback";
import { type TrackRow, trackCell } from "./trackSheetModel";
import {
	type PresetName,
	TRACK_STATE_LABELS,
	TRACK_STATE_MARKS,
	trackValueSwatch,
	trackValueText,
} from "./trackSheetValues";

export const TRACK_ROW_HEIGHT = 40;
export const TRACK_COLUMN_WIDTH = 112;
const OVERSCAN = 4;

export interface TrackSelection {
	rowKey: string;
	cue: number;
}

/** Which slice of a long axis is on screen, with a little overscan for touch flings. */
function visibleRange(offset: number, extent: number, size: number, count: number) {
	const first = Math.max(0, Math.floor(offset / size) - OVERSCAN);
	const last = Math.min(count, Math.ceil((offset + extent) / size) + OVERSCAN);
	return [first, last] as const;
}

/**
 * A spreadsheet of Cues across and fixture attributes down that renders only what is on screen.
 *
 * The body scrolls natively, so touch and wheel scrolling stay smooth; the Cue header and the
 * fixture column follow it by scroll offset, not by re-rendering, and only the visible cells
 * (plus a small overscan) exist in the DOM however large the Cuelist is.
 */
export function TrackSheetGrid({
	rows,
	columns,
	cues,
	presets,
	rowLabel,
	selection,
	onSelect,
}: {
	rows: readonly TrackRow[];
	columns: readonly number[];
	cues: readonly Cue[];
	presets: ReadonlyMap<string, PresetName>;
	rowLabel: (row: TrackRow) => { fixture: string; attribute: string };
	selection: TrackSelection | null;
	onSelect: (selection: TrackSelection) => void;
}) {
	const { body, header, labels, viewport, onScroll } = useSyncedViewport();
	const [firstRow, lastRow] = visibleRange(viewport.top, viewport.height, TRACK_ROW_HEIGHT, rows.length);
	const [firstColumn, lastColumn] = visibleRange(
		viewport.left,
		viewport.width,
		TRACK_COLUMN_WIDTH,
		columns.length,
	);
	const width = columns.length * TRACK_COLUMN_WIDTH;
	const height = rows.length * TRACK_ROW_HEIGHT;
	const visibleColumns = columns.slice(firstColumn, lastColumn).map((cue, offset) => ({
		cue,
		position: firstColumn + offset,
	}));
	const visibleRows = rows.slice(firstRow, lastRow).map((row, offset) => ({
		row,
		position: firstRow + offset,
	}));
	return (
		<div
			className="track-sheet-grid"
			role="grid"
			aria-label="Track sheet"
			aria-rowcount={rows.length + 1}
			aria-colcount={columns.length + 1}
		>
			<div className="track-sheet-corner" role="presentation">
				Fixture · Attribute
			</div>
			<div className="track-sheet-header" ref={header} role="row" aria-rowindex={1}>
				<div style={{ width, height: "100%", position: "relative" }}>
					{visibleColumns.map(({ cue, position }) => (
						<div
							key={cue}
							role="columnheader"
							aria-colindex={position + 2}
							className={`track-sheet-cue${cues[cue].cue_only ? " is-cue-only" : ""}`}
							style={{ left: position * TRACK_COLUMN_WIDTH, width: TRACK_COLUMN_WIDTH }}
							title={cues[cue].name}
						>
							<strong>Cue {cues[cue].number}</strong>
							<small>{cues[cue].cue_only ? "Cue only" : cues[cue].name || " "}</small>
						</div>
					))}
				</div>
			</div>
			<div className="track-sheet-labels" ref={labels} role="presentation">
				<div style={{ height, position: "relative" }}>
					{visibleRows.map(({ row, position }) => {
						const label = rowLabel(row);
						return (
							<div
								key={row.key}
								className="track-sheet-label"
								role="rowheader"
								style={{ top: position * TRACK_ROW_HEIGHT, height: TRACK_ROW_HEIGHT }}
							>
								<strong>{label.fixture}</strong>
								<span>{label.attribute}</span>
							</div>
						);
					})}
				</div>
			</div>
			<div className="track-sheet-body" ref={body} onScroll={onScroll}>
				<div style={{ width, height, position: "relative" }}>
					{visibleRows.map(({ row, position: rowPosition }) =>
						visibleColumns.map(({ cue, position: columnPosition }) => {
							return (
								<TrackSheetCell
									key={`${row.key}:${cue}`}
									row={row}
									cue={cue}
									cueNumber={cues[cue].number}
									rowIndex={rowPosition}
									columnIndex={columnPosition}
									presets={presets}
									label={rowLabel(row)}
									selected={selection?.rowKey === row.key && selection.cue === cue}
									onSelect={onSelect}
								/>
							);
						}),
					)}
				</div>
			</div>
		</div>
	);
}

function TrackSheetCell({
	row,
	cue,
	cueNumber,
	rowIndex,
	columnIndex,
	presets,
	label,
	selected,
	onSelect,
}: {
	row: TrackRow;
	cue: number;
	cueNumber: string;
	rowIndex: number;
	columnIndex: number;
	presets: ReadonlyMap<string, PresetName>;
	label: { fixture: string; attribute: string };
	selected: boolean;
	onSelect: (selection: TrackSelection) => void;
}) {
	const cell = trackCell(row, cue);
	const text =
		cell.state === "empty"
			? ""
			: cell.state === "released"
				? "Release"
				: trackValueText(cell.value, cell.preset, presets);
	const swatch = trackValueSwatch(cell.value);
	return (
		<Button
			variant="ghost"
			contentAlign="left"
			role="gridcell"
			aria-rowindex={rowIndex + 2}
			aria-colindex={columnIndex + 2}
			aria-selected={selected}
			aria-label={`${label.fixture} ${label.attribute}, Cue ${cueNumber}: ${TRACK_STATE_LABELS[cell.state]}${text ? ` ${text}` : ""}${cell.cueOnly ? ", Cue only" : ""}`}
			data-state={cell.state}
			className={`track-sheet-cell${cell.cueOnly ? " is-cue-only" : ""}${cell.groupId ? " from-group" : ""}${selected ? " is-selected" : ""}`}
			style={{
				top: rowIndex * TRACK_ROW_HEIGHT,
				left: columnIndex * TRACK_COLUMN_WIDTH,
				width: TRACK_COLUMN_WIDTH,
				height: TRACK_ROW_HEIGHT,
			}}
			onClick={() => onSelect({ rowKey: row.key, cue })}
		>
			{swatch && <i className="track-sheet-swatch" style={{ background: swatch }} />}
			<span className="track-sheet-value">{text}</span>
			{TRACK_STATE_MARKS[cell.state] && (
				<span className="track-sheet-mark" aria-hidden="true">
					{TRACK_STATE_MARKS[cell.state]}
				</span>
			)}
		</Button>
	);
}

/** The body's scroll window, with the Cue header and fixture column following its offsets. */
function useSyncedViewport() {
	const body = useRef<HTMLDivElement>(null);
	const header = useRef<HTMLDivElement>(null);
	const labels = useRef<HTMLDivElement>(null);
	const [viewport, setViewport] = useState({ top: 0, left: 0, width: 800, height: 600 });
	useLayoutEffect(() => {
		const element = body.current;
		if (!element) return;
		const measure = () =>
			setViewport((current) => ({
				...current,
				width: element.clientWidth,
				height: element.clientHeight,
			}));
		measure();
		if (typeof ResizeObserver === "undefined") return;
		const observer = new ResizeObserver(measure);
		observer.observe(element);
		return () => observer.disconnect();
	}, []);
	const onScroll = () => {
		const element = body.current;
		if (!element) return;
		if (header.current) header.current.scrollLeft = element.scrollLeft;
		if (labels.current) labels.current.scrollTop = element.scrollTop;
		setViewport((current) =>
			current.top === element.scrollTop && current.left === element.scrollLeft
				? current
				: { ...current, top: element.scrollTop, left: element.scrollLeft },
		);
	};
	return { body, header, labels, viewport, onScroll };
}
