import { Button } from "@tosklight/ui";
import { WindowHeader } from "@tosklight/ui/window-kit";
import { useState } from "react";
import { isVisualOnly } from "../patchUtils";
import { usePatchController } from "./controller";
import { selectLayer, setFixtureNumber } from "./fixtureActions";
import { addMultipatch } from "./multipatchActions";
import { ShowPatchSettings } from "./ShowPatchSettings";
import { type ShowPatchView, showPatchViewGroup } from "./showPatchHeader";

export function PatchHeader() {
	const controller = usePatchController();
	const { data, ui, server, appState, props } = controller;
	const selected = data.selected;
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	return (
		<>
			<WindowHeader
				title="Show Patch"
				settings={!props.compact}
				onSettings={(anchor) =>
					setSettingsAnchor(anchor.getBoundingClientRect())
				}
				info={{
					primary: `${data.all.length} fixtures · ${data.layers.length} layers`,
					secondary:
						controller.patch.error ??
						(server?.unresolvedMvrFixtures.length
							? `${server.unresolvedMvrFixtures.length} unresolved MVR fixtures excluded from output`
							: undefined),
				}}
				groups={[
					{
						id: "stage-renderer",
						actions: [
							...(props.onOpenStageWindow
								? [
										{
											id: "open-stage-renderer",
											label: "Open Stage Renderer",
											onPress: props.onOpenStageWindow,
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
								// A Venue object is placed one object at a time; it has no copies.
								disabled: !selected || isVisualOnly(selected.definition),
								onPress: () => void addMultipatch(controller),
							},
						],
					},
					{
						id: "patch-edit",
						actions: [
							{
								id: "delete",
								label: "Delete",
								active: ui.deleteArmed,
								disabled: data.visible.length === 0,
								onPress: () => ui.setDeleteArmed((armed) => !armed),
							},
							...(selected && appState.patchSetArmed
								? [
										{
											id: "fixture-number",
											label: "Set fixture ID",
											onPress: () =>
												void setFixtureNumber(controller, selected),
										},
									]
								: []),
						],
					},
					...(props.onMedia || props.onTracking
						? [
								showPatchViewGroup(
									"fixtures",
									[
										"fixtures",
										...(props.onMedia ? (["media"] as const) : []),
										...(props.onTracking ? (["tracking"] as const) : []),
									] satisfies ShowPatchView[],
									(view) => {
										if (view === "media") props.onMedia?.();
										if (view === "tracking") props.onTracking?.();
									},
								),
							]
						: []),
				]}
			/>
			{settingsAnchor ? (
				<ShowPatchSettings
					anchor={settingsAnchor}
					onClose={() => setSettingsAnchor(null)}
					onImportCsv={() => ui.setCsvImportOpen(true)}
				/>
			) : null}
		</>
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
