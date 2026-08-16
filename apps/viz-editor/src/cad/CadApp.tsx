import { save } from "@tauri-apps/plugin-dialog";
import { Button, SwitchField } from "@tosklight/ui";
import { WindowHeader, WindowSettings } from "@tosklight/ui/window-kit";
import { useEffect, useRef, useState } from "react";
import { beginWindowDrag, WindowControls } from "../WindowChrome";
import { CadViewport } from "./CadViewport";
import { buildCadPdf } from "./print";
import { cadSession } from "./session";
import {
	applySelectionChange,
	CAD_VIEW_LABELS,
	type CadPrintPage,
	type CadSceneSnapshot,
	type CadTransformPreview,
	type CadViewDirection,
	mapTile,
	newTile,
	normaliseQuarterTurns,
	projectPoint,
	removeSplitSide,
	type SelectionChange,
	setSplitRatio,
	splitTileAtEdge,
	type TileCamera,
	type TileEdge,
	type TileNode,
	type ViewportTile,
	viewAxes,
	type WorldAxis,
} from "./types";

const WORKSPACE_KEY = "tosklight:viz-editor:cad-workspace:v1";
const SETTINGS_KEY = "tosklight:viz-editor:cad-settings:v1";
const PRINT_KEY = "tosklight:viz-editor:cad-print-pages:v1";

interface CadSettings {
	snapToMounts: boolean;
	showFixtureIds: boolean;
	showDmxAddresses: boolean;
}

