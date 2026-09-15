import { open } from "@tauri-apps/plugin-dialog";
import { Button, SwitchField } from "@tosklight/ui";
import { useEffect, useRef, useState } from "react";
import { CommitNumber } from "./cadFields";
import {
	CAD_VIEW_LABELS,
	type CadViewDirection,
	type WorldAxis,
	viewAxes,
} from "./types";
import type { CadUnderlay, CadUnderlayPreview } from "./underlays";
import { underlaySession } from "./underlays";
import type { CadUnderlays } from "./useCadUnderlays";

const DRAWING_FILTER = [{ name: "Drawing", extensions: ["dxf", "svg"] }];

const VIEWS: CadViewDirection[] = [
	"top_down",
	"front_to_back",
	"back_to_front",
	"left_to_right",
	"right_to_left",
];

/** The pair of world axes a view shows, for the panel to name what a drawing is placed against. */
function axisLabel(view: CadViewDirection): string {
	const axes = viewAxes(view);
	const name = (axis: WorldAxis) => axis.toUpperCase();
	return `${name(axes.horizontal.axis)}/${name(axes.vertical.axis)}`;
}

function metres(millimetres: number): number {
	return Math.round(millimetres) / 1000;
}

/** What a chosen file holds, shown before anything is written into the show. */
function PlacementPreview({
	preview,
	view,
	busy,
	onView,
	onPlace,
	onCancel,
}: {
	preview: CadUnderlayPreview;
	view: CadViewDirection;
	busy: boolean;
	onView(view: CadViewDirection): void;
	onPlace(): void;
	onCancel(): void;
}) {
	const [, , maxX, maxY] = preview.extentsMillimetres;
	const [minX, minY] = preview.extentsMillimetres;
	return (
		<section className="cad-underlay-preview" aria-label="Place drawing">
			<h3>{preview.name}</h3>
			<dl>
				<dt>Drawn in</dt>
				<dd>{preview.units}</dd>
				<dt>Size</dt>
				<dd>
					{metres(maxX - minX)} × {metres(maxY - minY)} m
				</dd>
				<dt>Lines</dt>
				<dd>{preview.polylineCount.toLocaleString()}</dd>
			</dl>
			<label>
				<span>Axis</span>
				<select
					aria-label="Axis"
					value={view}
					onChange={(event) => onView(event.currentTarget.value as CadViewDirection)}
				>
					{VIEWS.map((candidate) => (
						<option key={candidate} value={candidate}>
							{CAD_VIEW_LABELS[candidate]} ({axisLabel(candidate)})
						</option>
					))}
				</select>
			</label>
			<div className="cad-underlay-actions">
				<Button disabled={busy} onClick={onPlace}>
					{busy ? "Placing…" : "Place Drawing"}
				</Button>
				<Button disabled={busy} onClick={onCancel}>
					Cancel
				</Button>
			</div>
		</section>
	);
}

/** One placed drawing: where it sits, how big it is, and whether the views draw it. */
function UnderlayRow({
	underlay,
	busy,
	onChange,
	onRemove,
}: {
	underlay: CadUnderlay;
	busy: boolean;
	onChange(next: CadUnderlay): void;
	onRemove(): void;
}) {
	return (
		<div className="cad-underlay-row">
			<header>
				<strong>{underlay.name}</strong>
				<small>
					{CAD_VIEW_LABELS[underlay.view]} · {underlay.sourceFormat.toUpperCase()}
				</small>
			</header>
			<SwitchField
				label="Show"
				offLabel={null}
				onLabel={null}
				checked={underlay.visible}
				onChange={(event) =>
					onChange({ ...underlay, visible: event.currentTarget.checked })
				}
			/>
			<CommitNumber
				label="X (m)"
				value={metres(underlay.originMillimetres[0])}
				onCommit={(x) =>
					onChange({ ...underlay, originMillimetres: [x * 1000, underlay.originMillimetres[1]] })
				}
			/>
			<CommitNumber
				label="Y (m)"
				value={metres(underlay.originMillimetres[1])}
				onCommit={(y) =>
					onChange({ ...underlay, originMillimetres: [underlay.originMillimetres[0], y * 1000] })
				}
			/>
			<CommitNumber
				label="Scale"
				min={0.001}
				value={underlay.scale}
				onCommit={(scale) => onChange({ ...underlay, scale })}
			/>
			<CommitNumber
				label="Rotation (°)"
				digits={1}
				value={underlay.rotationDegrees}
				onCommit={(rotationDegrees) => onChange({ ...underlay, rotationDegrees })}
			/>
			<Button
				className="cad-underlay-remove"
				disabled={busy}
				aria-label={`Remove ${underlay.name}`}
				onClick={onRemove}
			>
				Remove
			</Button>
		</div>
	);
}

