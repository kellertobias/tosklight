import { save } from "@tauri-apps/plugin-dialog";
import { Button, SelectField, SwitchField } from "@tosklight/ui";
import { WindowHeader, WindowSettings } from "@tosklight/ui/window-kit";
import { useEffect, useRef, useState } from "react";
import { type DocumentSummary, documentSession } from "../document/session";
import { beginWindowDrag } from "../WindowChrome";
import { CadTileViewBar } from "./CadTileViewBar";
import {
	DEFAULT_GRID,
	GRID_SPACINGS_MILLIMETRES,
} from "./cadGrid";
import { CadGridColour } from "./CadGridColour";
import { CadSidePanels } from "./CadSidePanels";
import { CadToolError, cadTitleGroups } from "./CadToolbar";
import { useCadTools } from "./cadTools";
import { visibleEntities } from "./cutPlanes";
import { CadViewport } from "./CadViewport";
import { pannedCamera, useCadShortcuts, zoomedCamera } from "./cadShortcuts";
import { buildCadPdf, type CadPrintDocumentInfo } from "./print";
import { cadSession } from "./session";
import { underlaysForView } from "./underlayGeometry";
import type { CadUnderlay } from "./underlays";
import { useCadPrintPages } from "./useCadPrintPages";
import { useCadUnderlays } from "./useCadUnderlays";
import {
	applySelectionChange,
	CAD_VIEW_LABELS,
	type CadPrintPage,
	type CadSceneSnapshot,
	type CadTransformPreview,
	type CadViewDirection,
	mapTile,
	newTile,
	legacyTopDownPlanPoint,
	normaliseQuarterTurns,
	projectPoint,
	removeSplitSide,
	type SelectionChange,
	setSplitRatio,
	splitTileAtEdge,
	type TileCamera,
	type TileEdge,
	type TileNode,
	type CadEntity,
	type ViewportTile,
	viewAxes,
	type WorldAxis,
} from "./types";

// v2 plans show +Y up. v1 stored the same views mirrored and is converted once on first read.
const WORKSPACE_KEY = "tosklight:viz-editor:cad-workspace:v2";
const LEGACY_WORKSPACE_KEY = "tosklight:viz-editor:cad-workspace:v1";
const SETTINGS_KEY = "tosklight:viz-editor:cad-settings:v1";

interface CadSettings {
	snapToMounts: boolean;
	showFixtureIds: boolean;
	showDmxAddresses: boolean;
	showCoordinateOrigins: boolean;
	showGrid: boolean;
	gridColour: string;
	/** Fixed grid spacing in millimetres; null follows the scale indicator. */
	gridSpacingMillimetres: number | null;
	showSubGrid: boolean;
}

/**
 * The CAD planning surface, as one destination of the editor window.
 *
 * It draws no window controls of its own: it is a screen inside the Architect window rather than
 * a window, and the operator who wants it beside the patch sheet opens a second editor window.
 */

/** Paperwork before the document has been read; the Show screen edits the real one. */
const NO_PAPERWORK = {
	lightingDesigner: "",
	showVersion: "",
	venue: "",
	contactEmail: "",
	contactPhone: "",
	project: "",
	showDate: "",
};

/** The camera that fits the whole rig into one tile, or null when there is nothing to fit. */
function fitTileCamera(
	entities: Parameters<typeof fittedCamera>[0] | undefined,
	layout: Parameters<typeof findTile>[0],
	id: string,
) {
	const tile = entities?.length ? findTile(layout, id) : undefined;
	return tile && entities
		? fittedCamera(entities, tile.view, tile.rotationQuarterTurns)
		: null;
}