export function CadApp() {
	const [scene, setScene] = useState<CadSceneSnapshot | null>(null);
	const [layout, setLayout] = useState<TileNode>(restoreLayout);
	const [settings, setSettings] = useState<CadSettings>(restoreSettings);
	const [settingsOpen, setSettingsOpen] = useState(false);
	const [preview, setPreview] = useState<CadTransformPreview | null>(null);
	const [printMode, setPrintMode] = useState(false);
	const [printPages, setPrintPages] =
		useState<CadPrintPage[]>(restorePrintPages);
	const [selectedPrintPageId, setSelectedPrintPageId] = useState<string | null>(
		null,
	);
	const [exporting, setExporting] = useState(false);
	const [activeTileId, setActiveTileId] = useState<string | null>(null);
	const [error, setError] = useState<string | null>(null);
	const sceneRef = useRef<CadSceneSnapshot | null>(null);
	const selectionQueue = useRef<Promise<void>>(Promise.resolve());

	function applyScene(next: CadSceneSnapshot | null) {
		sceneRef.current = next;
		setScene(next);
	}

	useEffect(() => {
		let disposed = false;
		let sceneUnlisten: (() => void) | undefined;
		let selectionUnlisten: (() => void) | undefined;
		cadSession
			.snapshot()
			.then((snapshot) => !disposed && applyScene(snapshot))
			.catch((reason) => !disposed && setError(String(reason)));
		cadSession
			.onSceneDelta((delta) => {
				setScene((current) => {
					if (!current || delta.sceneRevision < current.sceneRevision)
						return current;
					const entities = new Map(
						current.entities.map((entity) => [entity.id, entity]),
					);
					const drawings = new Map(
						current.drawings.map((drawing) => [drawing.id, drawing]),
					);
					for (const id of delta.removedIds) entities.delete(id);
					for (const entity of delta.upserted) entities.set(entity.id, entity);
					for (const drawing of delta.drawings)
						drawings.set(drawing.id, drawing);
					const next = {
						...current,
						sceneRevision: delta.sceneRevision,
						entities: [...entities.values()],
						drawings: [...drawings.values()],
						attachments: delta.attachments,
					};
					sceneRef.current = next;
					return next;
				});
			})
			.then((unlisten) => {
				sceneUnlisten = unlisten;
			})
			.catch(() => undefined);
		cadSession
			.onSelectionDelta((delta) => {
				setScene((current) => {
					if (!current || delta.revision < current.selectionRevision)
						return current;
					const next = {
						...current,
						selectionRevision: delta.revision,
						selectedIds: delta.selectedIds,
					};
					sceneRef.current = next;
					return next;
				});
			})
			.then((unlisten) => {
				selectionUnlisten = unlisten;
			})
			.catch(() => undefined);
		return () => {
			disposed = true;
			sceneUnlisten?.();
			selectionUnlisten?.();
		};
	}, []);

	useEffect(() => {
		localStorage.setItem(WORKSPACE_KEY, JSON.stringify(layout));
	}, [layout]);

	useEffect(() => {
		localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
	}, [settings]);

	useEffect(() => {
		localStorage.setItem(PRINT_KEY, JSON.stringify(printPages));
	}, [printPages]);

	function select(change: SelectionChange) {
		selectionQueue.current = selectionQueue.current.then(async () => {
			const current = sceneRef.current;
			if (!current) return;
			const ids = applySelectionChange(current.selectedIds, change);
			applyScene({ ...current, selectedIds: ids });
			try {
				const outcome = await cadSession.replaceSelection(
					current.selectionRevision,
					ids,
				);
				applyScene({
					...(sceneRef.current ?? current),
					selectionRevision: outcome.revision,
					selectedIds: outcome.selectedIds,
				});
			} catch (reason) {
				setError(String(reason));
				try {
					applyScene(await cadSession.snapshot());
				} catch (refreshReason) {
					setError(String(refreshReason));
				}
			}
		});
	}

	async function move(
		deltaMillimetres: [number, number, number],
		entityIds: readonly string[],
	) {
		if (!scene || !entityIds.length || printMode) return;
		setPreview(null);
		try {
			await cadSession.transform(
				scene.sceneRevision,
				entityIds,
				deltaMillimetres.map(Math.round) as [number, number, number],
				settings.snapToMounts,
			);
			applyScene(await cadSession.snapshot());
		} catch (reason) {
			setError(String(reason));
			applyScene(await cadSession.snapshot());
		}
	}

	function togglePrintMode() {
		setPreview(null);
		setPrintMode((current) => !current);
	}

	function addPrintPage(tile: ViewportTile) {
		const pageNumber = printPages.length + 1;
		const page: CadPrintPage = {
			id:
				globalThis.crypto?.randomUUID?.() ?? `page-${Date.now()}-${pageNumber}`,
			tileId: tile.id,
			name: `Page ${pageNumber}`,
			view: tile.view,
			rotationQuarterTurns: tile.rotationQuarterTurns,
			centreMillimetres: [-tile.camera.pan[0], -tile.camera.pan[1]],
			widthMillimetres: Math.max(3000, 360 / tile.camera.zoom),
			included: true,
		};
		setPrintPages((current) => [...current, page]);
		setSelectedPrintPageId(page.id);
	}

	function changePrintPage(id: string, change: Partial<CadPrintPage>) {
		setPrintPages((current) =>
			current.map((page) => (page.id === id ? { ...page, ...change } : page)),
		);
	}

	async function exportPdf() {
		if (!scene) return;
		const selected = printPages.filter((page) => page.included);
		if (!selected.length) return;
		const path = await save({
			title: "Export CAD plan pages",
			defaultPath: "ToskLight Rig Plan.pdf",
			filters: [{ name: "PDF document", extensions: ["pdf"] }],
		});
		if (!path) return;
		const pdfPath = path.toLowerCase().endsWith(".pdf") ? path : `${path}.pdf`;
		setExporting(true);
		try {
			await cadSession.exportPdf(pdfPath, buildCadPdf(scene, selected));
		} catch (reason) {
			setError(String(reason));
		} finally {
			setExporting(false);
		}
	}

	async function history(direction: "undo" | "redo") {
		if (!scene) return;
		try {
			await cadSession[direction](scene.sceneRevision);
			applyScene(await cadSession.snapshot());
		} catch (reason) {
			setError(String(reason));
		}
	}

	function updateTile(
		id: string,
		change: (tile: ViewportTile) => ViewportTile,
	) {
		setLayout((current) => mapTile(current, id, change));
	}

	function fit(id: string) {
		if (!scene?.entities.length) return;
		const tile = findTile(layout, id);
		if (!tile) return;
		updateTile(id, (tile) => ({
			...tile,
			camera: fittedCamera(
				scene.entities,
				tile.view,
				tile.rotationQuarterTurns,
			),
		}));
	}

	return (
		<main className="cad-app">
			<WindowControls />
			<WindowHeader
				title="Rig Planner · CAD"
				dragHandleProps={{
					"data-tauri-drag-region": true,
					onPointerDown: beginWindowDrag,
				}}
				groups={[
					{
						id: "cad-actions",
						actions: [
							{
								id: "print",
								label: "Print",
								active: printMode,
								onPress: togglePrintMode,
							},
							{
								id: "undo",
								label: "Undo",
								disabled: !scene,
								onPress: () => void history("undo"),
							},
							{
								id: "redo",
								label: "Redo",
								disabled: !scene,
								onPress: () => void history("redo"),
							},
						],
					},
				]}
				settings
				onSettings={() => setSettingsOpen(true)}
			/>
			{error ? <output className="cad-error">{error}</output> : null}
			<div className={`cad-print-layout ${printMode ? "is-printing" : ""}`}>
				<section className="cad-workspace">
					{scene ? (
						<CadTile
							node={layout}
							root={layout}
							closeActions={{}}
							scene={scene}
							settings={settings}
							preview={preview}
							onLayout={setLayout}
							onTile={updateTile}
							onSplitRatio={(id, ratio) =>
								setLayout((current) => setSplitRatio(current, id, ratio))
							}
							activeTileId={activeTileId}
							onActivate={setActiveTileId}
							onSelection={select}
							onPreview={setPreview}
							onMove={move}
							onFit={fit}
							printMode={printMode}
							printPages={printPages}
							selectedPrintPageId={selectedPrintPageId}
							onAddPrintPage={addPrintPage}
							onSelectPrintPage={setSelectedPrintPageId}
							onChangePrintPage={changePrintPage}
						/>
					) : (
						<div className="cad-loading">Loading the canonical rig…</div>
					)}
				</section>
				{printMode ? (
					<aside className="cad-print-sidebar" aria-label="Print pages">
						<header>
							<h2>Prints</h2>
							<span>A4 landscape</span>
						</header>
						<div className="cad-print-list">
							{printPages.length ? (
								printPages.map((page, index) => (
									<div
										key={page.id}
										className={`cad-print-row ${
											page.id === selectedPrintPageId ? "is-selected" : ""
										}`}
									>
										<input
											aria-label={`Include ${page.name}`}
											type="checkbox"
											checked={page.included}
											onChange={(event) =>
												changePrintPage(page.id, {
													included: event.currentTarget.checked,
												})
											}
										/>
										<button
											type="button"
											onClick={() => setSelectedPrintPageId(page.id)}
										>
											<strong>
												{index + 1}. {page.name}
											</strong>
											<small>{CAD_VIEW_LABELS[page.view]}</small>
										</button>
									</div>
								))
							) : (
								<p>Add a page from any view.</p>
							)}
						</div>
						<Button
							className="cad-export-pdf"
							disabled={exporting || !printPages.some((page) => page.included)}
							onClick={() => void exportPdf()}
						>
							{exporting ? "Exporting…" : "Export to PDF"}
						</Button>
					</aside>
				) : null}
			</div>
			{settingsOpen ? (
				<WindowSettings
					title="CAD Settings"
					onClose={() => setSettingsOpen(false)}
					tabs={[
						{
							id: "general",
							label: "General",
							content: (
								<div className="cad-settings-fields">
									<SwitchField
										label="Enable snapping"
										offLabel={null}
										onLabel={null}
										checked={settings.snapToMounts}
										onChange={(event) =>
											setSettings((current) => ({
												...current,
												snapToMounts: event.currentTarget.checked,
											}))
										}
									/>
									<SwitchField
										label="Show fixture IDs"
										offLabel={null}
										onLabel={null}
										checked={settings.showFixtureIds}
										onChange={(event) =>
											setSettings((current) => ({
												...current,
												showFixtureIds: event.currentTarget.checked,
											}))
										}
									/>
									<SwitchField
										label="Show DMX addresses"
										offLabel={null}
										onLabel={null}
										checked={settings.showDmxAddresses}
										onChange={(event) =>
											setSettings((current) => ({
												...current,
												showDmxAddresses: event.currentTarget.checked,
											}))
										}
									/>
								</div>
							),
						},
					]}
				/>
			) : null}
		</main>
	);
}

