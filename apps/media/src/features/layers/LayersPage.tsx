import { ErrorAlert } from "@tosklight/ui";
// The layer page: every output, every layer, and why a control might be unavailable.

import { Button } from "@tosklight/ui/controls";
import { ResourceState } from "../../app/ResourceState";
import { readOnlyReason } from "../../entities/output";
import type { OutputView } from "../../shared/api/generated/media-wire";
import {
	useLayerControl,
	useOutputsForControl,
} from "../../shared/api/layerControl";
import { useCatalog, useText, useVisualizers } from "../../shared/api/queries";
import { LayerCard } from "./LayerCard";

/** The library changes when someone imports, which is rare compared with layer state. */
const CATALOG_POLL_MS = 15_000;

export function LayersPage() {
	const outputs = useOutputsForControl();
	const catalog = useCatalog(CATALOG_POLL_MS);
	const control = useLayerControl();
	const text = useText();
	const visualizers = useVisualizers();

	return (
		<section className="media-page">
			{control.refusal && (
				<ErrorAlert as="p" className="media-state is-error" role="alert">
					{control.refusal.deskOwnsIt
						? "That change was not applied: a lighting desk is driving this output."
						: control.refusal.message}{" "}
					<Button size="compact" onClick={control.dismissRefusal}>
						Dismiss
					</Button>
				</ErrorAlert>
			)}

			<ResourceState
				resource={outputs}
				subject="outputs"
				isEmpty={(data) => data.length === 0}
				empty="This server has no enabled outputs. Add one in the configuration file."
			>
				{(data) =>
					data.map((output) => (
						<OutputLayers
							key={output.id}
							output={output}
							catalog={catalog.data}
							text={text.data}
							visualizers={visualizers.data}
							control={control}
						/>
					))
				}
			</ResourceState>
		</section>
	);
}

function OutputLayers({
	output,
	catalog,
	text,
	visualizers,
	control,
}: {
	output: OutputView;
	catalog: ReturnType<typeof useCatalog>["data"];
	text: ReturnType<typeof useText>["data"];
	visualizers: ReturnType<typeof useVisualizers>["data"];
	control: ReturnType<typeof useLayerControl>;
}) {
	const reason = readOnlyReason(output);
	return (
		<section className="media-output" aria-label={output.name}>
			<h2>{output.name}</h2>
			{reason && (
				<p className="media-state is-notice" role="status">
					{reason}
				</p>
			)}
			<div className="media-layer-grid">
				{output.layers.map((layer) => (
					<LayerCard
						key={layer.index}
						output={output}
						layer={layer}
						catalog={catalog}
						text={text}
						visualizers={visualizers}
						readOnly={reason !== undefined}
						onChange={(change) =>
							void control.update(output, layer.index, change)
						}
						onReset={() => void control.reset(output, layer.index)}
					/>
				))}
			</div>
		</section>
	);
}
