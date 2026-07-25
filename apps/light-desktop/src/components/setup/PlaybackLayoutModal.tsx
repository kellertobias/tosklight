import {
	type PointerEvent as ReactPointerEvent,
	type RefObject,
	useRef,
	useState,
} from "react";
import type { PlaybackSurfaceLayout } from "../../api/types";
import {
	Button,
	FormLayout,
	ModalTitleBar,
	NumberField,
	SelectField,
	SwitchField,
} from "../common";
import { WindowScrollArea } from "../window-kit";

export function reorderPlaybackRows(
	rows: PlaybackSurfaceLayout["rows"],
	from: number,
	to: number,
) {
	if (
		from === to ||
		from < 0 ||
		to < 0 ||
		from >= rows.length ||
		to >= rows.length
	)
		return rows;
	const next = [...rows];
	const [row] = next.splice(from, 1);
	next.splice(to, 0, row);
	return next;
}

type PlaybackRow = PlaybackSurfaceLayout["rows"][number];
type DraggedPlaybackRow = { pointerId: number; from: number };
type PlaybackPageMode = "follow_main" | "independent";

function releaseRowDrag(
	event: ReactPointerEvent<HTMLButtonElement>,
	dragRow: RefObject<DraggedPlaybackRow | null>,
) {
	dragRow.current = null;
	if (event.currentTarget.hasPointerCapture?.(event.pointerId)) {
		event.currentTarget.releasePointerCapture(event.pointerId);
	}
}

function PlaybackRowConfiguration({
	row,
	index,
	rowCount,
	maxFirst,
	dragRow,
	onMove,
	onUpdate,
	onRemove,
}: {
	row: PlaybackRow;
	index: number;
	rowCount: number;
	maxFirst: number;
	dragRow: RefObject<DraggedPlaybackRow | null>;
	onMove: (from: number, to: number) => void;
	onUpdate: (changes: Partial<PlaybackRow>) => void;
	onRemove: () => void;
}) {
	return (
		<article
			className="playback-row-configuration"
			data-playback-row-index={index}
		>
			<Button
				className="playback-row-drag"
				aria-label={`Reorder playback row ${index + 1}`}
				title="Drag to reorder"
				onPointerDown={(event) => {
					event.preventDefault();
					dragRow.current = { pointerId: event.pointerId, from: index };
					event.currentTarget.setPointerCapture?.(event.pointerId);
				}}
				onPointerMove={(event) => {
					const active = dragRow.current;
					if (!active || active.pointerId !== event.pointerId) return;
					const target = document
						.elementFromPoint(event.clientX, event.clientY)
						?.closest<HTMLElement>("[data-playback-row-index]");
					const to = Number(target?.dataset.playbackRowIndex);
					if (!Number.isInteger(to) || to === active.from) return;
					onMove(active.from, to);
					dragRow.current = { ...active, from: to };
				}}
				onPointerUp={(event) => releaseRowDrag(event, dragRow)}
				onPointerCancel={(event) => releaseRowDrag(event, dragRow)}
			>
				<span aria-hidden="true">⠿</span>
			</Button>
			<NumberField
				label="First Playback Number"
				min="0"
				max={maxFirst}
				value={row.first_playback_slot}
				onChange={(event) =>
					onUpdate({ first_playback_slot: Number(event.target.value) })
				}
			/>
			<SwitchField
				label="Fader"
				checked={row.has_fader}
				onChange={(event) => onUpdate({ has_fader: event.target.checked })}
			/>
			<NumberField
				label="Buttons"
				min="1"
				max="3"
				value={row.button_count}
				onChange={(event) =>
					onUpdate({ button_count: Number(event.target.value) })
				}
			/>
			<Button
				className="playback-row-remove"
				variant="danger"
				iconOnly
				aria-label={`Remove row ${index + 1}`}
				title={`Remove row ${index + 1}`}
				disabled={rowCount === 1}
				onClick={onRemove}
			>
				<svg aria-hidden="true" viewBox="0 0 24 24">
					<path d="M4 7h16M9 7V4h6v3m-9 0 1 13h10l1-13M10 11v5m4-5v5" />
				</svg>
			</Button>
		</article>
	);
}