/**
 * The drawings placed under the plan: a ground plan on the top-down view, a section on an
 * elevation. Placement is in metres of the show, so a drawing lines up with the rig rather than
 * with whatever origin its CAD file happened to use.
 */
export function CadUnderlayPanel({
	state,
	defaultView,
	only,
	chooseRequest = 0,
}: {
	state: CadUnderlays;
	defaultView: CadViewDirection;
	/**
	 * Where a tree lists the drawings, the panel edits only the one chosen there: its ID, or null
	 * when none is. Absent, the panel lists every drawing itself.
	 */
	only?: string | null;
	/** Counts presses of the side panel's Import drawing; each opens the file chooser once. */
	chooseRequest?: number;
}) {
	const [pending, setPending] = useState<{
		path: string;
		preview: CadUnderlayPreview;
	} | null>(null);
	const [view, setView] = useState<CadViewDirection>(defaultView);
	const [reading, setReading] = useState(false);
	const [readError, setReadError] = useState<string | null>(null);
	const handledChoose = useRef(chooseRequest);
	useEffect(() => {
		if (chooseRequest === handledChoose.current) return;
		handledChoose.current = chooseRequest;
		void choose();
	});

	async function choose() {
		const path = await open({ filters: DRAWING_FILTER, multiple: false });
		if (typeof path !== "string") return;
		setReading(true);
		setReadError(null);
		try {
			setPending({ path, preview: await underlaySession.preview(path) });
			setView(defaultView);
		} catch (reason) {
			setReadError(String(reason));
			setPending(null);
		} finally {
			setReading(false);
		}
	}

	async function place() {
		if (!pending) return;
		await state.place(pending.path, view);
		setPending(null);
	}

	const listed =
		only === undefined
			? state.underlays
			: state.underlays.filter((underlay) => underlay.id === only);

	return (
		<div className="cad-underlay-panel">
			{only === undefined ? (
				<Button
					className="cad-add-underlay"
					disabled={reading || state.busy}
					onClick={() => void choose()}
				>
					{reading ? "Reading drawing…" : "Add Drawing"}
				</Button>
			) : reading ? (
				<p role="status">Reading drawing…</p>
			) : null}
			{readError ? <output className="cad-error">{readError}</output> : null}
			{state.error ? <output className="cad-error">{state.error}</output> : null}
			{pending ? (
				<PlacementPreview
					preview={pending.preview}
					view={view}
					busy={state.busy}
					onView={setView}
					onPlace={() => void place()}
					onCancel={() => setPending(null)}
				/>
			) : null}
			<div className="cad-underlay-list">
				{listed.length ? (
					listed.map((underlay) => (
						<UnderlayRow
							key={underlay.id}
							underlay={underlay}
							busy={state.busy}
							onChange={(next) => void state.change(next)}
							onRemove={() => void state.remove(underlay.id)}
						/>
					))
				) : only === undefined ? (
					<p>
						Place a DXF or SVG of the venue and the plan is drawn under the rig.
					</p>
				) : null}
			</div>
		</div>
	);
}