function findTile(node: TileNode, id: string): ViewportTile | null {
	if (node.type === "tile") return node.id === id ? node : null;
	return findTile(node.first, id) ?? findTile(node.second, id);
}

interface CadTileProps {
	node: TileNode;
	root: TileNode;
	closeActions: Partial<Record<TileEdge, ClosePaneAction>>;
	scene: CadSceneSnapshot;
	settings: CadSettings;
	preview: CadTransformPreview | null;
	onLayout(layout: TileNode): void;
	onTile(id: string, change: (tile: ViewportTile) => ViewportTile): void;
	onSplitRatio(id: string, ratio: number): void;
	activeTileId: string | null;
	onActivate(id: string): void;
	onSelection(change: SelectionChange): void;
	onPreview(preview: CadTransformPreview | null): void;
	onMove(
		delta: [number, number, number],
		entityIds: readonly string[],
	): Promise<void>;
	onFit(id: string): void;
	printMode: boolean;
	printPages: readonly CadPrintPage[];
	selectedPrintPageId: string | null;
	onAddPrintPage(tile: ViewportTile): void;
	onSelectPrintPage(id: string): void;
	onChangePrintPage(id: string, change: Partial<CadPrintPage>): void;
}

interface ClosePaneAction {
	splitId: string;
	remove: "first" | "second";
}

