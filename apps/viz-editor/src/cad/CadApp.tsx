import { Button } from "@tosklight/ui";
import { useEffect, useMemo, useState } from "react";
import { CadViewport } from "./CadViewport";
import { cadSession } from "./session";
import {
	CAD_VIEW_LABELS,
	mapTile,
	newTile,
	projectPoint,
	splitTile,
	type CadSceneSnapshot,
	type TileCamera,
	type TileNode,
	type ViewportTile,
} from "./types";

const WORKSPACE_KEY = "tosklight:viz-editor:cad-workspace:v1";

export function CadApp() {
	const [scene, setScene] = useState<CadSceneSnapshot | null>(null);
	const [layout, setLayout] = useState<TileNode>(restoreLayout);
	const [snapToMounts, setSnapToMounts] = useState(true);
	const [error, setError] = useState<string | null>(null);

	useEffect(() => {
		let disposed = false;
		let sceneUnlisten: (() => void) | undefined;
		let selectionUnlisten: (() => void) | undefined;
		cadSession
			.snapshot()
			.then((snapshot) => !disposed && setScene(snapshot))
			.catch((reason) => !disposed && setError(String(reason)));
		cadSession
			.onSceneDelta((delta) => {
				setScene((current) => {
					if (!current || delta.sceneRevision <= current.sceneRevision) return current;
					const entities = new Map(current.entities.map((entity) => [entity.id, entity]));
					for (const id of delta.removedIds) entities.delete(id);
					for (const entity of delta.upserted) entities.set(entity.id, entity);
					return {
						...current,
						sceneRevision: delta.sceneRevision,
						entities: [...entities.values()],
						attachments: delta.attachments,
					};
				});
			})
			.then((unlisten) => {
				sceneUnlisten = unlisten;
			})
			.catch(() => undefined);
		cadSession
			.onSelectionDelta((delta) => {
				setScene((current) =>
					!current || delta.revision < current.selectionRevision
						? current
						: {
								...current,
								selectionRevision: delta.revision,
								selectedIds: delta.selectedIds,
							},
				);
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

	const selectedNames = useMemo(() => {
		if (!scene?.selectedIds.length) return "Nothing selected";
		const entities = new Map(scene.entities.map((entity) => [entity.id, entity.name]));
		return scene.selectedIds.map((id) => entities.get(id) ?? id).join(", ");
	}, [scene]);

	async function select(ids: readonly string[]) {
		if (!scene) return;
		try {
			const outcome = await cadSession.replaceSelection(scene.selectionRevision, ids);
			setScene((current) => current && ({
				...current,
				selectionRevision: outcome.revision,
				selectedIds: outcome.selectedIds,
			}));
		} catch (reason) {
			setError(String(reason));
			setScene(await cadSession.snapshot());
		}
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
			setScene(await cadSession.snapshot());
		} catch (reason) {
			setError(String(reason));
			setScene(await cadSession.snapshot());
		}
	}

	async function history(direction: "undo" | "redo") {
		if (!scene) return;
		try {
			await cadSession[direction](scene.sceneRevision);
			setScene(await cadSession.snapshot());
		} catch (reason) {
			setError(String(reason));
		}
	}

	function updateTile(id: string, change: (tile: ViewportTile) => ViewportTile) {
		setLayout((current) => mapTile(current, id, change));
	}

	function fit(id: string) {
		if (!scene?.entities.length) return;
		const tile = findTile(layout, id);
		if (!tile) return;
		const positions = scene.entities.map((entity) => projectPoint(entity.positionMillimetres, tile.view));
		const minX = Math.min(...positions.map((position) => position[0]));
		const maxX = Math.max(...positions.map((position) => position[0]));
		const minY = Math.min(...positions.map((position) => position[1]));
		const maxY = Math.max(...positions.map((position) => position[1]));
		updateTile(id, (tile) => ({
			...tile,
			camera: {
				pan: [-(minX + maxX) / 2, -(minY + maxY) / 2],
				zoom: Math.max(0.008, Math.min(0.2, 900 / Math.max(5000, maxX - minX, maxY - minY))),
			},
		}));
	}

	return (
		<main className="cad-app">
			<header className="cad-titlebar">
				<div>
					<h1>Rig Planner · CAD</h1>
					<p>First synchronized 2D planning slice of TL-60</p>
				</div>
				<div className="cad-title-actions">
					<Button onClick={() => history("undo")}>Undo</Button>
					<Button onClick={() => history("redo")}>Redo</Button>
					<label className="cad-snap">
						<input
							type="checkbox"
							checked={snapToMounts}
							onChange={(event) => setSnapToMounts(event.currentTarget.checked)}
						/>
						Snap to declared truss mounts
					</label>
				</div>
			</header>
			{error ? <output className="cad-error" onClick={() => setError(null)}>{error}</output> : null}
			<section className="cad-workspace">
				{scene ? (
					<CadTile
						node={layout}
						root={layout}
						scene={scene}
						snapToMounts={snapToMounts}
						onLayout={setLayout}
						onTile={updateTile}
						onFit={fit}
						onSelection={select}
						onMove={move}
					/>
				) : (
					<div className="cad-loading">Loading the canonical rig…</div>
				)}
			</section>
			<footer className="cad-status">
				<span>{selectedNames}</span>
				<span>{scene ? `Scene r${scene.sceneRevision} · Selection r${scene.selectionRevision}` : "Connecting…"}</span>
				<span>{scene?.attachments.length ?? 0} mounted</span>
			</footer>
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
	scene: CadSceneSnapshot;
	snapToMounts: boolean;
	onLayout(layout: TileNode): void;
	onTile(id: string, change: (tile: ViewportTile) => ViewportTile): void;
	onFit(id: string): void;
	onSelection(ids: readonly string[]): void;
	onMove(delta: [number, number, number], entityIds: readonly string[]): Promise<void>;
}

function CadTile(props: CadTileProps) {
	const { node } = props;
	if (node.type === "split") {
		return (
			<div
				className={`cad-split is-${node.direction}`}
				style={{ "--cad-split-ratio": `${node.ratio * 100}%` } as React.CSSProperties}
			>
				<CadTile {...props} node={node.first} />
				<div className="cad-divider" />
				<CadTile {...props} node={node.second} />
			</div>
		);
	}
	return (
		<section className="cad-tile">
			<header className="cad-tile-tools">
				<select
					aria-label="View direction"
					value={node.view}
					onChange={(event) => props.onTile(node.id, (tile) => ({
						...tile,
						view: event.currentTarget.value as ViewportTile["view"],
					}))}
				>
					{Object.entries(CAD_VIEW_LABELS).map(([value, label]) => <option key={value} value={value}>{label}</option>)}
				</select>
				<Button onClick={() => props.onFit(node.id)}>Fit</Button>
				<Button aria-label="Split horizontally" onClick={() => props.onLayout(splitTile(props.root, node.id, "horizontal"))}>Split H</Button>
				<Button aria-label="Split vertically" onClick={() => props.onLayout(splitTile(props.root, node.id, "vertical"))}>Split V</Button>
			</header>
			<CadViewport
				entities={props.scene.entities}
				selectedIds={props.scene.selectedIds}
				view={node.view}
				camera={node.camera}
				snapToMounts={props.snapToMounts}
				onCamera={(camera: TileCamera) => props.onTile(node.id, (tile) => ({ ...tile, camera }))}
				onSelection={props.onSelection}
				onMove={props.onMove}
			/>
		</section>
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
