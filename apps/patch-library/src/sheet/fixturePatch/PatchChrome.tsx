import { isVisualOnly } from "../patchUtils";
import { Button, SwitchField } from "@tosklight/ui";
import { WindowHeader, WindowSettings } from "@tosklight/ui/window-kit";
import { useState } from "react";
import { TrashIcon } from "../../library/trashIcon";
import type { PatchLayer } from "../../wire";
import {
	NO_LAYER_ID,
	type PatchController,
	usePatchController,
} from "./controller";
import {
	selectLayer,
	setFixtureNumber,
	toggleLayerLock,
	toggleLayerVisibility,
} from "./fixtureActions";
import { addMultipatch } from "./multipatchActions";
import { DeleteLayerConfirm } from "./PatchDialogs";
import {
	activeQuickView,
	PATCH_QUICK_VIEWS,
	PATCH_SHEET_COLUMNS,
	quickViewHiddenColumns,
} from "./patchColumns";

export function PatchHeader() {
	const controller = usePatchController();
	const { data, ui, editArmed, props } = controller;
	const selected = data.selected;
	const activeLayer = data.layers.find((layer) => layer.id === ui.activeLayer);
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	return (
		<>
		<WindowHeader
			title={props.title}
			settings
			onSettings={(anchor) => setSettingsAnchor((open) => (open ? null : anchor.getBoundingClientRect()))}
			info={patchHeaderInfo(controller)}
			groups={[
				quickViewGroup(controller),
				{
					id: "stage-preview",
					actions: [
						...(props.onStagePreview
							? [
									{
										id: "preview-stage",
										label: "Preview Stage",
										active: props.stagePreviewOpen,
										onPress: props.onStagePreview,
										onLongPress: props.onOpenStageWindow,
									},
								]
							: []),
					],
				},
				{
					id: "patch-kind",
					actions: [
						...(props.onMedia
							? [
									{
										id: "fixtures",
										label: "Fixtures",
										active: true,
										onPress: () => undefined,
									},
									{
										id: "media",
										label: "Media Servers",
										onPress: props.onMedia,
									},
								]
							: []),
					],
				},
				{
					id: "patch-create",
					actions: [
						{
							id: "layer",
							label: "+ Add layer",
							onPress: () => ui.setLayerModal("add"),
						},
						{
							id: "fixture",
							label: "+ Add fixture",
							onPress: () => ui.setBrowserOpen(true),
						},
						// A Venue object is placed one object at a time and has no copies, so the Venue screen does
						// not offer them at all.
						...(props.scope === "venue"
							? []
							: [
									{
										id: "multipatch",
										label: "+ Add multi-patch",
										disabled: !data.selected || isVisualOnly(data.selected.definition),
										onPress: () => void addMultipatch(controller),
									},
								]),
					],
				},
				{
					id: "patch-edit",
					actions: [
						...(activeLayer && ui.layerModal !== "select"
							? [
									{
										id: "layer-visible-2d",
										label: (activeLayer.visible2d ?? true)
											? "Hide in 2D"
											: "Show in 2D",
										onPress: () =>
											void toggleLayerVisibility(
												controller,
												activeLayer.id,
												"2d",
											),
									},
									{
										id: "layer-visible-3d",
										label: (activeLayer.visible3d ?? true)
											? "Hide in 3D"
											: "Show in 3D",
										onPress: () =>
											void toggleLayerVisibility(
												controller,
												activeLayer.id,
												"3d",
											),
									},
									{
										id: "layer-lock",
										label: activeLayer.locked
											? "Unlock Layer"
											: "Lock Layer",
										onPress: () =>
											void toggleLayerLock(controller, activeLayer.id),
									},
								]
							: []),
						...(selected && editArmed
							? [
									{
										id: "fixture-number",
										label: "Set fixture ID",
										onPress: () => void setFixtureNumber(controller, selected),
									},
								]
							: []),
						{
							id: "delete",
							label: "Delete",
							active: ui.deleteArmed,
							disabled: data.visible.length === 0,
							onPress: () => ui.setDeleteArmed((armed) => !armed),
						},
					],
				},
			]}
		/>
		{settingsAnchor ? (
			<PatchColumnSettings anchor={settingsAnchor} onClose={() => setSettingsAnchor(null)} />
		) : null}
		</>
	);
}

function patchHeaderInfo({ data, patch, library }: PatchController) {
	const unresolved = library?.unresolvedMvrFixtures.length;
	return {
		primary: `${data.scoped.length} fixtures · ${data.layers.length} layers`,
		secondary:
			patch.error ??
			(unresolved ? `${unresolved} unresolved MVR fixtures excluded from output` : undefined),
	};
}