function CadTile(props: CadTileProps) {
	const { node } = props;
	if (node.type === "split") {
		return (
			<div
				className={`cad-split is-${node.direction}`}
				style={
					{ "--cad-split-ratio": `${node.ratio * 100}%` } as React.CSSProperties
				}
			>
				<CadTile
					{...props}
					node={node.first}
					closeActions={childCloseActions(props.closeActions, node, "first")}
				/>
				<CadDivider node={node} onRatio={props.onSplitRatio} />
				<CadTile
					{...props}
					node={node.second}
					closeActions={childCloseActions(props.closeActions, node, "second")}
				/>
			</div>
		);
	}
	return (
		<section
			className={`cad-tile ${props.activeTileId === node.id ? "is-active" : ""}`}
			onPointerDown={() => props.onActivate(node.id)}
		>
			{props.printMode ? (
				<Button
					className="cad-add-print-page"
					onPointerDown={(event) => event.stopPropagation()}
					onClick={() => props.onAddPrintPage(node)}
				>
					Add New Page
				</Button>
			) : null}
			<div className="cad-view-control">
				<select
					aria-label="View direction"
					value={node.view}
					onChange={(event) => {
						const view = event.currentTarget.value as ViewportTile["view"];
						props.onTile(node.id, (tile) => ({
							...tile,
							view,
							rotationQuarterTurns: 0,
							camera: fittedCamera(props.scene.entities, view, 0),
						}));
					}}
				>
					{Object.entries(CAD_VIEW_LABELS).map(([value, label]) => (
						<option key={value} value={value}>
							{label}
						</option>
					))}
				</select>
				<Button
					className="cad-fit-view"
					disabled={!props.scene.entities.length}
					onClick={() => props.onFit(node.id)}
				>
					Fit
				</Button>
			</div>
			<CadOrientation
				view={node.view}
				rotationQuarterTurns={node.rotationQuarterTurns}
				onRotate={
					node.view === "top_down"
						? (delta) => rotateTile(props, node, delta)
						: undefined
				}
			/>
			{(["left", "right", "top", "bottom"] as TileEdge[]).map((edge) => {
				const close = props.closeActions[edge];
				return (
					<div key={edge} className={`cad-edge-controls is-${edge}`}>
						<Button
							className="cad-add-viewport"
							aria-label={`Add viewport ${edge}`}
							title={`Add viewport ${edge}`}
							onPointerDown={(event) => event.stopPropagation()}
							onClick={() =>
								props.onLayout(splitTileAtEdge(props.root, node.id, edge))
							}
						>
							<span aria-hidden="true">+</span>
						</Button>
						{close ? (
							<Button
								className="cad-close-viewport"
								aria-label={`Close pane ${edge}`}
								title={`Close pane ${edge}`}
								onPointerDown={(event) => event.stopPropagation()}
								onClick={() =>
									props.onLayout(
										removeSplitSide(props.root, close.splitId, close.remove),
									)
								}
							>
								<span aria-hidden="true">{edgeArrow(edge)}</span>
							</Button>
						) : null}
					</div>
				);
			})}
			<CadViewport
				entities={props.scene.entities}
				drawings={props.scene.drawings}
				selectedIds={props.scene.selectedIds}
				preview={props.preview}
				view={node.view}
				rotationQuarterTurns={node.rotationQuarterTurns}
				camera={node.camera}
				showFixtureIds={props.settings.showFixtureIds}
				showDmxAddresses={props.settings.showDmxAddresses}
				printMode={props.printMode}
				onCamera={(camera: TileCamera) =>
					props.onTile(node.id, (tile) => ({ ...tile, camera }))
				}
				onSelection={props.onSelection}
				onPreview={props.onPreview}
				onMove={props.onMove}
				editEnabled={!props.printMode}
				printPages={props.printPages.filter(
					(page) =>
						page.tileId === node.id &&
						page.view === node.view &&
						page.rotationQuarterTurns === node.rotationQuarterTurns,
				)}
				selectedPrintPageId={props.selectedPrintPageId}
				onSelectPrintPage={props.onSelectPrintPage}
				onChangePrintPage={props.onChangePrintPage}
			/>
		</section>
	);
}