function PlaybackLayoutControls({
	columns,
	rowCount,
	pageMode,
	pageModeLocked,
	invalid,
	onColumns,
	onPageMode,
	onAddRow,
	onSave,
	onClose,
}: {
	columns: number;
	rowCount: number;
	pageMode: PlaybackPageMode;
	pageModeLocked: boolean;
	invalid: boolean;
	onColumns: (columns: number) => void;
	onPageMode: (mode: PlaybackPageMode) => void;
	onAddRow: () => void;
	onSave: () => void;
	onClose: () => void;
}) {
	return (
		<>
			<ModalTitleBar
				title="Configure Playbacks"
				actions={
					<>
						<Button disabled={rowCount >= 127} onClick={onAddRow}>
							Add Row
						</Button>
						<Button
							className="playback-layout-save"
							variant="primary"
							disabled={invalid}
							onClick={onSave}
						>
							Save
						</Button>
					</>
				}
				closeLabel="Close playback configuration"
				onClose={onClose}
			/>
			<FormLayout columns={2} minColumnWidth={190}>
				<NumberField
					label="Playbacks per row"
					min="1"
					max="32"
					value={columns}
					onChange={(event) => onColumns(Number(event.target.value))}
				/>
				<SelectField
					label="Page Mode"
					value={pageMode}
					disabled={pageModeLocked}
					onChange={(value) => onPageMode(value as PlaybackPageMode)}
					options={
						pageModeLocked
							? [{ value: "follow_main", label: "Main Page" }]
							: [
									{ value: "follow_main", label: "Follow Main" },
									{ value: "independent", label: "Dedicated Page" },
								]
					}
				/>
			</FormLayout>
			{pageModeLocked && (
				<small className="playback-page-mode-note">
					The default screen owns the main playback page.
				</small>
			)}
		</>
	);
}

export function PlaybackLayoutModal({
	initialLayout,
	pageMode,
	pageModeLocked = false,
	onSave,
	onClose,
}: {
	initialLayout: PlaybackSurfaceLayout;
	pageMode: PlaybackPageMode;
	pageModeLocked?: boolean;
	onSave: (layout: PlaybackSurfaceLayout, pageMode: PlaybackPageMode) => void;
	onClose: () => void;
}) {
	const [layout, setLayout] = useState(() => structuredClone(initialLayout));
	const [draftPageMode, setDraftPageMode] = useState(pageMode);
	const dragRow = useRef<DraggedPlaybackRow | null>(null);
	const maxRows = 127;
	const maxFirst = 128 - layout.playbacks_per_row;
	const invalid =
		layout.playbacks_per_row < 1 ||
		layout.playbacks_per_row > 32 ||
		layout.rows.length === 0 ||
		layout.rows.length > maxRows ||
		layout.playbacks_per_row * layout.rows.length > 127 ||
		layout.rows.some(
			(row) =>
				row.first_playback_slot < 1 ||
				row.first_playback_slot > maxFirst ||
				row.button_count < 0 ||
				row.button_count > 3,
		);
	const updateRow = (
		index: number,
		changes: Partial<PlaybackSurfaceLayout["rows"][number]>,
	) =>
		setLayout((current) => ({
			...current,
			rows: current.rows.map((row, rowIndex) =>
				rowIndex === index ? { ...row, ...changes } : row,
			),
		}));
	const addRow = () =>
		setLayout((current) => {
			const previous = current.rows.at(-1);
			return {
				...current,
				rows: [
					...current.rows,
					{
						first_playback_slot: Math.min(
							128 - current.playbacks_per_row,
							(previous?.first_playback_slot ?? 1) + current.playbacks_per_row,
						),
						has_fader: true,
						button_count: 3,
					},
				],
			};
		});
	const moveRow = (from: number, to: number) => {
		if (from === to) return;
		setLayout((current) => ({
			...current,
			rows: reorderPlaybackRows(current.rows, from, to),
		}));
	};

	return (
		<div
			className="stacked-modal-layer"
			onPointerDown={(event) =>
				event.target === event.currentTarget && onClose()
			}
		>
			<section
				className="nested-modal playback-layout-modal"
				role="dialog"
				aria-modal="true"
				aria-label="Configure Playbacks"
			>
				<PlaybackLayoutControls
					columns={layout.playbacks_per_row}
					rowCount={layout.rows.length}
					pageMode={draftPageMode}
					pageModeLocked={pageModeLocked}
					invalid={invalid}
					onColumns={(playbacks_per_row) =>
						setLayout((current) => ({ ...current, playbacks_per_row }))
					}
					onPageMode={setDraftPageMode}
					onAddRow={addRow}
					onSave={() => onSave(layout, draftPageMode)}
					onClose={onClose}
				/>
				<WindowScrollArea className="playback-row-list">
					{layout.rows.map((row, index) => (
						<PlaybackRowConfiguration
							key={index}
							row={row}
							index={index}
							rowCount={layout.rows.length}
							maxFirst={maxFirst}
							dragRow={dragRow}
							onMove={moveRow}
							onUpdate={(changes) => updateRow(index, changes)}
							onRemove={() =>
								setLayout((current) => ({
									...current,
									rows: current.rows.filter(
										(_, rowIndex) => rowIndex !== index,
									),
								}))
							}
						/>
					))}
				</WindowScrollArea>
			</section>
		</div>
	);
}
