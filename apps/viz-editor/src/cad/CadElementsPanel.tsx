/**
 * The CAD window's Elements panel: what the venue is made of, apart from the lamps.
 *
 * **Drawings** lists every placed DXF or SVG and every line, box, text and measurement drawn on a
 * view, in folders the operator arranges freely. **Objects** lists the placed Venue items and 3D
 * models. The tabs and the **+** that adds to the open tab live in the side panel's title row, so
 * this panel receives which tab is open and each add as a request.
 */
import { open } from "@tauri-apps/plugin-dialog";
import type { TitleDropdownItem } from "@tosklight/ui";
import { useEffect, useRef, useState } from "react";
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

export type ElementsTab = "drawings" | "objects";

/** Each add the title row's **+** asks for, as a counter the panel answers once per press. */
export interface ElementsRequests {
	newFolder: number;
	chooseDrawing: number;
	importModel: number;
}

const IMPORTING = "Importing 3D model…";

/** What the **+** in the title row offers for the open tab. */
export function elementsAddItems(
	tab: ElementsTab,
	tools: CadTools,
	request: (kind: keyof ElementsRequests) => void,
): TitleDropdownItem[] {
	if (tab === "drawings")
		return [
			{
				kind: "action",
				id: "import-drawing",
				label: "Import drawing (DXF, SVG)…",
				onPress: () => request("chooseDrawing"),
			},
			{ kind: "action", id: "new-folder", label: "New folder", onPress: () => request("newFolder") },
		];
	return [
		...CAD_ADD_ACTIONS.map(
			({ kind, label }): TitleDropdownItem => ({
				kind: "action",
				id: `add-${kind}`,
				label: label.replace(/^Add /u, "").replace(/^./u, (first) => first.toUpperCase()),
				disabled: !tools.onAdd,
				onPress: () => tools.onAdd?.(kind),
			}),
		),
		{
			kind: "action",
			id: "import-model",
			label: "Import 3D model…",
			onPress: () => request("importModel"),
		},
	];
}

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

/** Runs `action` once for every press after the first render, never for the count it started with. */
function useRequest(count: number, action: () => void) {
	const handled = useRef(count);
	useEffect(() => {
		if (count === handled.current) return;
		handled.current = count;
		action();
	});
}

function DrawingsTab({
	documentKey,
	underlayState,
	tools,
	defaultView,
	requests,
}: {
	documentKey: string | null;
	underlayState: CadUnderlays;
	tools: CadTools;
	defaultView: CadViewDirection;
	requests: ElementsRequests;
}) {
	const { tree, change, error } = useCadDrawingTree(documentKey);
	const [selected, setSelected] = useState<{ id: string; isFolder: boolean } | null>(null);
	const leaves = drawingLeaves(underlayState.underlays, tools.annotations);
	const leaf =
		selected && !selected.isFolder
			? leaves.find((candidate) => candidate.id === selected.id)
			: undefined;
	return (
		<CadDrawingTree
			tree={tree}
			leaves={leaves}
			selected={selected}
			onSelect={setSelected}
			onChange={change}
			newFolderRequest={requests.newFolder}
		>
			{error ? <output className="cad-error">{error}</output> : null}
			<CadUnderlayPanel
				state={underlayState}
				defaultView={defaultView}
				only={leaf?.kind === "underlay" ? leaf.id : null}
				chooseRequest={requests.chooseDrawing}
			/>
			{leaf?.kind === "annotation" ? (
				<section className="cad-elements-selected" aria-label="Selected drawing">
					<strong>{leaf.name}</strong>
					<small>{leaf.detail}</small>
					<button
						type="button"
						className="ui-button"
						onClick={() => {
							void tools.remove(leaf.id);
							setSelected(null);
						}}
					>
						Erase
					</button>
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

function ObjectsTab({
	entities,
	selectedIds,
	requests,
	onSelect,
}: {
	entities: readonly CadEntity[];
	selectedIds: readonly string[];
	requests: ElementsRequests;
	onSelect(ids: string[]): void;
}) {
	const [status, setStatus] = useState("");
	const objects = venueObjects(entities);

	useRequest(requests.importModel, () => void importModel());

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
			{status ? (
				<p className="cad-elements-status" role="status">
					{status}
				</p>
			) : null}
			{list(
				"Venue items",
				objects.filter((entity) => entity.scenery),
				"No trusses, stage elements or curtains placed yet.",
			)}
			{list(
				"3D models",
				objects.filter((entity) => !entity.scenery),
				"No 3D models placed yet.",
			)}
		</div>
	);
}

export function CadElementsPanel({
	tab,
	requests,
	documentKey,
	underlayState,
	tools,
	defaultView,
	entities,
	selectedIds,
	onSelect,
}: {
	tab: ElementsTab;
	requests: ElementsRequests;
	documentKey: string | null;
	underlayState: CadUnderlays;
	tools: CadTools;
	defaultView: CadViewDirection;
	entities: readonly CadEntity[];
	selectedIds: readonly string[];
	onSelect(ids: string[]): void;
}) {
	return (
		<div className="cad-elements-panel">
			{tab === "drawings" ? (
				<DrawingsTab
					documentKey={documentKey}
					underlayState={underlayState}
					tools={tools}
					defaultView={defaultView}
					requests={requests}
				/>
			) : (
				<ObjectsTab
					entities={entities}
					selectedIds={selectedIds}
					requests={requests}
					onSelect={onSelect}
				/>
			)}
		</div>
	);
}
