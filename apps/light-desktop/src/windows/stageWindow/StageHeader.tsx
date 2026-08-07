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
import type { StageProjection2d } from "../../api/client/stageLayout";
import { useStageLayoutActions } from "../../features/stageLayout/StageLayoutActions";
import { useDesktopBridge } from "../../platform/desktop";
import { useApp } from "../../state/AppContext";
import type { AppState } from "../../types";
import { StageVizSettings } from "./StageVizSettings";
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
	const bridge = useDesktopBridge();
	const tauri = bridge.available;
	// The renderer draws in its own process into the desk's window, which a browser cannot host.
	const [vizAvailable, setVizAvailable] = useState(false);
	useEffect(() => {
		let cancelled = false;
		void bridge
			.stagePaneAvailable()
			.then((available) => {
				if (!cancelled) setVizAvailable(available);
			})
			.catch(() => undefined);
		return () => {
			cancelled = true;
		};
	}, [bridge]);
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
									{
										value: "3d-viz",
										label: "3D Viz",
										disabled: !vizAvailable,
									},
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
							{options.view === "2d" && (
								<Stage2dSettings
									layout={layout}
									projection={projection}
									setProjection={setProjection}
									canRegenerate={canRegenerate}
									regenerating={regenerating}
									regenerationError={regenerationError}
									regenerate={regenerate}
								/>
							)}
							{options.view === "3d" && (
								<Stage3dSettings options={options} conesAvailable={!vizAvailable} />
							)}
							{options.view === "3d-viz" && <StageVizSettings />}
						</FormLayout>
					),
				},
			]}
		/>
	);
}

/**
 * The three styles an operator picks between, over the five the state can hold.
 *
 * Saved layouts carry `lines_and_beams` and `beams` from before the picker had three entries, and
 * a show that chose one keeps drawing it — it simply reads as the nearest of the three here.
 */
type RenderStyle = "none" | "lines" | "cones";

const RENDER_STYLE_VALUES: Record<RenderStyle, AppState["stageRenderQuality"]> = {
	none: "none",
	lines: "lines_only",
	cones: "improved_beams",
};

function renderStyle(quality: AppState["stageRenderQuality"]): RenderStyle {
	if (quality === "none") return "none";
	return quality === "lines_only" ? "lines" : "cones";
}

const DETAIL_LABELS: Record<StageOptionsModel["view"], string> = {
	"2d": "2D",
	"3d": "3D",
	"3d-viz": "3D Viz",
};

/** The 2D layout, which has nothing to say once the Stage is showing a 3D view. */
function Stage2dSettings({
	layout,
	projection,
	setProjection,
	canRegenerate,
	regenerating,
	regenerationError,
	regenerate,
}: {
	layout: StageLayoutModel;
	projection: StageProjection2d;
	setProjection: (value: StageProjection2d) => void;
	canRegenerate: boolean;
	regenerating: boolean;
	regenerationError: string | null;
	regenerate: () => Promise<void>;
}) {
	return (
		<>
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
					<Button disabled={regenerating} onClick={() => void regenerate()}>
						{regenerating ? "Regenerating 2D layout…" : "Regenerate 2D layout"}
					</Button>
				</>
			)}
			{regenerationError && (
				<div className="stage-2d-layout-error" role="alert">
					{regenerationError}
				</div>
			)}
		</>
	);
}

function Stage3dSettings({
	options,
	conesAvailable,
}: {
	options: StageOptionsModel;
	conesAvailable: boolean;
}) {
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
			<SwitchField
				label="Beam Guidelines"
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
				label="Render Style"
				description={
					conesAvailable
						? "No Beams still lights the lenses and moves the heads; it draws nothing leaving them. Cones are volumetric and the most expensive."
						: "No Beams still lights the lenses and moves the heads; it draws nothing leaving them. Cones are drawn by the 3D Viz view on this machine."
				}
				value={renderStyle(options.renderQuality)}
				onChange={(style) =>
					dispatch({
						type: "SET_STAGE_OPTIONS",
						renderQuality: RENDER_STYLE_VALUES[style],
					})
				}
				options={[
					{ value: "none", label: "No Beams" },
					{ value: "lines", label: "Lines" },
					// The desk's own cones are the expensive path, and where the renderer can draw
					// this Stage it draws them better. Offering both would be offering the worse one.
					{ value: "cones", label: "Cones", disabled: !conesAvailable },
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
