/**
 * One tile's viewport, and the only part of a tile that follows the move preview.
 *
 * It reads the preview straight from the shared store, so a drag re-renders the viewports that
 * draw it rather than the CAD screen around them, and it keeps the props it derives for the
 * viewport stable between renders so the renderer's cached rig is not thrown away.
 */
import { useCallback, useMemo } from "react";
import type { CadTileProps } from "./CadApp";
import { useLivePreview } from "./cadPreviewStore";
import { CadViewport } from "./CadViewport";
import { visibleEntities } from "./cutPlanes";
import { withTurnedPlacements } from "./gizmoRotation";
import { withDuplicatePreview } from "./cadDuplicate";
import type { TileCamera, ViewportTile } from "./types";
import { underlaysForView } from "./underlayGeometry";

export function CadTileViewport({
	props,
	node,
}: {
	props: CadTileProps;
	node: ViewportTile;
}) {
	const { scene, settings, printMode, onTile } = props;
	const live = useLivePreview(props.previewStore, scene.sceneRevision);
	// The elements this tile shows, once its own cut planes are applied.
	const visible = useMemo(
		() => visibleEntities(scene.entities, node.view, node.cutPlanes),
		[scene.entities, node.view, node.cutPlanes],
	);
	// A turn in flight draws each turned element where the turn puts it, and a duplicating move
	// leaves the originals and moves their copies.
	const { entities, preview } = useMemo(
		() => withDuplicatePreview(withTurnedPlacements(visible, live?.placements), live),
		[visible, live],
	);
	const underlays = useMemo(
		() => underlaysForView(props.underlays, node.view),
		[props.underlays, node.view],
	);
	const printPages = useMemo(
		() =>
			props.printPages.filter(
				(page) =>
					page.kind !== "fixture_list" &&
					page.tileId === node.id &&
					page.view === node.view &&
					page.rotationQuarterTurns === node.rotationQuarterTurns,
			),
		[props.printPages, node.id, node.view, node.rotationQuarterTurns],
	);
	const { showGrid, gridColour, gridSpacingMillimetres, showSubGrid } = settings;
	const grid = useMemo(
		() => ({
			show: showGrid,
			colour: gridColour,
			spacingMillimetres: gridSpacingMillimetres,
			subGrid: showSubGrid,
		}),
		[showGrid, gridColour, gridSpacingMillimetres, showSubGrid],
	);
	const onCamera = useCallback(
		(camera: TileCamera) => onTile(node.id, (tile) => ({ ...tile, camera })),
		[onTile, node.id],
	);
	return (
		<CadViewport
			entities={entities}
			drawings={scene.drawings}
			selectedIds={scene.selectedIds}
			preview={preview}
			view={node.view}
			rotationQuarterTurns={node.rotationQuarterTurns}
			camera={node.camera}
			showFixtureIds={settings.showFixtureIds}
			showDmxAddresses={settings.showDmxAddresses}
			showCoordinateOrigins={settings.showCoordinateOrigins}
			snapping={settings.snapToMounts && !printMode}
			grid={grid}
			printMode={printMode}
			underlays={underlays}
			onCamera={onCamera}
			onSelection={props.onSelection}
			expandSelection={props.expandSelection}
			onFocusEntity={props.onFocusEntity}
			onPreview={props.onPreview}
			onObjectMenu={printMode ? undefined : props.onObjectMenu}
			onMove={props.onMove}
			onTransforms={props.onTransforms}
			onDuplicateMove={props.onDuplicateMove}
			editEnabled={!printMode}
			printPages={printPages}
			selectedPrintPageId={props.selectedPrintPageId}
			onSelectPrintPage={props.onSelectPrintPage}
			onChangePrintPage={props.onChangePrintPage}
			documentInfo={props.documentInfo}
		/>
	);
}
