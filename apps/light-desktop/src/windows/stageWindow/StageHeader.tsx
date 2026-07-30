import {
	Button,
	FormLayout,
	HorizontalFaderField,
	MultiValueToggleField,
	SelectField,
	SwitchField,
} from "@tosklight/ui";
import { WindowHeader, WindowSettings } from "@tosklight/ui/window-kit";
import { useEffect, useState } from "react";
import type { StageProjection2d } from "../../api/generated/light-wire";
import { useStageLayoutActions } from "../../features/stageLayout/StageLayoutActions";
import { useDesktopBridge } from "../../platform/desktop";
import { useApp } from "../../state/AppContext";
import type { StageLayoutModel, StageOptionsModel } from "./types";

function StageSettings({
	anchor,
	layout,
	options,
	onClose,
	writable,
}: {
	anchor: DOMRect | null;
	layout: StageLayoutModel;
	options: StageOptionsModel;
	onClose: () => void;
	writable: boolean;
}) {
	const { state, dispatch } = useApp();
	const tauri = useDesktopBridge().available;
	const actions = useStageLayoutActions();
	const [projection, setProjection] = useState(
		layout.positions2dConfig.projection,
	);
	const [regenerating, setRegenerating] = useState(false);
	const [regenerationError, setRegenerationError] = useState<string | null>(
		null,
	);
	useEffect(() => {
		if (!regenerating) setProjection(layout.positions2dConfig.projection);
	}, [layout.positions2dConfig.projection, regenerating]);
	const canRegenerate = writable && Boolean(actions?.canWrite);
	const regenerate = async () => {
		if (!actions || !canRegenerate || regenerating) return;
		setRegenerating(true);
		setRegenerationError(null);
		try {
			await actions.regenerate2d(projection);
		} catch (cause) {
			setRegenerationError(
				cause instanceof Error ? cause.message : String(cause),
			);
		} finally {
			setRegenerating(false);
		}
	};
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
							<MultiValueToggleField
								label="View"
								value={options.view}
								onChange={options.setView}
								options={[
									{ value: "2d", label: "2D" },
									{ value: "3d", label: "3D", disabled: !tauri },
								]}
							/>
							<div className="stage-2d-layout-status">
								<strong>2D layout</strong>
								<span>
									{layout.positions2dConfig.provenance === "automatic"
										? "Automatic"
										: "Manual"}{" "}
									· {PROJECTION_LABELS[layout.positions2dConfig.projection]}
								</span>
							</div>
							{canRegenerate && (
								<>
									<SelectField
										label="Regenerate from 3D"
										value={projection}
										disabled={regenerating}
										onChange={setProjection}
										options={PROJECTION_OPTIONS}
									/>
									<Button
										disabled={regenerating}
										onClick={() => void regenerate()}
									>
										{regenerating
											? "Regenerating 2D layout…"
											: "Regenerate 2D layout"}
									</Button>
								</>
							)}
							{regenerationError && (
								<div className="stage-2d-layout-error" role="alert">
									{regenerationError}
								</div>
							)}
							{options.view === "3d" && (
								<>
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
									<SwitchField
										label="Beam direction guidelines"
										offLabel="Hidden"
										onLabel="Visible"
										checked={state.stageShowBeamGuides}
										onChange={(event) =>
											dispatch({
												type: "SET_STAGE_OPTIONS",
												showBeamGuides: event.target.checked,
											})
										}
									/>
									<MultiValueToggleField
										label="Render quality"
										description="Improved beams adds feathered falloff. Up to eight highest-contributing directional sources also light opaque Stage surfaces, stop at the first opaque intersection, and cast bounded soft shadows; other sources retain their feathered volume."
										value={options.renderQuality}
										onChange={(renderQuality) =>
											dispatch({
												type: "SET_STAGE_OPTIONS",
												renderQuality,
											})
										}
										options={[
											{ value: "lines_only", label: "Lines only" },
											{
												value: "lines_and_beams",
												label: "Lines + beams",
											},
											{ value: "beams", label: "Beams" },
											{
												value: "improved_beams",
												label: "Improved beams",
											},
										]}
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
								</>
							)}
						</FormLayout>
					),
				},
			]}
		/>
	);
}

export function StageHeader({
	layout,
	options,
	selectedCount,
	writable = true,
}: {
	layout: StageLayoutModel;
	options: StageOptionsModel;
	selectedCount: number;
	writable?: boolean;
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
					layout={layout}
					options={options}
					onClose={() => setSettingsOpen(false)}
					writable={writable}
				/>
			)}
		</>
	);
}

const PROJECTION_OPTIONS: Array<{
	value: StageProjection2d;
	label: string;
}> = [
	{ value: "top_to_bottom", label: "Top to Bottom" },
	{ value: "bottom_to_top", label: "Bottom to Top" },
	{ value: "front_to_back", label: "Front to Back" },
	{ value: "back_to_front", label: "Back to Front" },
	{ value: "left_to_right", label: "Left to Right" },
	{ value: "right_to_left", label: "Right to Left" },
];

const PROJECTION_LABELS = Object.fromEntries(
	PROJECTION_OPTIONS.map(({ value, label }) => [value, label]),
) as Record<StageProjection2d, string>;
