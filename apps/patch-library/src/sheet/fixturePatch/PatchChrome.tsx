import { Button, SwitchField } from "@tosklight/ui";
import { WindowHeader } from "@tosklight/ui/window-kit";
import { usePatchController } from "./controller";
import { selectLayer, setFixtureNumber } from "./fixtureActions";
import { addMultipatch } from "./multipatchActions";

export function PatchHeader() {
	const controller = usePatchController();
	const { data, ui, library, editArmed, props } = controller;
	const selected = data.selected;
	return (
		<WindowHeader
			title={props.title}
			info={{
				primary: `${data.scoped.length} fixtures · ${data.layers.length} layers`,
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
					<b>{layer.name}</b>
					<span>
						{
							data.scoped.filter(
								(fixture) => (fixture.layer_id || "default") === layer.id,
							).length
						}
					</span>
				</Button>
			))}
		</aside>
	);
}
