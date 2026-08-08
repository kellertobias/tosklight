// The layer page: every output, every layer, and why a control might be unavailable.

import { Button } from "@tosklight/ui/controls";
import { ResourceState } from "../../app/ResourceState";
import { readOnlyReason } from "../../entities/output";
import { useCatalog } from "../../shared/api/queries";
import type { OutputView } from "../../shared/api/generated/media-wire";
import { LayerCard } from "./LayerCard";
import { useLayerControl, useOutputsForControl } from "./service";

/** The library changes when someone imports, which is rare compared with layer state. */
const CATALOG_POLL_MS = 15_000;

export function LayersPage() {
	const outputs = useOutputsForControl();
	const catalog = useCatalog(CATALOG_POLL_MS);
	const control = useLayerControl(outputs.data);

	return (
		<section className="media-page">
			{control.refusal && (
				<p className="media-state is-error" role="alert">
					{control.refusal.deskOwnsIt
						? "That change was not applied: a lighting desk is driving this output."
						: control.refusal.message}{" "}
					<Button size="compact" onClick={control.dismissRefusal}>
						Dismiss
					</Button>
				</p>
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
	control,
}: {
	output: OutputView;
	catalog: ReturnType<typeof useCatalog>["data"];
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
						readOnly={reason !== undefined}
						onChange={(change) => void control.update(output, layer.index, change)}
						onReset={() => void control.reset(output, layer.index)}
					/>
				))}
			</div>
		</section>
	);
}
