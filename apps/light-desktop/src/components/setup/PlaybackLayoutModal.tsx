import {
	Button,
	FormLayout,
	ModalRegistration,
	ModalTitleBar,
	NumberField,
	SelectField,
} from "@tosklight/ui";
import { WindowScrollArea } from "@tosklight/ui/window-kit";
import {
	type PointerEvent as ReactPointerEvent,
	type RefObject,
	useRef,
	useState,
} from "react";
import type {
	PlaybackSurfaceLayout,
	PlaybackPageSource,
} from "../../api/types";

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
export type SurfaceOption = { id: string; name: string };

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
				min="1"
				max={maxFirst}
				value={row.first_position}
				onChange={(event) =>
					onUpdate({ first_position: Number(event.target.value) })
				}
			/>
			<NumberField
				label="Playbacks in row"
				min="1"
				max="1000"
				value={row.count}
				onChange={(event) => onUpdate({ count: Number(event.target.value) })}
			/>
			<SelectField
				label="Presentation"
				value={row.presentation}
				options={[
					{ value: "fader", label: "Faders" },
					{ value: "buttons", label: "Buttons" },
					{ value: "push_encoder", label: "Push encoders" },
				]}
				onChange={(value) =>
					onUpdate({ presentation: value as PlaybackRow["presentation"] })
				}
			/>
			<NumberField
				label="Buttons"
				min="0"
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

const MAX_PLAYBACK_ROWS = 1000;
export function playbackLayoutInvalid(layout: PlaybackSurfaceLayout) {
	return (
		layout.rows.length === 0 ||
		layout.rows.length > MAX_PLAYBACK_ROWS ||
		layout.rows.some(
			(row) =>
				!Number.isInteger(row.first_position) ||
				!Number.isInteger(row.count) ||
				!Number.isInteger(row.button_count) ||
				row.first_position < 1 ||
				row.count < 1 ||
				row.first_position + row.count - 1 > 1000 ||
				row.button_count < 0 ||
				row.button_count > 3 ||
				!["fader", "buttons", "push_encoder"].includes(row.presentation),
		)
	);
}
export function canAddPlaybackRow(layout: PlaybackSurfaceLayout) {
	const previous = layout.rows.at(-1);
	return (
		layout.rows.length < MAX_PLAYBACK_ROWS &&
		(!previous || previous.first_position + previous.count * 2 - 1 <= 1000)
	);
}
export function addPlaybackRow(
	current: PlaybackSurfaceLayout,
): PlaybackSurfaceLayout {
	if (!canAddPlaybackRow(current)) return current;
	const previous = current.rows.at(-1);
	return {
		rows: [
			...current.rows,
			{
				first_position: previous ? previous.first_position + previous.count : 1,
				count: previous?.count ?? 10,
				presentation: previous?.presentation ?? "fader",
				button_count: previous?.button_count ?? 3,
			},
		],
	};
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
	pageSource,
	pageSourceLocked = false,
	onPageSource,
	scrollRows = false,
	surfaceOptions = [],
}: {
	layout: PlaybackSurfaceLayout;
	onLayout: LayoutUpdate;
	pageSource: PlaybackPageSource;
	pageSourceLocked?: boolean;
	onPageSource: (mode: PlaybackPageSource) => void;
	/** The modal scrolls its rows; an embedded form scrolls with its host. */
	scrollRows?: boolean;
	surfaceOptions?: SurfaceOption[];
}) {
	const dragRow = useRef<DraggedPlaybackRow | null>(null);
	const maxFirst = 1000;
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
				<SelectField
					label="Page source"
					value={pageSource.type}
					disabled={pageSourceLocked}
					options={[
						{ value: "own", label: "Own page" },
						{ value: "pinned", label: "Pinned page" },
						{ value: "follow", label: "Follow surface" },
					]}
					onChange={(value) =>
						onPageSource(
							value === "follow"
								? {
										type: "follow",
										surface_id: surfaceOptions[0]?.id ?? "main",
									}
								: {
										type: value as "own" | "pinned",
										page: pageSource.type === "follow" ? 1 : pageSource.page,
									},
						)
					}
				/>
				{pageSource.type === "follow" ? (
					<SelectField
						label="Follow surface"
						value={pageSource.surface_id}
						disabled={pageSourceLocked}
						options={surfaceOptions.map((surface) => ({
							value: surface.id,
							label: surface.name,
						}))}
						onChange={(surface_id) =>
							onPageSource({ type: "follow", surface_id })
						}
					/>
				) : (
					<NumberField
						label="Page"
						min="1"
						max="99"
						value={pageSource.page}
						disabled={pageSourceLocked}
						onChange={(event) =>
							onPageSource({ ...pageSource, page: Number(event.target.value) })
						}
					/>
				)}
			</FormLayout>
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
	pageSource,
	pageSourceLocked = false,
	surfaceOptions = [],
	onSave,
	onClose,
}: {
	initialLayout: PlaybackSurfaceLayout;
	pageSource: PlaybackPageSource;
	pageSourceLocked?: boolean;
	surfaceOptions?: SurfaceOption[];
	onSave: (
		layout: PlaybackSurfaceLayout,
		pageSource: PlaybackPageSource,
	) => void;
	onClose: () => void;
}) {
	const [layout, setLayout] = useState(() => structuredClone(initialLayout));
	const [draftPageSource, setDraftPageSource] = useState(pageSource);
	const invalid =
		playbackLayoutInvalid(layout) ||
		(draftPageSource.type !== "follow" &&
			(!Number.isInteger(draftPageSource.page) ||
				draftPageSource.page < 1 ||
				draftPageSource.page > 99));

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
							onPress: () => onSave(layout, draftPageSource),
						}}
						closeLabel="Close playback configuration"
						onClose={onClose}
					/>
					<PlaybackLayoutFields
						layout={layout}
						onLayout={setLayout}
						pageSource={draftPageSource}
						pageSourceLocked={pageSourceLocked}
						surfaceOptions={surfaceOptions}
						onPageSource={setDraftPageSource}
						scrollRows
					/>
				</section>
			</div>
		</ModalRegistration>
	);
}