function restoreSettings(): CadSettings {
	try {
		const stored = JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? "null");
		return {
			snapToMounts: stored?.snapToMounts !== false,
			showFixtureIds: stored?.showFixtureIds === true,
			showDmxAddresses: stored?.showDmxAddresses === true,
		};
	} catch {
		return {
			snapToMounts: true,
			showFixtureIds: false,
			showDmxAddresses: false,
		};
	}
}

function restorePrintPages(): CadPrintPage[] {
	try {
		const stored = JSON.parse(localStorage.getItem(PRINT_KEY) ?? "[]");
		if (!Array.isArray(stored)) return [];
		return stored.filter(
			(page): page is CadPrintPage =>
				typeof page?.id === "string" &&
				typeof page?.tileId === "string" &&
				typeof page?.name === "string" &&
				page?.centreMillimetres?.length === 2 &&
				Number.isFinite(page?.widthMillimetres),
		);
	} catch {
		return [];
	}
}

function rotateTile(props: CadTileProps, tile: ViewportTile, delta: -1 | 1) {
	const rotationQuarterTurns = normaliseQuarterTurns(
		tile.rotationQuarterTurns + delta,
	);
	props.onTile(tile.id, (current) => ({
		...current,
		rotationQuarterTurns,
		camera: fittedCamera(
			props.scene.entities,
			current.view,
			rotationQuarterTurns,
		),
	}));
}

function RotateViewButton({
	direction,
	onRotate,
}: {
	direction: "clockwise" | "counterclockwise";
	onRotate(): void;
}) {
	return (
		<Button
			className={`cad-rotate-view is-${direction}`}
			aria-label={`Rotate top-down view 90 degrees ${direction}`}
			title={`Rotate 90 degrees ${direction}`}
			onPointerDown={(event) => event.stopPropagation()}
			onClick={onRotate}
		>
			<svg aria-hidden="true" viewBox="0 0 56 56">
				{direction === "counterclockwise" ? (
					<>
						<path d="M 54 9 A 45 45 0 0 0 9 54" />
						<path d="M 9 54 L 8 43 M 9 54 L 20 53" />
					</>
				) : (
					<>
						<path d="M 2 47 A 45 45 0 0 0 47 2" />
						<path d="M 47 2 L 36 3 M 47 2 L 48 13" />
					</>
				)}
			</svg>
		</Button>
	);
}

function childCloseActions(
	outer: Partial<Record<TileEdge, ClosePaneAction>>,
	node: Extract<TileNode, { type: "split" }>,
	branch: "first" | "second",
): Partial<Record<TileEdge, ClosePaneAction>> {
	const actions: Partial<Record<TileEdge, ClosePaneAction>> = {};
	for (const edge of ["left", "right", "top", "bottom"] as TileEdge[]) {
		if (touchesOuterEdge(node.direction, branch, edge) && outer[edge])
			actions[edge] = outer[edge];
	}
	const edge =
		node.direction === "horizontal"
			? branch === "first"
				? "right"
				: "left"
			: branch === "first"
				? "bottom"
				: "top";
	actions[edge] = {
		splitId: node.id,
		remove: branch === "first" ? "second" : "first",
	};
	return actions;
}

function touchesOuterEdge(
	direction: "horizontal" | "vertical",
	branch: "first" | "second",
	edge: TileEdge,
) {
	if (direction === "horizontal") {
		if (edge === "left") return branch === "first";
		if (edge === "right") return branch === "second";
		return true;
	}
	if (edge === "top") return branch === "first";
	if (edge === "bottom") return branch === "second";
	return true;
}

function edgeArrow(edge: TileEdge) {
	return { left: "←", right: "→", top: "↑", bottom: "↓" }[edge];
}