export function CadApp() {
	const [scene, setScene] = useState<CadSceneSnapshot | null>(null);
	const [layout, setLayout] = useState<TileNode>(restoreLayout);
	const [settings, setSettings] = useState<CadSettings>(restoreSettings);
	const [settingsOpen, setSettingsOpen] = useState(false);
	const [preview, setPreview] = useState<CadTransformPreview | null>(null);
	// Which side panel is open, and nothing when neither is. Print and Elements are two independent
	// choices rather than a mode with a tab strip inside it, so each title button opens its own
	// panel and closes it again when it is already the one showing.
	const [printPanel, setPrintPanel] = useState<"print" | "elements" | null>(
		null,
	);
	// Sheets belong to the Print panel; the others open the sidebar without papering the views.
	const printMode = printPanel === "print";
	const panelOpen = printPanel !== null;
	const printPageState = useCadPrintPages();
	const { pages: printPages, selectedId: selectedPrintPageId } = printPageState;
	const [exporting, setExporting] = useState(false);
	const [activeTileId, setActiveTileId] = useState<string | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [documentInfo, setDocumentInfo] = useState<DocumentSummary | null>(
		null,
	);
	const tools = useCadTools();
	const underlayState = useCadUnderlays(documentInfo?.showId ?? null);
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
					const drawings = new Map(
						current.drawings.map((drawing) => [drawing.id, drawing]),
					);
					for (const drawing of delta.drawings)
						drawings.set(drawing.id, drawing);
					const next = {
						...current,
						sceneRevision: delta.sceneRevision,
						// Native CAD deltas carry the complete physical-instance snapshot. Replacing
						// it also removes deleted multi-patches whose instance IDs are not root IDs.
						entities: delta.upserted,
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
		const refreshDocument = () =>
			documentSession
				.current()
				.then(setDocumentInfo)
				.catch((reason) => setError(String(reason)));
		const refreshFocusedWindow = () => {
			refreshDocument();
			cadSession
				.snapshot()
				.then(applyScene)
				.catch((reason) => setError(String(reason)));
		};
		refreshDocument();
		window.addEventListener("focus", refreshFocusedWindow);
		return () => window.removeEventListener("focus", refreshFocusedWindow);
	}, []);

	useEffect(() => {
		localStorage.setItem(WORKSPACE_KEY, JSON.stringify(layout));
	}, [layout]);

	useEffect(() => {
		localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
	}, [settings]);

	// The CAD settings belong to this computer's Architect, so another open window follows a change.
	useEffect(() => {
		const follow = (event: StorageEvent) => {
			if (event.key === SETTINGS_KEY) setSettings(restoreSettings());
		};
		window.addEventListener("storage", follow);
		return () => window.removeEventListener("storage", follow);
	}, []);

	const [focusedEntityId, setFocusedEntityId] = useState<string | null>(null);

	// An object an add button just placed is selected as soon as the drawing shows it, which opens Info.
	const handledPlacement = useRef(tools.placed?.request ?? 0);
	useEffect(() => {
		const placed = tools.placed;
		if (!placed || placed.request === handledPlacement.current || !scene) return;
		if (!scene.entities.some((entity) => entity.logicalFixtureId === placed.fixtureId)) return;
		handledPlacement.current = placed.request;
		setFocusedEntityId(placed.fixtureId);
		select({ type: "replace", ids: [placed.fixtureId] });
	});

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
		spread: boolean,
	) {
		if (!scene || !entityIds.length || printMode) return;
		setPreview(null);
		try {
			await cadSession.transform(
				scene.sceneRevision,
				entityIds,
				deltaMillimetres.map(Math.round) as [number, number, number],
				settings.snapToMounts,
				spread,
			);
			applyScene(await cadSession.snapshot());
		} catch (reason) {
			setError(String(reason));
			applyScene(await cadSession.snapshot());
		}
	}

	function togglePrintPanel(panel: "print" | "elements") {
		setPreview(null);
		setPrintPanel((current) => (current === panel ? null : panel));
	}

	async function exportPdf() {
		if (!scene) return;
		const selected = printPages.filter((page) => page.included);
		if (!selected.length) return;
		const path = await save({
			title: "Export CAD plan pages",
			defaultPath: "ToskLight Architect Plan.pdf",
			filters: [{ name: "PDF document", extensions: ["pdf"] }],
		});
		if (!path) return;
		const pdfPath = path.toLowerCase().endsWith(".pdf") ? path : `${path}.pdf`;
		setExporting(true);
		try {
			await cadSession.exportPdf(
				pdfPath,
				buildCadPdf(
					scene,
					selected,
					printInfo(documentInfo, documentInfo ?? NO_PAPERWORK),
					underlayState.underlays,
					tools.annotations,
				),
			);
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
		const camera = fitTileCamera(scene?.entities, layout, id);
		if (camera) updateTile(id, (tile) => ({ ...tile, camera }));
	}

	useCadShortcuts((shortcut) => {
		if (shortcut.type === "tool") {
			// Drawing is off while the print pages are open, and a host without tools offers none.
			if (tools.onAdd && !printMode) tools.setTool(shortcut.tool);
			return;
		}
		const tileId = activeTile(layout, activeTileId)?.id;
		if (!tileId) return;
		updateTile(tileId, (tile) => {
			switch (shortcut.type) {
				case "view":
					// As the view menu does: the new direction starts unrotated and framed on the rig.
					return {
						...tile,
						view: shortcut.view,
						rotationQuarterTurns: 0,
						camera: scene
							? fittedCamera(scene.entities, shortcut.view, 0)
							: tile.camera,
					};
				case "zoom":
					return { ...tile, camera: zoomedCamera(tile.camera, shortcut.factor) };
				case "pan":
					return {
						...tile,
						camera: pannedCamera(tile.camera, shortcut.horizontal, shortcut.vertical),
					};
			}
		});
	});

	return (
		<main className="cad-app">
			<WindowHeader
				title="CAD"
				dragHandleProps={{
					"data-tauri-drag-region": true,
					onPointerDown: beginWindowDrag,
				}}
				groups={[
					// Editing the drawing and describing the print are two different jobs, so the tool
					// groups come first and the side panels after them.
					...cadTitleGroups(tools, {
						disabled: !scene,
						onUndo: () => void history("undo"),
						onRedo: () => void history("redo"),
					}),
					{
						id: "cad-print",
						actions: [
							{
								id: "print",
								label: "Plans",
								active: printPanel === "print",
								onPress: () => togglePrintPanel("print"),
							},
							{
								id: "elements",
								label: "Elements",
								active: printPanel === "elements",
								onPress: () => togglePrintPanel("elements"),
							},
						],
					},
				]}
				settings
				onSettings={() => setSettingsOpen(true)}
			/>
			<CadToolError tools={tools} />
			{error ? <output className="cad-error">{error}</output> : null}
			<div className={`cad-print-layout ${panelOpen ? "is-printing" : ""}`}>
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
							onFocusEntity={setFocusedEntityId}
							onPreview={setPreview}
							onMove={move}
							onFit={fit}
							printMode={printMode}
							underlays={underlayState.underlays}
							printPages={printPages}
							selectedPrintPageId={selectedPrintPageId}
							onAddPrintPage={printPageState.addPlanPage}
							onSelectPrintPage={printPageState.select}
							onChangePrintPage={printPageState.change}
							documentInfo={printInfo(documentInfo, documentInfo ?? NO_PAPERWORK)}
						/>
					) : (
						<div className="cad-loading">Loading the canonical rig…</div>
					)}
				</section>
				<CadSidePanels
					panel={printPanel}
					scene={scene}
					tools={tools}
					underlayState={underlayState}
					defaultView={activeTileView(layout, activeTileId)}
					documentKey={documentInfo?.showId ?? null}
					printPages={printPageState}
					exporting={exporting}
					onExport={() => void exportPdf()}
					onSelect={(ids) => select({ type: "replace", ids })}
					focusedEntityId={focusedEntityId}
					onFocusEntity={setFocusedEntityId}
					onError={(reason) => setError(String(reason))}
				/>
			</div>
			{settingsOpen ? (
				<WindowSettings
					title="Architect Settings"
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
									<SwitchField
										label="Show coordinate origins"
										offLabel={null}
										onLabel={null}
										checked={settings.showCoordinateOrigins}
										onChange={(event) => {
											const checked = event.currentTarget.checked;
											setSettings((current) => ({
												...current,
												showCoordinateOrigins: checked,
											}));
										}}
									/>
								</div>
							),
						},
						{
							id: "grid",
							label: "Grid",
							content: (
								<GridSettings
									settings={settings}
									onChange={(change) =>
										setSettings((current) => ({ ...current, ...change }))
									}
								/>
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

/** The view of the tile the operator last worked in, which a new drawing is placed on by default. */
function activeTileView(
	node: TileNode,
	activeTileId: string | null,
): CadViewDirection {
	return activeTile(node, activeTileId)?.view ?? "top_down";
}

/** The tile the operator last worked in, or the first one before they have used any. */
function activeTile(
	node: TileNode,
	activeTileId: string | null,
): ViewportTile | null {
	const tiles: ViewportTile[] = [];
	const walk = (candidate: TileNode) => {
		if (candidate.type === "tile") tiles.push(candidate);
		else {
			walk(candidate.first);
			walk(candidate.second);
		}
	};
	walk(node);
	return tiles.find((tile) => tile.id === activeTileId) ?? tiles[0] ?? null;
}

export interface CadTileProps {
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
	onFocusEntity?(entityId: string | null): void;
	onPreview(preview: CadTransformPreview | null): void;
	onMove(
		delta: [number, number, number],
		entityIds: readonly string[],
		spread: boolean,
	): Promise<void>;
	onFit(id: string): void;
	printMode: boolean;
	underlays: readonly CadUnderlay[];
	printPages: readonly CadPrintPage[];
	selectedPrintPageId: string | null;
	onAddPrintPage(tile: ViewportTile): void;
	onSelectPrintPage(id: string): void;
	onChangePrintPage(id: string, change: Partial<CadPrintPage>): void;
	documentInfo: CadPrintDocumentInfo;
}

interface ClosePaneAction {
	splitId: string;
	remove: "first" | "second";
}

/** The elements one tile shows, once its own cut planes are applied. */
function tileEntities(scene: { entities: readonly CadEntity[] }, tile: ViewportTile) {
	return visibleEntities(scene.entities, tile.view, tile.cutPlanes);
}

function CadTile(props: CadTileProps) {
	// The range settings take the same corner as the view controls, so opening them slides the
	// controls out rather than stacking a second row of chrome over the drawing.
	const [rangeOpen, setRangeOpen] = useState(false);
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
			<CadTileViewBar
				node={node}
				scene={props.scene}
				onTile={props.onTile}
				onFit={props.onFit}
				rangeOpen={rangeOpen}
				setRangeOpen={setRangeOpen}
			/>
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
				entities={tileEntities(props.scene, node)}
				drawings={props.scene.drawings}
				selectedIds={props.scene.selectedIds}
				preview={props.preview}
				view={node.view}
				rotationQuarterTurns={node.rotationQuarterTurns}
				camera={node.camera}
				showFixtureIds={props.settings.showFixtureIds}
				showDmxAddresses={props.settings.showDmxAddresses}
				showCoordinateOrigins={props.settings.showCoordinateOrigins}
				grid={{
					show: props.settings.showGrid,
					colour: props.settings.gridColour,
					spacingMillimetres: props.settings.gridSpacingMillimetres,
					subGrid: props.settings.showSubGrid,
				}}
				printMode={props.printMode}
				underlays={underlaysForView(props.underlays, node.view)}
				onCamera={(camera: TileCamera) =>
					props.onTile(node.id, (tile) => ({ ...tile, camera }))
				}
				onSelection={props.onSelection}
				onFocusEntity={props.onFocusEntity}
				onPreview={props.onPreview}
				onMove={props.onMove}
				editEnabled={!props.printMode}
				printPages={props.printPages.filter(
					(page) =>
						page.kind !== "fixture_list" &&
						page.tileId === node.id &&
						page.view === node.view &&
						page.rotationQuarterTurns === node.rotationQuarterTurns,
				)}
				selectedPrintPageId={props.selectedPrintPageId}
				onSelectPrintPage={props.onSelectPrintPage}
				onChangePrintPage={props.onChangePrintPage}
				documentInfo={props.documentInfo}
			/>
		</section>
	);
}

function printInfo(
	summary: DocumentSummary | null,
	draft: Pick<
		DocumentSummary,
		| "lightingDesigner"
		| "showVersion"
		| "venue"
		| "contactEmail"
		| "contactPhone"
		| "project"
		| "showDate"
	>,
): CadPrintDocumentInfo {
	return {
		showName: summary?.name ?? "",
		lightingDesigner: draft.lightingDesigner,
		showVersion: draft.showVersion,
		venue: draft.venue,
		contactEmail: draft.contactEmail,
		contactPhone: draft.contactPhone,
		project: draft.project,
		showDate: draft.showDate,
		companyLogo: summary?.companyLogo ?? "",
		lastSavedAt: summary?.lastSavedAt ?? 0,
		fixtureCount: summary?.fixtureCount ?? 0,
		universeCount: summary?.universeCount ?? 0,
	};
}

function restoreSettings(): CadSettings {
	try {
		const stored = JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? "null");
		return {
			snapToMounts: stored?.snapToMounts !== false,
			showFixtureIds: stored?.showFixtureIds === true,
			showDmxAddresses: stored?.showDmxAddresses === true,
			showCoordinateOrigins: stored?.showCoordinateOrigins === true,
			// Settings saved before the grid existed show it, in its default colour and spacing.
			showGrid: stored?.showGrid !== false,
			gridColour:
				typeof stored?.gridColour === "string" && /^#[0-9a-f]{6}$/iu.test(stored.gridColour)
					? stored.gridColour
					: DEFAULT_GRID.colour,
			gridSpacingMillimetres: GRID_SPACINGS_MILLIMETRES.includes(stored?.gridSpacingMillimetres)
				? stored.gridSpacingMillimetres
				: null,
			showSubGrid: stored?.showSubGrid === true,
		};
	} catch {
		return {
			snapToMounts: true,
			showFixtureIds: false,
			showDmxAddresses: false,
			showCoordinateOrigins: false,
			showGrid: DEFAULT_GRID.show,
			gridColour: DEFAULT_GRID.colour,
			gridSpacingMillimetres: DEFAULT_GRID.spacingMillimetres,
			showSubGrid: DEFAULT_GRID.subGrid,
		};
	}
}

