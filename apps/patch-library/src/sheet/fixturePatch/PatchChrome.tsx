import { Button } from "@tosklight/ui";
import { WindowHeader } from "@tosklight/ui/window-kit";
import { usePatchController } from "./controller";
import {
	selectLayer,
	setFixtureNumber,
	toggleLayerLock,
} from "./fixtureActions";
import { addMultipatch } from "./multipatchActions";

export function PatchHeader() {
	const controller = usePatchController();
	const { data, ui, library, editArmed, props } = controller;
	const selected = data.selected;
	const activeLayer = data.layers.find((layer) => layer.id === ui.activeLayer);
	return (
		<WindowHeader
			title={props.title}
			info={{
				primary: `${data.all.length} fixtures · ${data.layers.length} layers`,
				secondary:
					controller.patch.error ??
					(library?.unresolvedMvrFixtures.length
						? `${library.unresolvedMvrFixtures.length} unresolved MVR fixtures excluded from output`
						: undefined),
			}}
			groups={[
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
						{
							id: "multipatch",
							label: "+ Add multi-patch",
							disabled: !data.selected,
							onPress: () => void addMultipatch(controller),
						},
					],
				},
				{
					id: "patch-edit",
					actions: [
						...(activeLayer && ui.layerModal !== "select"
							? [
									{
										id: "layer-lock",
										label: activeLayer.locked ? "Unlock Layer" : "Lock Layer",
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
	);
}

export function PatchLayers() {
	const controller = usePatchController();
	const { data, ui } = controller;
	return (
		<aside className="patch-layers">
			<h3>{ui.layerModal === "select" ? "Select layer" : "Layers"}</h3>
			<Button
				className={ui.activeLayer === "all" ? "active" : ""}
				onClick={() =>
					ui.layerModal === "select" ? undefined : ui.setActiveLayer("all")
				}
			>
				<b>All fixtures</b>
				<span>{data.all.length}</span>
			</Button>
			{data.layers.map((layer) => (
				<Button
					key={layer.id}
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
							data.all.filter(
								(fixture) => (fixture.layer_id || "default") === layer.id,
							).length
						}
					</span>
				</Button>
			))}
		</aside>
	);
}
