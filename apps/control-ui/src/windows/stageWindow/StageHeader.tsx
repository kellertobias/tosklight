import { useState } from "react";
import {
	FormLayout,
	HorizontalFaderField,
	MultiValueToggleField,
	SwitchField,
} from "../../components/common";
import { WindowHeader, WindowSettings } from "../../components/window-kit";
import { useDesktopBridge } from "../../platform/desktop";
import { useApp } from "../../state/AppContext";
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
	const tauri = useDesktopBridge().available;
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
							<MultiValueToggleField
								label="View"
								value={options.view}
								onChange={options.setView}
								options={[
									{ value: "2d", label: "2D" },
									{ value: "3d", label: "3D", disabled: !tauri },
								]}
							/>
							<SwitchField
								label="Groups shortcuts"
								checked={options.groupsVisible}
								onChange={(event) =>
									dispatch({
										type: "SET_STAGE_OPTIONS",
										groupsVisible: event.target.checked,
									})
								}
							/>
							<SwitchField
								label="Show Selection"
								checked={state.stageShowSelection}
								onChange={(event) =>
									dispatch({
										type: "SET_STAGE_OPTIONS",
										showSelection: event.target.checked,
									})
								}
							/>
							<SwitchField
								label="Floor grid"
								checked={state.stageShowFloorGrid}
								onChange={(event) =>
									dispatch({
										type: "SET_STAGE_OPTIONS",
										showFloorGrid: event.target.checked,
									})
								}
							/>
							<SwitchField
								label="Beam direction guides"
								checked={state.stageShowBeamGuides}
								onChange={(event) =>
									dispatch({
										type: "SET_STAGE_OPTIONS",
										showBeamGuides: event.target.checked,
									})
								}
							/>
							<HorizontalFaderField
								label="Environment brightness"
								value={state.stageEnvironmentBrightness}
								minimum={0}
								maximum={2}
								step={0.05}
								display={`${Math.round(state.stageEnvironmentBrightness * 100)}%`}
								onChange={(environmentBrightness) =>
									dispatch({
										type: "SET_STAGE_OPTIONS",
										environmentBrightness,
									})
								}
							/>
						</FormLayout>
					),
				},
			]}
		/>
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
					...(options.mode === "setup"
						? []
						: [
								[
									{
										id: "follow",
										label: "Follow Preload",
										active: options.followPreload,
										onClick: options.toggleFollowPreload,
									},
								],
							]),
					[
						{
							id: "select",
							label: "Select fixtures",
							active: options.mode === "select",
							onClick: () => options.setMode("select"),
						},
						{
							id: "setup",
							label: "Setup positions",
							active: options.mode === "setup",
							onClick: () => options.setMode("setup"),
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
