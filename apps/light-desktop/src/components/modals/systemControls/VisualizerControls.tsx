import { Button, SelectField } from "@tosklight/ui";
import type {
	VisualizerRenderQuality,
	VisualizerView,
	VisualizerViewMode,
} from "../../../api/client/visualizerView";

/** The eight named views, in the order the renderer's own number keys reach them. */
export const VISUALIZER_VIEWS = [
	{ mode: "full_3d", label: "3D Full" },
	{ mode: "top_down", label: "Top Down" },
	{ mode: "front_to_back", label: "Front → Back" },
	{ mode: "left_to_right", label: "Left → Right" },
	{ mode: "right_to_left", label: "Right → Left" },
	{ mode: "simple_3d", label: "3D Simple" },
	{ mode: "back_to_front", label: "Back → Front" },
	{ mode: "lines_3d", label: "3D Lines" },
] as const satisfies readonly { mode: VisualizerViewMode; label: string }[];

const QUALITIES = [
	{ quality: "draft", label: "Draft" },
	{ quality: "standard", label: "Standard" },
	{ quality: "high", label: "High" },
	{ quality: "ultra", label: "Ultra" },
] as const satisfies readonly {
	quality: VisualizerRenderQuality;
	label: string;
}[];

interface VisualizerControlsProps {
	view: VisualizerView | null;
	targets: readonly string[];
	target: string;
	busy: boolean;
	error: string | null;
	onSelectTarget(target: string): void;
	onSelectMode(mode: VisualizerViewMode): void;
	onSelectQuality(quality: VisualizerRenderQuality): void;
	onResetPhysics(): void;
}

/**
 * Which way the connected visualizer is pointing.
 *
 * The desk chooses the view; the renderer presents it. An operator who then moves the camera in
 * the renderer keeps it until the desk says something new, which is why selecting the view that
 * is already shown is still worth doing.
 */
export function VisualizerControls(props: VisualizerControlsProps) {
	return (
		<section
			className="system-controls-visualizer"
			aria-label="Visualizer view"
		>
			<header className="system-controls-visualizer-header">
				<h3>Visualizer</h3>
				{props.targets.length > 1 && (
					<SelectField
						label="Renderer"
						ariaLabel="Renderer"
						value={props.target}
						options={props.targets.map((target) => ({
							value: target,
							label: target,
						}))}
						onChange={props.onSelectTarget}
					/>
				)}
			</header>
			<div className="system-controls-visualizer-views" role="group">
				{VISUALIZER_VIEWS.map((entry) => (
					<Button
						key={entry.mode}
						className={
							props.view?.mode === entry.mode ? "active" : undefined
						}
						aria-pressed={props.view?.mode === entry.mode}
						disabled={props.busy || props.view === null}
						onClick={() => props.onSelectMode(entry.mode)}
					>
						{entry.label}
					</Button>
				))}
			</div>
			<SelectField
				label="Rendering quality"
				ariaLabel="Rendering quality"
				value={props.view?.quality ?? "high"}
				options={QUALITIES.map((entry) => ({
					value: entry.quality,
					label: entry.label,
				}))}
				onChange={(quality) =>
					props.onSelectQuality(quality as VisualizerRenderQuality)
				}
			/>
			<Button
				disabled={props.busy || props.view === null}
				onClick={props.onResetPhysics}
			>
				Reset physics scenery
			</Button>
			{props.error ? (
				<p className="system-controls-visualizer-error" role="status">
					{props.error}
				</p>
			) : (
				props.view === null && (
					<p className="system-controls-visualizer-error" role="status">
						Reading the visualizer view…
					</p>
				)
			)}
		</section>
	);
}
