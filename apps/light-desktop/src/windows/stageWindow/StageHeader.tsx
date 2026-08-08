import {
	Button,
	FormLayout,
	MultiValueToggleField,
	SelectField,
	SwitchField,
} from "@tosklight/ui";
import { WindowHeader, WindowSettings } from "@tosklight/ui/window-kit";
import { useState } from "react";
import { useApp } from "../../state/AppContext";
import type { Stage2dSide } from "../../types";
import { StageVizSettings } from "./StageVizSettings";
import type { StageOptionsModel } from "./types";

function StageSettings({
	anchor,
	options,
	onClose,
}: {
	anchor: DOMRect | null;
	options: StageOptionsModel;
	onClose: () => void;
}) {
	const { state, dispatch } = useApp();
	return (
		<WindowSettings
			modal={false}
			anchor={anchor}
			title="Stage Settings"
			onClose={onClose}
			tabs={[
				{
					id: "stage",
					label: "Stage",
					content: (
						<FormLayout labelPlacement="side">
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
							{/*
							 * None of the three is disabled where the renderer is missing. All
							 * three are its picture now, so disabling would mean disabling the
							 * Stage entirely — and an operator setting up a show on a machine
							 * that cannot draw one still has a view to choose for the machine it
							 * will run on. The pane says what it cannot do instead.
							 */}
							<MultiValueToggleField
								label="View"
								value={options.view}
								onChange={options.setView}
								options={[
									{ value: "2d", label: "2D" },
									{ value: "3d", label: "3D" },
									{ value: "3d-viz", label: "3D Viz" },
								]}
							/>
						</FormLayout>
					),
				},
				{
					id: "detail",
					// Named for what it holds rather than "Advanced": the operator switched the view
					// on the first tab, and this is that view's own settings.
					label: DETAIL_LABELS[options.view],
					content: (
						<FormLayout labelPlacement="side">
							{options.view === "2d" && <Stage2dSettings options={options} />}
							{options.view === "3d" && <Stage3dSettings />}
							{options.view === "3d-viz" && <StageVizSettings />}
						</FormLayout>
					),
				},
			]}
		/>
	);
}

const DETAIL_LABELS: Record<StageOptionsModel["view"], string> = {
	"2d": "2D",
	"3d": "3D",
	"3d-viz": "3D Viz",
};

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
	return (
		<>
			<Button
				onClick={() =>
					dispatch({
						type: "SET_STAGE_NAVIGATION",
						zoom: 1,
						panX: 0,
						panY: 0,
						orbitX: 0,
						orbitY: 0,
					})
				}
			>
				Reset 3D view
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
	{ value: "left", label: "Left · from stage left" },
	{ value: "right", label: "Right · from stage right" },
];
