import { Button } from "../../common";
import { WindowHeader } from "../../window-kit";
import { usePatchController } from "./controller";
import { selectLayer, setFixtureNumber } from "./fixtureActions";
import { addMultipatch } from "./multipatchActions";

export function PatchHeader() {
	const controller = usePatchController();
	const { data, ui, server, appState, props } = controller;
	const selected = data.selected;
	return (
		<WindowHeader
			title="Show Patch"
			info={{
				primary: `${data.all.length} fixtures · ${data.layers.length} layers`,
				secondary:
					controller.patch.error ??
					(server.unresolvedMvrFixtures.length
						? `${server.unresolvedMvrFixtures.length} unresolved MVR fixtures excluded from output`
						: undefined),
			}}
			actions={[
				[
					...(props.onStagePreview
						? [
								{
									id: "preview-stage",
									label: "Preview Stage",
									active: props.stagePreviewOpen,
									onClick: props.onStagePreview,
								},
							]
						: []),
				],
				[
					...(props.onMedia
						? [
								{
									id: "fixtures",
									label: "Fixtures",
									active: true,
									onClick: () => undefined,
								},
								{
									id: "media",
									label: "Media Servers",
									onClick: props.onMedia,
								},
							]
						: []),
				],
				[
					...(selected && appState.patchSetArmed
						? [
								{
									id: "fixture-number",
									label: "Set fixture ID",
									onClick: () => void setFixtureNumber(controller, selected),
								},
							]
						: []),
					{
						id: "layer",
						label: "+ Add layer",
						onClick: () => ui.setLayerModal("add"),
					},
					{
						id: "fixture",
						label: "+ Add fixture",
						onClick: () => ui.setBrowserOpen(true),
					},
					{
						id: "multipatch",
						label: "+ Add multi-patch",
						disabled: !data.selected,
						onClick: () => void addMultipatch(controller),
					},
					{
						id: "delete",
						label: "Delete",
						active: ui.deleteArmed,
						disabled: data.visible.length === 0,
						onClick: () => ui.setDeleteArmed((armed) => !armed),
					},
				],
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
					<b>{layer.name}</b>
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
