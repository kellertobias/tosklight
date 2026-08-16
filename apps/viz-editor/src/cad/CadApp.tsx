import { Button, SwitchField } from "@tosklight/ui";
import { WindowHeader, WindowSettings } from "@tosklight/ui/window-kit";
import { useEffect, useRef, useState } from "react";
import { beginWindowDrag, WindowControls } from "../WindowChrome";
import { CadViewport } from "./CadViewport";
import { cadSession } from "./session";
import {
	applySelectionChange,
	CAD_VIEW_LABELS,
	type CadSceneSnapshot,
	type CadViewDirection,
	mapTile,
	newTile,
	projectPoint,
	type SelectionChange,
	setSplitRatio,
	splitTileAtEdge,
	type TileCamera,
	type TileEdge,
	type TileNode,
	type ViewportTile,
} from "./types";

const WORKSPACE_KEY = "tosklight:viz-editor:cad-workspace:v1";

export function CadApp() {
	const [scene, setScene] = useState<CadSceneSnapshot | null>(null);
	const [layout, setLayout] = useState<TileNode>(restoreLayout);
	const [snapToMounts, setSnapToMounts] = useState(true);
	const [settingsOpen, setSettingsOpen] = useState(false);
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
					if (!current || delta.sceneRevision <= current.sceneRevision)
						return current;
					const entities = new Map(
						current.entities.map((entity) => [entity.id, entity]),
					);
					for (const id of delta.removedIds) entities.delete(id);
					for (const entity of delta.upserted) entities.set(entity.id, entity);
					const next = {
						...current,
						sceneRevision: delta.sceneRevision,
						entities: [...entities.values()],
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
		if (!scene || !entityIds.length) return;
		try {
			await cadSession.transform(
				scene.sceneRevision,
				entityIds,
				deltaMillimetres.map(Math.round) as [number, number, number],
				snapToMounts,
			);
			applyScene(await cadSession.snapshot());
		} catch (reason) {
			setError(String(reason));
			applyScene(await cadSession.snapshot());
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
		const positions = scene.entities.map((entity) =>
			projectPoint(entity.positionMillimetres, tile.view),
		);
		const minX = Math.min(...positions.map((position) => position[0]));
		const maxX = Math.max(...positions.map((position) => position[0]));
		const minY = Math.min(...positions.map((position) => position[1]));
		const maxY = Math.max(...positions.map((position) => position[1]));
		updateTile(id, (tile) => ({
			...tile,
			camera: {
				pan: [-(minX + maxX) / 2, -(minY + maxY) / 2],
				zoom: Math.max(
					0.008,
					Math.min(0.2, 900 / Math.max(5000, maxX - minX, maxY - minY)),
				),
			},
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
								id: "settings",
								label: "Settings",
								onPress: () => setSettingsOpen(true),
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
							{
								id: "fit",
								label: "Fit",
								disabled: !scene?.entities.length,
								onPress: () => fit(activeTileId ?? firstTileId(layout)),
							},
						],
					},
				]}
			/>
			{error ? <output className="cad-error">{error}</output> : null}
			<section className="cad-workspace">
				{scene ? (
					<CadTile
						node={layout}
						root={layout}
						scene={scene}
						snapToMounts={snapToMounts}
						onLayout={setLayout}
						onTile={updateTile}
						onSplitRatio={(id, ratio) =>
							setLayout((current) => setSplitRatio(current, id, ratio))
						}
						activeTileId={activeTileId}
						onActivate={setActiveTileId}
						onSelection={select}
						onMove={move}
					/>
				) : (
					<div className="cad-loading">Loading the canonical rig…</div>
				)}
			</section>
			{settingsOpen ? (
				<WindowSettings
					title="CAD Settings"
					onClose={() => setSettingsOpen(false)}
					tabs={[
						{
							id: "snapping",
							label: "Snapping",
							content: (
								<SwitchField
									label="Enable snapping"
									offLabel={null}
									onLabel={null}
									checked={snapToMounts}
									onChange={(event) =>
										setSnapToMounts(event.currentTarget.checked)
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

function firstTileId(node: TileNode): string {
	return node.type === "tile" ? node.id : firstTileId(node.first);
}

function findTile(node: TileNode, id: string): ViewportTile | null {
	if (node.type === "tile") return node.id === id ? node : null;
	return findTile(node.first, id) ?? findTile(node.second, id);
}

interface CadTileProps {
	node: TileNode;
	root: TileNode;
	scene: CadSceneSnapshot;
	snapToMounts: boolean;
	onLayout(layout: TileNode): void;
	onTile(id: string, change: (tile: ViewportTile) => ViewportTile): void;
	onSplitRatio(id: string, ratio: number): void;
	activeTileId: string | null;
	onActivate(id: string): void;
	onSelection(change: SelectionChange): void;
	onMove(
		delta: [number, number, number],
		entityIds: readonly string[],
	): Promise<void>;
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
				<CadTile {...props} node={node.first} />
				<CadDivider node={node} onRatio={props.onSplitRatio} />
				<CadTile {...props} node={node.second} />
			</div>
		);
	}
	return (
		<section
			className={`cad-tile ${props.activeTileId === node.id ? "is-active" : ""}`}
			onPointerDown={() => props.onActivate(node.id)}
		>
			<div className="cad-view-control">
				<select
					aria-label="View direction"
					value={node.view}
					onChange={(event) => {
						const view = event.currentTarget.value as ViewportTile["view"];
						props.onTile(node.id, (tile) => ({ ...tile, view }));
					}}
				>
					{Object.entries(CAD_VIEW_LABELS).map(([value, label]) => (
						<option key={value} value={value}>
							{label}
						</option>
					))}
				</select>
			</div>
			<CadOrientation view={node.view} />
			{(["left", "right", "top", "bottom"] as TileEdge[]).map((edge) => (
				<Button
					key={edge}
					className={`cad-add-viewport is-${edge}`}
					aria-label={`Add viewport ${edge}`}
					title={`Add viewport ${edge}`}
					onPointerDown={(event) => event.stopPropagation()}
					onClick={() =>
						props.onLayout(splitTileAtEdge(props.root, node.id, edge))
					}
				>
					<span aria-hidden="true">+</span>
				</Button>
			))}
			<CadViewport
				entities={props.scene.entities}
				selectedIds={props.scene.selectedIds}
				view={node.view}
				camera={node.camera}
				snapToMounts={props.snapToMounts}
				onCamera={(camera: TileCamera) =>
					props.onTile(node.id, (tile) => ({ ...tile, camera }))
				}
				onSelection={props.onSelection}
				onMove={props.onMove}
			/>
		</section>
	);
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

const ORIENTATION: Record<
	CadViewDirection,
	{ horizontal: string; vertical: string; depth: string }
> = {
	top_down: { horizontal: "+X", vertical: "−Y", depth: "+Z" },
	left_to_right: { horizontal: "+Y", vertical: "+Z", depth: "+X" },
	right_to_left: { horizontal: "−Y", vertical: "+Z", depth: "−X" },
	front_to_back: { horizontal: "+X", vertical: "+Z", depth: "+Y" },
	back_to_front: { horizontal: "−X", vertical: "+Z", depth: "−Y" },
};

function CadOrientation({ view }: { view: CadViewDirection }) {
	const axes = ORIENTATION[view];
	return (
		<div
			className="cad-orientation"
			role="img"
			aria-label={`Orientation: right ${axes.horizontal}, up ${axes.vertical}, depth ${axes.depth}`}
		>
			<span className="cad-axis-horizontal">{axes.horizontal}</span>
			<span className="cad-axis-vertical">{axes.vertical}</span>
			<span className="cad-axis-depth">{axes.depth}</span>
		</div>
	);
}

function restoreLayout(): TileNode {
	try {
		const stored = localStorage.getItem(WORKSPACE_KEY);
		if (stored) {
			const parsed = JSON.parse(stored) as TileNode;
			return parsed;
		}
	} catch {
		// A broken workspace preference must not prevent the canonical show from opening.
	}
	return newTile();
}
