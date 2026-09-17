import {
	Button,
	FormLayout,
	ModalRegistration,
	ModalTitleBar,
	NumberField,
	SelectField,
	SwitchField,
} from "@tosklight/ui";
import { WindowScrollArea } from "@tosklight/ui/window-kit";
import {
	type PointerEvent as ReactPointerEvent,
	type RefObject,
	useRef,
	useState,
} from "react";
import type { PlaybackSurfaceLayout } from "../../api/types";

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
export type PlaybackPageMode = "follow_main" | "independent";

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
				offLabel="Buttons only"
				onLabel="With fader"
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

const MAX_PLAYBACK_ROWS = 127;

/** Whether a draft layout can be stored; an invalid draft stays on screen unsaved. */
export function playbackLayoutInvalid(layout: PlaybackSurfaceLayout) {
	const maxFirst = 128 - layout.playbacks_per_row;
	return (
		layout.playbacks_per_row < 1 ||
		layout.playbacks_per_row > 32 ||
		layout.rows.length === 0 ||
		layout.rows.length > MAX_PLAYBACK_ROWS ||
		layout.playbacks_per_row * layout.rows.length > 127 ||
		layout.rows.some(
			(row) =>
				row.first_playback_slot < 1 ||
				row.first_playback_slot > maxFirst ||
				row.button_count < 0 ||
				row.button_count > 3,
		)
	);
}

/** Adds a row after the last one, continuing its playback numbering. */
export function addPlaybackRow(
	current: PlaybackSurfaceLayout,
): PlaybackSurfaceLayout {
	if (current.rows.length >= MAX_PLAYBACK_ROWS) return current;
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
}

export function canAddPlaybackRow(layout: PlaybackSurfaceLayout) {
	return layout.rows.length < MAX_PLAYBACK_ROWS;
}

type LayoutUpdate = (
	change: (current: PlaybackSurfaceLayout) => PlaybackSurfaceLayout,
) => void;

/**
 * The playback layout form: playbacks per row, page mode, and one form row per playback row.
 * The Configure Playbacks modal and the Playbacks tab of Configure Screen both render it.
 */
export function PlaybackLayoutFields({
	layout,
	onLayout,
	pageMode,
	pageModeLocked = false,
	onPageMode,
	scrollRows = false,
}: {
	layout: PlaybackSurfaceLayout;
	onLayout: LayoutUpdate;
	pageMode: PlaybackPageMode;
	pageModeLocked?: boolean;
	onPageMode: (mode: PlaybackPageMode) => void;
	/** The modal scrolls its rows; an embedded form scrolls with its host. */
	scrollRows?: boolean;
}) {
	const dragRow = useRef<DraggedPlaybackRow | null>(null);
	const maxFirst = 128 - layout.playbacks_per_row;
	const rows = layout.rows.map((row, index) => (
		<PlaybackRowConfiguration
			key={index}
			row={row}
			index={index}
			rowCount={layout.rows.length}
			maxFirst={maxFirst}
			dragRow={dragRow}
			onMove={(from, to) => {
				if (from === to) return;
				onLayout((current) => ({
					...current,
					rows: reorderPlaybackRows(current.rows, from, to),
				}));
			}}
			onUpdate={(changes) =>
				onLayout((current) => ({
					...current,
					rows: current.rows.map((candidate, rowIndex) =>
						rowIndex === index ? { ...candidate, ...changes } : candidate,
					),
				}))
			}
			onRemove={() =>
				onLayout((current) => ({
					...current,
					rows: current.rows.filter((_, rowIndex) => rowIndex !== index),
				}))
			}
		/>
	));
	return (
		<>
			<FormLayout columns={2} minColumnWidth={190}>
				<NumberField
					label="Playbacks per row"
					min="1"
					max="32"
					value={layout.playbacks_per_row}
					onChange={(event) => {
						const playbacks_per_row = Number(event.target.value);
						onLayout((current) => ({ ...current, playbacks_per_row }));
					}}
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
			{scrollRows ? (
				<WindowScrollArea className="playback-row-list">
					{rows}
				</WindowScrollArea>
			) : (
				rows
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
	const invalid = playbackLayoutInvalid(layout);

	return (
		<ModalRegistration onClose={onClose}>
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
					<ModalTitleBar
						title="Configure Playbacks"
						groups={[
							{
								id: "layout",
								actions: [
									{
										id: "add-row",
										label: "Add Row",
										disabled: !canAddPlaybackRow(layout),
										onPress: () => setLayout(addPlaybackRow),
									},
								],
							},
						]}
						accept={{
							id: "save",
							label: "Save",
							className: "playback-layout-save",
							variant: "primary",
							disabled: invalid,
							onPress: () => onSave(layout, draftPageMode),
						}}
						closeLabel="Close playback configuration"
						onClose={onClose}
					/>
					<PlaybackLayoutFields
						layout={layout}
						onLayout={setLayout}
						pageMode={draftPageMode}
						pageModeLocked={pageModeLocked}
						onPageMode={setDraftPageMode}
						scrollRows
					/>
				</section>
			</div>
		</ModalRegistration>
	);
}
