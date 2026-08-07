import { HorizontalFaderField, SwitchField } from "@tosklight/ui";
import { useEffect, useState } from "react";
import { useDesktopBridge } from "../../platform/desktop";
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
export function StageVizSettings() {
	const { state, dispatch } = useApp();
	const bridge = useDesktopBridge();
	const [renderer, setRenderer] = useState<string | null>(null);
	const [trouble, setTrouble] = useState<string | null>(null);

	// What is drawing and how the picture reaches the desk. An operator asking why the Stage is
	// slow should not have to guess which GPU answered or which transport it is on.
	useEffect(() => {
		let cancelled = false;
		const poll = async () => {
			const [description, detail] = await bridge.stagePaneStatus();
			if (cancelled) return;
			setRenderer(description);
			setTrouble(detail);
		};
		void poll();
		const timer = window.setInterval(() => void poll(), 2_000);
		return () => {
			cancelled = true;
			window.clearInterval(timer);
		};
	}, [bridge]);

	const send = (atmosphere: number, ambient: number) => {
		void bridge.setStagePanePicture(atmosphere, ambient);
	};

	return (
		<>
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
					send(vizAtmosphere, state.stageEnvironmentBrightness);
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
					send(state.stageVizAtmosphere, environmentBrightness);
				}}
			/>
			<div className="stage-viz-status">
				<strong>Renderer</strong>
				<span>{renderer ?? "Not drawing this Stage"}</span>
			</div>
			{trouble && (
				<div className="stage-viz-trouble" role="alert">
					{trouble}
				</div>
			)}
		</>
	);
}
