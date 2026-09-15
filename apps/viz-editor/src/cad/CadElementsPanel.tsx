/**
 * The CAD window's Elements panel: what the venue is made of, apart from the lamps.
 *
 * **Drawings** lists every placed DXF or SVG and every line, box, text and measurement drawn on a
 * view, in folders the operator arranges freely. **Objects** lists the placed Venue items and 3D
 * models, adds more, and places the chosen one.
 */
import { open } from "@tauri-apps/plugin-dialog";
import { Button, NumberField, TitleChrome } from "@tosklight/ui";
import { useState } from "react";
import { documentSession } from "../document/session";
import type { CadAnnotation } from "./annotations";
import { formatMeasurement, measurementLength } from "./annotationGeometry";
import { CadDrawingTree } from "./CadDrawingTree";
import { CAD_ADD_ACTIONS } from "./CadToolbar";
import { CadUnderlayPanel } from "./CadUnderlayPanel";
import type { CadTools } from "./cadTools";
import type { DrawingLeaf } from "./drawingTree";
import { CAD_VIEW_LABELS, type CadEntity, type CadViewDirection } from "./types";
import type { CadUnderlay } from "./underlays";
import type { CadUnderlays } from "./useCadUnderlays";
import { useCadDrawingTree } from "./useCadDrawingTree";
import { VENUE_MODEL_EXTENSIONS } from "./venueModelFormats";
import "./cadElements.css";

type ElementsTab = "drawings" | "objects";

const IMPORTING = "Importing 3D model…";

function annotationName(annotation: CadAnnotation): string {
	switch (annotation.kind) {
		case "polyline":
			return annotation.closed ? "Closed line" : "Line";
		case "box":
			return "Box";
		case "text":
			return `Text “${annotation.text}”`;
		case "measure":
			return `Measure ${formatMeasurement(measurementLength(annotation))}`;
	}
}

/** Every drawing the tree can arrange: placed files first, then drawn items. */
export function drawingLeaves(
	underlays: readonly CadUnderlay[],
	annotations: readonly CadAnnotation[],
): DrawingLeaf[] {
	return [
		...underlays.map(
			(underlay): DrawingLeaf => ({
				id: underlay.id,
				kind: "underlay",
				name: underlay.name,
				detail: `${CAD_VIEW_LABELS[underlay.view]} · ${underlay.sourceFormat.toUpperCase()}`,
			}),
		),
		...annotations.map(
			(annotation): DrawingLeaf => ({
				id: annotation.id,
				kind: "annotation",
				name: annotationName(annotation),
				detail: CAD_VIEW_LABELS[annotation.view],
			}),
		),
	];
}

function metres(millimetres: number) {
	return Math.round(millimetres) / 1000;
}

/** One entry per placed object: multi-patch copies share their logical fixture. */
function venueObjects(entities: readonly CadEntity[]) {
	const seen = new Set<string>();
	return entities.filter((entity) => {
		if (entity.kind !== "venue" || seen.has(entity.logicalFixtureId)) return false;
		seen.add(entity.logicalFixtureId);
		return true;
	});
}

function DrawingsTab({
	documentKey,
	underlayState,
	tools,
	defaultView,
}: {
	documentKey: string | null;
	underlayState: CadUnderlays;
	tools: CadTools;
	defaultView: CadViewDirection;
}) {
	const { tree, change, error } = useCadDrawingTree(documentKey);
	const [selected, setSelected] = useState<{ id: string; isFolder: boolean } | null>(null);
	const leaves = drawingLeaves(underlayState.underlays, tools.annotations);
	const leaf = selected && !selected.isFolder
		? leaves.find((candidate) => candidate.id === selected.id)
		: undefined;
	return (
		<CadDrawingTree
			tree={tree}
			leaves={leaves}
			selected={selected}
			onSelect={setSelected}
			onChange={change}
		>
			{error ? <output className="cad-error">{error}</output> : null}
			<CadUnderlayPanel
				state={underlayState}
				defaultView={defaultView}
				only={leaf?.kind === "underlay" ? leaf.id : null}
			/>
			{leaf?.kind === "annotation" ? (
				<section className="cad-elements-selected" aria-label="Selected drawing">
					<strong>{leaf.name}</strong>
					<small>{leaf.detail}</small>
					<Button
						onClick={() => {
							void tools.remove(leaf.id);
							setSelected(null);
						}}
					>
						Erase
					</Button>
				</section>
			) : null}
		</CadDrawingTree>
	);
}

function ObjectRow({
	entity,
	selected,
	onSelect,
}: {
	entity: CadEntity;
	selected: boolean;
	onSelect(): void;
}) {
	const [width, depth, height] = entity.sizeMillimetres.map(metres);
	return (
		<button
			type="button"
			className={`cad-elements-object ${selected ? "is-selected" : ""}`.trim()}
			aria-pressed={selected}
			onClick={onSelect}
		>
			<strong>{entity.name}</strong>
			<small>
				{entity.fixtureDisplayId} · {entity.scenery?.kind ?? entity.fixtureProfile ?? "model"} ·{" "}
				{width} × {depth} × {height} m
			</small>
		</button>
	);
}