/** The Architect's column quick views; a host that does not offer them gets an empty group. */
function quickViewGroup({ columns, props }: PatchController) {
	const active = activeQuickView(columns.hiddenColumns);
	return {
		id: "patch-views",
		actions: props.quickViews
			? PATCH_QUICK_VIEWS.map((view) => ({
					id: `view-${view.id}`,
					label: view.label,
					active: active?.id === view.id,
					onPress: () => columns.setHiddenColumns(quickViewHiddenColumns(view)),
				}))
			: [],
	};
}

function PatchColumnSettings({ anchor, onClose }: { anchor: DOMRect; onClose: () => void }) {
	return (
		<WindowSettings
			modal={false}
			anchor={anchor}
			title={usePatchController().props.title}
			onClose={onClose}
			tabs={[
				{
					id: "columns",
					label: "Columns",
					content: (
						<section>
							<h3>Visible columns</h3>
							<PatchColumnSwitches />
						</section>
					),
				},
			]}
		/>
	);
}

/** One switch per column. The last column still shown cannot be switched off. */
function PatchColumnSwitches() {
	const { hiddenColumns, setHiddenColumns } = usePatchController().columns;
	const shown = PATCH_SHEET_COLUMNS.length - hiddenColumns.size;
	return (
		<div className="patch-column-options">
			{PATCH_SHEET_COLUMNS.map(({ id, label }) => {
				const visible = !hiddenColumns.has(id);
				return (
					<SwitchField
						key={id}
						label={label}
						offLabel="Hidden"
						onLabel="Visible"
						checked={visible}
						disabled={visible && shown === 1}
						onChange={(event) =>
							setHiddenColumns(
								event.target.checked
									? [...hiddenColumns].filter((column) => column !== id)
									: [...hiddenColumns, id],
							)
						}
					/>
				);
			})}
		</div>
	);
}

export function PatchLayers() {
	const controller = usePatchController();
	const { data, ui } = controller;
	const [deleting, setDeleting] = useState<PatchLayer | null>(null);
	const canDelete = (layer: PatchLayer) =>
		Boolean(controller.library?.deletePatchLayer) &&
		layer.id !== "default" &&
		ui.layerModal !== "select";
	return (
		<aside className="patch-layers">
			<div className="patch-layers-title">
				<h3>{ui.layerModal === "select" ? "Select layer" : "Layers"}</h3>
				<SwitchField
					label="Show all"
					aria-label="Show all layers"
					offLabel=""
					onLabel=""
					checked={ui.showAllLayers}
					onChange={(event) => ui.setShowAllLayers(event.target.checked)}
				/>
			</div>
			<Button
				className={ui.activeLayer === "all" ? "active" : ""}
				onClick={() =>
					ui.layerModal === "select" ? undefined : ui.setActiveLayer("all")
				}
			>
				<b>All fixtures</b>
				<span>{data.scoped.length}</span>
			</Button>
			{data.showUnassigned ? (
				<Button
					className={`patch-layer-unassigned ${ui.activeLayer === NO_LAYER_ID ? "active" : ""}`.trim()}
					onClick={() =>
						ui.layerModal === "select"
							? void selectLayer(controller, "default")
							: ui.setActiveLayer(NO_LAYER_ID)
					}
				>
					<span className="patch-layer-copy">
						<b>No Layer Assigned</b>
					</span>
					<span>{data.unassigned.length}</span>
				</Button>
			) : null}
			{data.layers.map((layer) => (
				<div
					key={layer.id}
					className={`patch-layer-row${canDelete(layer) ? " has-delete" : ""}`}
				>
					<Button
						className={ui.activeLayer === layer.id ? "active" : ""}
						onClick={() =>
							ui.layerModal === "select"
								? void selectLayer(controller, layer.id)
								: ui.setActiveLayer(layer.id)
						}
					>
						<span className="patch-layer-copy">
							<b>{layer.name}</b>
							{layer.locked ? <small>Layer Locked</small> : null}
						</span>
						<span>
							{
								data.scoped.filter(
									(fixture) => (fixture.layer_id || "default") === layer.id,
								).length
							}
						</span>
					</Button>
					{canDelete(layer) ? (
						<Button
							className="patch-layer-delete"
							aria-label={`Delete layer ${layer.name}`}
							onClick={() => setDeleting(layer)}
						>
							<TrashIcon />
						</Button>
					) : null}
				</div>
			))}
			{deleting ? (
				<DeleteLayerConfirm layer={deleting} onClose={() => setDeleting(null)} />
			) : null}
		</aside>
	);
}
