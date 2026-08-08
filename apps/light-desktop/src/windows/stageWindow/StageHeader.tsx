import {
	Button,
	FormLayout,
	SelectField,
	SwitchField,
} from "@tosklight/ui";
import { WindowHeader, WindowSettings } from "@tosklight/ui/window-kit";
import { useState } from "react";
import { useDesktopBridge } from "../../platform/desktop";
import { useApp } from "../../state/AppContext";
import type { Stage2dSide } from "../../types";
import { StageVizSettings } from "./StageVizSettings";
import type { StageOptionsModel } from "./types";

/**
 * The Stage's settings, with one tab per view.
 *
 * The tab *is* the view. An operator opening the 3D Viz tab is asking to look at the 3D Viz
 * picture, not to read its settings while looking at something else — the old arrangement had them
 * pick a view on one tab and then find its settings on another, which is two gestures for one
 * decision and leaves the panel able to describe a view that is not on screen.
 */
function StageSettings({
	anchor,
	options,
	onClose,
}: {
	anchor: DOMRect | null;
	options: StageOptionsModel;
	onClose: () => void;
}) {
	return (
		<WindowSettings
			modal={false}
			anchor={anchor}
			title="Stage Settings"
			onClose={onClose}
			activeTab={options.view}
			onTabChange={(view) => options.setView(view as StageOptionsModel["view"])}
			tabs={[
				{
					id: "2d",
					label: "2D",
					content: (
						<FormLayout labelPlacement="side">
							<Stage2dSettings options={options} />
							<StageCommonSettings options={options} />
						</FormLayout>
					),
				},
				{
					id: "3d",
					label: "3D",
					content: (
						<FormLayout labelPlacement="side">
							<Stage3dSettings />
							<StageCommonSettings options={options} />
						</FormLayout>
					),
				},
				{
					id: "3d-viz",
					label: "3D Viz",
					content: (
						<FormLayout labelPlacement="side">
							<StageVizSettings />
							<StageCommonSettings options={options} />
						</FormLayout>
					),
				},
			]}
		/>
	);
}

/** What every view has, whichever one is on screen. */
function StageCommonSettings({ options }: { options: StageOptionsModel }) {
	const { state, dispatch } = useApp();
	return (
		<>
			<SwitchField
				label="Group shortcuts"
				offLabel="Hidden"
				onLabel="Visible"
				checked={options.groupsVisible}
				onChange={(event) =>
					dispatch({
						type: "SET_STAGE_OPTIONS",
						groupsVisible: event.target.checked,
					})
				}
			/>
			<SwitchField
				label="Show selection"
				offLabel="Hidden"
				onLabel="Visible"
				checked={state.stageShowSelection}
				onChange={(event) =>
					dispatch({
						type: "SET_STAGE_OPTIONS",
						showSelection: event.target.checked,
					})
				}
			/>
		</>
	);
}

/**
 * The 2D Stage, which is the renderer's plan of the rig seen from one side.
 *
 * There is no saved arrangement to regenerate any more. A 2D Stage *is* the projection of where
 * the fixtures actually are, so the only question it can be asked is which side to project from —
 * and the answer takes effect on the picture rather than replacing anything stored in the show.
 */
function Stage2dSettings({ options }: { options: StageOptionsModel }) {
	return (
		<SelectField
			label="Viewed from"
			description="Where the operator is standing to look at the rig. The plan is drawn from the fixtures' own positions, so changing this changes the picture and nothing in the show."
			value={options.side2d}
			onChange={options.setSide2d}
			options={SIDE_OPTIONS}
		/>
	);
}

/**
 * The 3D Stage, which is an outline diagram rather than a picture of light.
 *
 * Deliberately almost empty. This view draws boxes and aim lines and simulates no light at all, so
 * a render style and an environment brightness would both be controls over something that is not
 * happening — and the beam guidelines are not offered either, because here they are the picture
 * rather than an addition to it.
 */
function Stage3dSettings() {
	const { state, dispatch } = useApp();
	const bridge = useDesktopBridge();
	return (
		<>
			{/*
			 * Framing is asked of the renderer rather than described to it. Only the renderer knows
			 * how big the rig is, so a desk naming a camera position would be guessing at the one
			 * number the answer depends on — which is why this used to set a zoom and an orbit that
			 * nothing had read since the renderer took over the picture, and so did nothing at all.
			 */}
			<Button onClick={() => void bridge.sendStagePaneInput("frame", 0, 0)}>
				Reset view
			</Button>
			<SwitchField
				label="Floor grid"
				offLabel="Hidden"
				onLabel="Visible"
				checked={state.stageShowFloorGrid}
				onChange={(event) =>
					dispatch({
						type: "SET_STAGE_OPTIONS",
						showFloorGrid: event.target.checked,
					})
				}
			/>
		</>
	);
}

export function StageHeader({
	options,
	selectedCount,
}: {
	options: StageOptionsModel;
	selectedCount: number;
}) {
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	const [settingsOpen, setSettingsOpen] = useState(false);
	return (
		<>
			<WindowHeader
				title="Stage"
				info={{
					primary: `${selectedCount} selected`,
					secondary:
						"Tap to select · Shift for range · Control/Command tracks macro",
				}}
				actions={[
					[
						{
							id: "follow",
							label: "Follow Preload",
							active: options.followPreload,
							onClick: options.toggleFollowPreload,
						},
					],
					[
						{
							id: "select",
							label: "Select fixtures",
							active: options.mode === "select",
							onClick: () => options.setMode("select"),
						},
						{
							id: "navigate",
							label: "Navigate",
							active: options.mode === "navigate",
							onClick: () => options.setMode("navigate"),
						},
					],
				]}
				settings
				onSettings={(anchor) => {
					setSettingsAnchor(anchor.getBoundingClientRect());
					setSettingsOpen(true);
				}}
			/>
			{settingsOpen && (
				<StageSettings
					anchor={settingsAnchor}
					options={options}
					onClose={() => setSettingsOpen(false)}
				/>
			)}
		</>
	);
}

const SIDE_OPTIONS: Array<{ value: Stage2dSide; label: string }> = [
	{ value: "top", label: "Above · plan" },
	{ value: "front", label: "Front · from the house" },
	{ value: "back", label: "Back · from upstage" },
	{ value: "left", label: "House left · stage right" },
	{ value: "right", label: "House right · stage left" },
];