/** The Grid tab of the CAD settings: whether the grid shows, its colour, spacing and sub-grid. */
function GridSettings({
	settings,
	onChange,
}: {
	settings: CadSettings;
	onChange(change: Partial<CadSettings>): void;
}) {
	return (
		<div className="cad-settings-fields">
			<SwitchField
				label="Show grid"
				offLabel={null}
				onLabel={null}
				checked={settings.showGrid}
				onChange={(event) => onChange({ showGrid: event.currentTarget.checked })}
			/>
			<CadGridColour
				value={settings.gridColour}
				onChange={(gridColour) => onChange({ gridColour })}
			/>
			<SelectField
				label="Grid spacing"
				value={String(settings.gridSpacingMillimetres ?? "scale")}
				onChange={(value) =>
					onChange({ gridSpacingMillimetres: value === "scale" ? null : Number(value) })
				}
				options={[
					{ value: "scale", label: "Follow the scale indicator" },
					...GRID_SPACINGS_MILLIMETRES.map((millimetres) => ({
						value: String(millimetres),
						label: millimetres < 1000 ? `${millimetres / 10} cm` : `${millimetres / 1000} m`,
					})),
				]}
			/>
			<SwitchField
				label="Show sub-grid"
				offLabel={null}
				onLabel={null}
				checked={settings.showSubGrid}
				onChange={(event) => onChange({ showSubGrid: event.currentTarget.checked })}
			/>
		</div>
	);
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

/** Centre of the orientation circle and the radius its rotate arrows run along, in its wrap's pixels. */
const ROTATE_CENTRE = 52;
const ROTATE_RADIUS = 47;

/**
 * A short arc just outside the orientation circle from one angle to another, with its head at the
 * end. Angles are degrees counterclockwise from the right, as on a protractor, so 20° to 40° lies at
 * the circle's top right; the head therefore points the way the view turns.
 */
export function rotateArrowGeometry(fromDegrees: number, toDegrees: number) {
	const point = (degrees: number): [number, number] => {
		const radians = (degrees * Math.PI) / 180;
		return [
			ROTATE_CENTRE + ROTATE_RADIUS * Math.cos(radians),
			ROTATE_CENTRE - ROTATE_RADIUS * Math.sin(radians),
		];
	};
	const [x1, y1] = point(fromDegrees);
	const [x2, y2] = point(toDegrees);
	const clockwise = toDegrees < fromDegrees;
	const end = (toDegrees * Math.PI) / 180;
	// The direction the arc is travelling at its end, on screen.
	const [vx, vy] = clockwise
		? [Math.sin(end), Math.cos(end)]
		: [-Math.sin(end), -Math.cos(end)];
	const barb = (turnDegrees: number): [number, number] => {
		const turn = (turnDegrees * Math.PI) / 180;
		const [bx, by] = [-vx, -vy];
		return [
			x2 + 6 * (bx * Math.cos(turn) - by * Math.sin(turn)),
			y2 + 6 * (bx * Math.sin(turn) + by * Math.cos(turn)),
		];
	};
	const left = barb(-32);
	const right = barb(32);
	const xs = [x1, x2, left[0], right[0]];
	const ys = [y1, y2, left[1], right[1]];
	const pad = 5;
	const box = {
		x: Math.floor(Math.min(...xs) - pad),
		y: Math.floor(Math.min(...ys) - pad),
		width: Math.ceil(Math.max(...xs) - Math.min(...xs) + pad * 2),
		height: Math.ceil(Math.max(...ys) - Math.min(...ys) + pad * 2),
	};
	const f = (value: number) => value.toFixed(2);
	return {
		box,
		arc: `M ${f(x1)} ${f(y1)} A ${ROTATE_RADIUS} ${ROTATE_RADIUS} 0 0 ${clockwise ? 1 : 0} ${f(x2)} ${f(y2)}`,
		head: `M ${f(left[0])} ${f(left[1])} L ${f(x2)} ${f(y2)} L ${f(right[0])} ${f(right[1])}`,
	};
}

function RotateViewButton({
	direction,
	onRotate,
}: {
	direction: "clockwise" | "counterclockwise";
	onRotate(): void;
}) {
	// Both arrows sit at the circle's top right: clockwise from 40° down to 20°, counterclockwise from
	// 50° up to 70°, each with its head at the end further from the other.
	const { box, arc, head } =
		direction === "clockwise" ? rotateArrowGeometry(40, 20) : rotateArrowGeometry(50, 70);
	return (
		<Button
			className={`cad-rotate-view is-${direction}`}
			aria-label={`Rotate top-down view 90 degrees ${direction}`}
			title={`Rotate 90 degrees ${direction}`}
			style={{ left: box.x, top: box.y, width: box.width, height: box.height }}
			onPointerDown={(event) => event.stopPropagation()}
			onClick={onRotate}
		>
			<svg aria-hidden="true" viewBox={`${box.x} ${box.y} ${box.width} ${box.height}`}>
				<path d={arc} />
				<path d={head} />
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

export function fittedCamera(
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
		const legacy = localStorage.getItem(LEGACY_WORKSPACE_KEY);
		if (legacy)
			return normaliseStoredLayout(
				legacyTopDownLayout(JSON.parse(legacy) as TileNode),
			);
	} catch {
		// A broken workspace preference must not prevent the canonical show from opening.
	}
	return newTile();
}

/** A v1 layout's top-down cameras, pointed at the same part of the rig in the +Y-up plan. */
function legacyTopDownLayout(node: TileNode): TileNode {
	if (node.type !== "tile")
		return {
			...node,
			first: legacyTopDownLayout(node.first),
			second: legacyTopDownLayout(node.second),
		};
	if (node.view !== "top_down" || node.camera?.pan?.length !== 2) return node;
	return {
		...node,
		camera: {
			...node.camera,
			pan: legacyTopDownPlanPoint(
				node.camera.pan,
				node.rotationQuarterTurns ?? 0,
			),
		},
	};
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
