import {
	Button,
	ColorPickerField,
	HorizontalFaderField,
	MultiValueToggleField,
	SwitchField,
} from "@tosklight/ui";
import { useEffect, useState } from "react";
import { useDesktopBridge } from "../../platform/desktop";
import type { StagePanePicture } from "../../platform/desktop/types";
import { useApp } from "../../state/AppContext";

/**
 * The 3D Viz view's own settings.
 *
 * This picture is drawn by the renderer, in its own process, so these are its settings rather than
 * the desk's — haze, environment brightness, exposure. They are sent to it as the operator moves
 * them; nothing here is drawn by the desk.
 *
 * The pane's geometry is deliberately absent. Where the Stage is and how big it is comes from the
 * layout, and an operator moving a pane is already saying it.
 */
export function StageVizSettings({ paneId }: { paneId?: string } = {}) {
	const { state, dispatch } = useApp();
	const bridge = useDesktopBridge();
	const [trouble, setTrouble] = useState<string | null>(null);
	const pane = paneId
		? state.desks
				.find((desk) => desk.id === state.activeDeskId)
				?.panes.find((candidate) => candidate.id === paneId)
		: undefined;
	const fogControl = (
		option:
			| "lampFogCloudiness"
			| "lampFogTurbulence"
			| "laserFogCloudiness"
			| "laserFogTurbulence",
		value: number,
	) => {
		if (paneId)
			dispatch({ type: "SET_PANE_FOG_VARIATION", id: paneId, option, value });
	};

	/*
	 * Only what went wrong. Which GPU answered and which transport the picture came over is the
	 * renderer's business, not the operator's: a Stage that is drawing correctly raises no
	 * question that naming the adapter answers, and a line of hardware detail sitting under the
	 * settings is one more thing to read past every time. A failure still says so, because that
	 * is something an operator can act on.
	 */
	useEffect(() => {
		let cancelled = false;
		const poll = async () => {
			const [, detail] = await bridge.stagePaneStatus(paneId);
			if (cancelled) return;
			setTrouble(detail);
		};
		void poll();
		const timer = window.setInterval(() => void poll(), 2_000);
		return () => {
			cancelled = true;
			window.clearInterval(timer);
		};
	}, [bridge, paneId]);

	/*
	 * Nothing is sent from here. The picture crosses whenever it changes and whenever a renderer
	 * starts, from the one place that watches all of it — sending from each control as well would
	 * mean two senders racing over the same settings.
	 */
	return (
		<>
			<Button
				onClick={() => void bridge.sendStagePaneInput("frame", 0, 0, paneId)}
			>
				Reset view
			</Button>
			{/*
			 * No beam guidelines here. This view draws the beams themselves, and a dotted line
			 * down the middle of a beam that is already on screen says nothing the beam did not.
			 * They belong to the 3D view, which draws no beams and where they are the picture.
			 */}
			<HorizontalFaderField
				label="Fog / haze"
				description="How much haze the beams are drawn through. A beam is only visible in something."
				value={state.stageVizAtmosphere}
				minimum={0}
				maximum={1}
				step={0.01}
				display={`${Math.round(state.stageVizAtmosphere * 100)}%`}
				onChange={(vizAtmosphere) => {
					dispatch({ type: "SET_STAGE_OPTIONS", vizAtmosphere });
				}}
			/>
			<HorizontalFaderField
				label="Environment brightness"
				description="How brightly everything that is not a light source is lit. At zero the rig is visible only where a fixture puts light on it."
				value={state.stageEnvironmentBrightness}
				minimum={0}
				maximum={2}
				step={0.05}
				display={`${Math.round(state.stageEnvironmentBrightness * 100)}%`}
				onChange={(environmentBrightness) => {
					dispatch({ type: "SET_STAGE_OPTIONS", environmentBrightness });
				}}
			/>
			<MultiValueToggleField
				label="Render quality"
				description="How much the renderer is asked to do per frame. Ultra is the most expensive and the least forgiving of a busy machine."
				value={state.stageVizQuality}
				onChange={(vizQuality) => {
					dispatch({ type: "SET_STAGE_OPTIONS", vizQuality });
				}}
				options={[
					{ value: "draft", label: "Draft" },
					{ value: "standard", label: "Standard" },
					{ value: "high", label: "High" },
					{ value: "ultra", label: "Ultra" },
				]}
			/>
			{paneId && state.stageVizQuality === "ultra" && (
				<>
					<HorizontalFaderField
						label="Lamp fog cloudiness"
						description="How uneven and patchy lamp haze is in Ultra. Zero is spatially uniform."
						value={pane?.lampFogCloudiness ?? 0.7}
						minimum={0}
						maximum={1}
						step={0.01}
						display={`${Math.round((pane?.lampFogCloudiness ?? 0.7) * 100)}%`}
						onChange={(value) => fogControl("lampFogCloudiness", value)}
					/>
					<HorizontalFaderField
						label="Lamp fog turbulence"
						description="How quickly lamp-haze patches move and change in Ultra. Zero is stationary."
						value={pane?.lampFogTurbulence ?? 1}
						minimum={0}
						maximum={1}
						step={0.01}
						display={`${Math.round((pane?.lampFogTurbulence ?? 1) * 100)}%`}
						onChange={(value) => fogControl("lampFogTurbulence", value)}
					/>
					<HorizontalFaderField
						label="Laser fog cloudiness"
						description="How uneven and patchy laser haze is in Ultra. Zero preserves uniform laser haze."
						value={pane?.laserFogCloudiness ?? 0}
						minimum={0}
						maximum={1}
						step={0.01}
						display={`${Math.round((pane?.laserFogCloudiness ?? 0) * 100)}%`}
						onChange={(value) => fogControl("laserFogCloudiness", value)}
					/>
					<HorizontalFaderField
						label="Laser fog turbulence"
						description="How quickly laser-haze patches move and change in Ultra. Zero is stationary."
						value={pane?.laserFogTurbulence ?? 0}
						minimum={0}
						maximum={1}
						step={0.01}
						display={`${Math.round((pane?.laserFogTurbulence ?? 0) * 100)}%`}
						onChange={(value) => fogControl("laserFogTurbulence", value)}
					/>
				</>
			)}
			<HorizontalFaderField
				label="Exposure"
				description="What the whole picture is scaled by before it is shown, as a camera's exposure would."
				value={state.stageVizExposure}
				minimum={0.05}
				maximum={4}
				step={0.05}
				display={`${state.stageVizExposure.toFixed(2)}×`}
				onChange={(vizExposure) => {
					dispatch({ type: "SET_STAGE_OPTIONS", vizExposure });
				}}
			/>
			<HorizontalFaderField
				label="Laser brightness"
				description="Lasers have no honest reference — how strong a beam looks depends on the haze, the room and the eye — so it is the operator's, like the fog."
				value={state.stageVizLaserBrightness}
				minimum={0}
				maximum={4}
				step={0.05}
				display={`${state.stageVizLaserBrightness.toFixed(2)}×`}
				onChange={(vizLaserBrightness) => {
					dispatch({ type: "SET_STAGE_OPTIONS", vizLaserBrightness });
				}}
			/>
			{/*
			 * The colour behind the rig, which is the room rather than the show. Very dark and blue
			 * by default: a stage seen from the house is never black and never grey. It applies to
			 * every renderer-drawn Stage, because it is one room.
			 */}
			<ColorPickerField
				label="Background"
				description="The colour behind the rig, in every Stage view."
				value={state.stageVizBackground}
				colors={STAGE_BACKGROUNDS}
				onChange={(vizBackground) =>
					dispatch({ type: "SET_STAGE_OPTIONS", vizBackground })
				}
			/>
			<SwitchField
				label="Fixture labels"
				offLabel="Hidden"
				onLabel="Visible"
				checked={state.stageVizShowLabels}
				onChange={(event) => {
					dispatch({
						type: "SET_STAGE_OPTIONS",
						vizShowLabels: event.target.checked,
					});
				}}
			/>
			{trouble && (
				<div className="stage-viz-trouble" role="alert">
					{trouble}
				</div>
			)}
		</>
	);
}

/*
 * The colours a room behind a rig is worth being.
 *
 * The picker's usual palette is for gels and pool buttons — saturated, and the wrong question
 * entirely here. What this chooses is how dark the room is and which way it leans, so the swatches
 * run from black through the dark blues and greys a stage is actually seen against. Nothing bright:
 * a light background does not make a rig easier to read, it makes the beams impossible.
 */
const STAGE_BACKGROUNDS = [
	"#000000",
	"#020304",
	"#04060a",
	"#070a12",
	"#0a0e18",
	"#0d1220",
	"#101728",
	"#141c30",
	"#050505",
	"#0a0a0a",
	"#101010",
	"#161616",
	"#040706",
	"#061012",
	"#0a1416",
	"#12100a",
] as const;