function CadDivider({
	node,
	onRatio,
}: {
	node: Extract<TileNode, { type: "split" }>;
	onRatio(id: string, ratio: number): void;
}) {
	const dragging = useRef(false);
	return (
		<hr
			className="cad-divider"
			tabIndex={0}
			aria-label={
				node.direction === "horizontal" ? "Resize columns" : "Resize rows"
			}
			aria-orientation={
				node.direction === "horizontal" ? "vertical" : "horizontal"
			}
			aria-valuemin={15}
			aria-valuemax={85}
			aria-valuenow={Math.round(node.ratio * 100)}
			onPointerDown={(event) => {
				event.preventDefault();
				event.stopPropagation();
				dragging.current = true;
				event.currentTarget.setPointerCapture?.(event.pointerId);
			}}
			onPointerMove={(event) => {
				if (!dragging.current) return;
				const rect = event.currentTarget.parentElement?.getBoundingClientRect();
				if (!rect) return;
				const ratio =
					node.direction === "horizontal"
						? (event.clientX - rect.left) / rect.width
						: (event.clientY - rect.top) / rect.height;
				onRatio(node.id, ratio);
			}}
			onPointerUp={(event) => {
				dragging.current = false;
				event.currentTarget.releasePointerCapture?.(event.pointerId);
			}}
			onPointerCancel={() => {
				dragging.current = false;
			}}
		/>
	);
}

const DEPTH_AXIS: Record<CadViewDirection, { axis: WorldAxis; sign: 1 | -1 }> =
	{
		top_down: { axis: "z", sign: 1 },
		left_to_right: { axis: "x", sign: 1 },
		right_to_left: { axis: "x", sign: -1 },
		front_to_back: { axis: "y", sign: 1 },
		back_to_front: { axis: "y", sign: -1 },
	};

function CadOrientation({
	view,
	rotationQuarterTurns,
	onRotate,
}: {
	view: CadViewDirection;
	rotationQuarterTurns: number;
	onRotate?(delta: -1 | 1): void;
}) {
	const axes = viewAxes(view, rotationQuarterTurns);
	const horizontal = axisLabel(axes.horizontal);
	const vertical = axisLabel(axes.vertical);
	const depth = axisLabel(DEPTH_AXIS[view]);
	return (
		<div className="cad-orientation-wrap">
			<div
				className="cad-orientation"
				role="img"
				aria-label={`Orientation: right ${horizontal}, up ${vertical}, depth ${depth}`}
			>
				<span className={`cad-axis-horizontal is-${axes.horizontal.axis}`}>
					{horizontal}
				</span>
				<span className={`cad-axis-vertical is-${axes.vertical.axis}`}>
					{vertical}
				</span>
				<span className="cad-axis-origin" aria-hidden="true">
					+
				</span>
				<span className={`cad-axis-depth is-${DEPTH_AXIS[view].axis}`}>
					{depth}
				</span>
			</div>
			{onRotate ? (
				<>
					<RotateViewButton
						direction="counterclockwise"
						onRotate={() => onRotate(-1)}
					/>
					<RotateViewButton
						direction="clockwise"
						onRotate={() => onRotate(1)}
					/>
				</>
			) : null}
		</div>
	);
}

function axisLabel(value: { axis: WorldAxis; sign: 1 | -1 }) {
	return `${value.sign === 1 ? "+" : "−"}${value.axis.toUpperCase()}`;
}

function fittedCamera(
	entities: readonly CadSceneSnapshot["entities"][number][],
	view: CadViewDirection,
	rotationQuarterTurns: number,
): TileCamera {
	if (!entities.length) return { pan: [0, 0], zoom: 0.08 };
	const positions = entities.map((entity) =>
		projectPoint(entity.positionMillimetres, view, rotationQuarterTurns),
	);
	const minX = Math.min(...positions.map((position) => position[0]));
	const maxX = Math.max(...positions.map((position) => position[0]));
	const minY = Math.min(...positions.map((position) => position[1]));
	const maxY = Math.max(...positions.map((position) => position[1]));
	return {
		pan: [-(minX + maxX) / 2, -(minY + maxY) / 2],
		zoom: Math.max(
			0.008,
			Math.min(0.2, 900 / Math.max(5000, maxX - minX, maxY - minY)),
		),
	};
}

function restoreLayout(): TileNode {
	try {
		const stored = localStorage.getItem(WORKSPACE_KEY);
		if (stored) {
			const parsed = JSON.parse(stored) as TileNode;
			return normaliseStoredLayout(parsed);
		}
	} catch {
		// A broken workspace preference must not prevent the canonical show from opening.
	}
	return newTile();
}

function normaliseStoredLayout(node: TileNode): TileNode {
	if (node.type === "tile") {
		return {
			...node,
			rotationQuarterTurns: normaliseQuarterTurns(
				node.rotationQuarterTurns ?? 0,
			),
		};
	}
	return {
		...node,
		first: normaliseStoredLayout(node.first),
		second: normaliseStoredLayout(node.second),
	};
}
