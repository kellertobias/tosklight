// The generated sources, and how to reach them.
//
// A visualizer is selected the same way a clip is — by address — so this page's job is to show
// which address reaches which visualizer, and to let an operator put one on a layer without
// looking the number up first.

import { Button, SelectField } from "@tosklight/ui/controls";
import { useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { addressLabel } from "../../entities/catalog";
import type { OutputView, VisualizerView } from "../../shared/api/generated/media-wire";
import { useVisualizers } from "../../shared/api/queries";
import { useLayerControl, useOutputsForControl } from "../../shared/api/layerControl";

export function VisualizersPage() {
	const visualizers = useVisualizers();
	const outputs = useOutputsForControl();
	const control = useLayerControl(outputs.data);
	const [layer, setLayer] = useState(0);

	return (
		<section className="media-page">
			{control.refusal && (
				<p className="media-state is-error" role="alert">
					{control.refusal.deskOwnsIt
						? "That selection was not applied: a lighting desk is driving this output."
						: control.refusal.message}{" "}
					<Button size="compact" onClick={control.dismissRefusal}>
						Dismiss
					</Button>
				</p>
			)}

			<LayerChoice
				outputs={outputs.data ?? []}
				layer={layer}
				onChange={setLayer}
			/>

			<ResourceState
				resource={visualizers}
				subject="the visualizers"
				isEmpty={(data) => data.length === 0}
				empty="No generated visualizers are assigned to an address."
			>
				{(data) => (
					<div className="media-output-grid">
						{data.map((visualizer) => (
							<VisualizerCard
								key={`${visualizer.address.folder}/${visualizer.address.file}`}
								visualizer={visualizer}
								outputs={outputs.data ?? []}
								onSelect={(output) =>
									void control.update(output, layer, {
										folder: visualizer.address.folder,
										file: visualizer.address.file,
									})
								}
							/>
						))}
					</div>
				)}
			</ResourceState>
		</section>
	);
}

function LayerChoice({
	outputs,
	layer,
	onChange,
}: {
	outputs: OutputView[];
	layer: number;
	onChange: (layer: number) => void;
}) {
	// Every output has at least as many layers as the smallest personality here, so the choice is
	// a layer number rather than one list per output.
	const layers = Math.max(1, ...outputs.map((output) => output.layerCount));
	return (
		<SelectField
			label="Put the chosen visualizer on"
			value={String(layer)}
			options={Array.from({ length: layers }, (_, index) => ({
				value: String(index),
				label: `Layer ${index + 1}`,
			}))}
			onChange={(next) => onChange(Number(next))}
		/>
	);
}

function VisualizerCard({
	visualizer,
	outputs,
	onSelect,
}: {
	visualizer: VisualizerView;
	outputs: OutputView[];
	onSelect: (output: OutputView) => void;
}) {
	const address = addressLabel(visualizer.address.folder, visualizer.address.file);
	return (
		<article className="media-output-card" aria-label={visualizer.name}>
			<header>
				<h2>{visualizer.name}</h2>
				<span className="media-address">{address}</span>
			</header>
			<p>
				{visualizer.kind} · type {visualizer.typeId}
			</p>
			<p className="media-visualizer-uses">
				Controls: {visualizer.uses.join(", ")}
			</p>
			<div className="media-visualizer-actions">
				{outputs.map((output) => (
					<Button
						key={output.id}
						disabled={output.dmxActive}
						onClick={() => onSelect(output)}
					>
						{outputs.length === 1 ? "Select" : `Select on ${output.name}`}
					</Button>
				))}
			</div>
		</article>
	);
}