/** Where the chosen object stands, in metres; a change moves it like dragging it in a view. */
function ObjectPlacement({
	entity,
	onMove,
}: {
	entity: CadEntity;
	onMove(delta: [number, number, number]): void;
}) {
	return (
		<section className="cad-elements-selected" aria-label="Selected object">
			<strong>{entity.name}</strong>
			{(["X", "Y", "Z"] as const).map((axis, index) => (
				<NumberField
					key={axis}
					label={`${axis} (m)`}
					step={0.01}
					value={metres(entity.positionMillimetres[index])}
					onChange={(event) => {
						const target = Number(event.currentTarget.value) * 1000;
						if (!Number.isFinite(target)) return;
						const delta: [number, number, number] = [0, 0, 0];
						delta[index] = target - entity.positionMillimetres[index];
						if (Math.abs(delta[index]) >= 1) onMove(delta);
					}}
				/>
			))}
		</section>
	);
}

function ObjectsTab({
	entities,
	selectedIds,
	tools,
	onSelect,
	onMove,
}: {
	entities: readonly CadEntity[];
	selectedIds: readonly string[];
	tools: CadTools;
	onSelect(ids: string[]): void;
	onMove(delta: [number, number, number], ids: readonly string[]): void;
}) {
	const [status, setStatus] = useState("");
	const objects = venueObjects(entities);
	const items = objects.filter((entity) => entity.scenery);
	const models = objects.filter((entity) => !entity.scenery);
	const chosen =
		selectedIds.length === 1
			? objects.find((entity) => entity.logicalFixtureId === selectedIds[0])
			: undefined;

	async function importModel() {
		const path = await open({
			multiple: false,
			directory: false,
			filters: [{ name: "3D model", extensions: [...VENUE_MODEL_EXTENSIONS] }],
		});
		if (typeof path !== "string") return;
		setStatus(IMPORTING);
		try {
			const imported = await documentSession.importVenueModel(path, null);
			onSelect([imported.fixtureId]);
			setStatus(`${imported.name} placed at the stage origin.`);
		} catch (reason) {
			setStatus(`Could not import the 3D model: ${String(reason)}`);
		}
	}

	const list = (title: string, shown: CadEntity[], empty: string) => (
		<section className="cad-elements-group" aria-label={title}>
			<h3>{title}</h3>
			{shown.length ? (
				shown.map((entity) => (
					<ObjectRow
						key={entity.logicalFixtureId}
						entity={entity}
						selected={selectedIds.includes(entity.logicalFixtureId)}
						onSelect={() => onSelect([entity.logicalFixtureId])}
					/>
				))
			) : (
				<p>{empty}</p>
			)}
		</section>
	);

	return (
		<div className="cad-elements-objects">
			<div className="cad-elements-actions">
				{CAD_ADD_ACTIONS.map(({ kind, label }) => (
					<Button
						key={kind}
						disabled={!tools.onAdd}
						onClick={() => tools.onAdd?.(kind)}
					>
						{label}
					</Button>
				))}
				<Button disabled={status === IMPORTING} onClick={() => void importModel()}>
					{status === IMPORTING ? IMPORTING : "Import 3D model"}
				</Button>
			</div>
			{status && status !== IMPORTING ? (
				<p className="cad-elements-status" role="status">
					{status}
				</p>
			) : null}
			{list("Venue items", items, "No trusses, stage elements or curtains placed yet.")}
			{list("3D models", models, "No 3D models placed yet.")}
			{chosen ? (
				<ObjectPlacement
					entity={chosen}
					onMove={(delta) => onMove(delta, [chosen.logicalFixtureId])}
				/>
			) : null}
		</div>
	);
}

export function CadElementsPanel({
	documentKey,
	underlayState,
	tools,
	defaultView,
	entities,
	selectedIds,
	onSelect,
	onMove,
}: {
	documentKey: string | null;
	underlayState: CadUnderlays;
	tools: CadTools;
	defaultView: CadViewDirection;
	entities: readonly CadEntity[];
	selectedIds: readonly string[];
	onSelect(ids: string[]): void;
	onMove(delta: [number, number, number], ids: readonly string[]): void;
}) {
	const [tab, setTab] = useState<ElementsTab>("drawings");
	return (
		<div className="cad-elements-panel">
			<TitleChrome
				className="ui-window-action-groups cad-elements-tabs"
				groupClassName="ui-window-action-group"
				terminalActions={[]}
				groups={[
					{
						id: "elements-tabs",
						kind: "tabs",
						activeId: tab,
						onActiveChange: (id) => setTab(id as ElementsTab),
						actions: [
							{ id: "drawings", label: "Drawings" },
							{ id: "objects", label: "Objects" },
						],
					},
				]}
			/>
			{tab === "drawings" ? (
				<DrawingsTab
					documentKey={documentKey}
					underlayState={underlayState}
					tools={tools}
					defaultView={defaultView}
				/>
			) : (
				<ObjectsTab
					entities={entities}
					selectedIds={selectedIds}
					tools={tools}
					onSelect={onSelect}
					onMove={onMove}
				/>
			)}
		</div>
	);
}
