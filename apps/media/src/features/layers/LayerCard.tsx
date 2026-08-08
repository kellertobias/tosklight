// One layer's controls.

import { Button } from "@tosklight/ui/controls";
import type { CatalogView, LayerView, OutputView, UpdateLayer } from "../../shared/api/generated/media-wire";
import { layerName, percent, sourceBadge } from "../../entities/output";
import { addressLabel, resolveAddress } from "../../entities/catalog";
import { MediaPicker } from "./MediaPicker";

export interface LayerCardProps {
	output: OutputView;
	layer: LayerView;
	catalog: CatalogView | undefined;
	/** True while a desk owns the output; every write control is disabled, not hidden. */
	readOnly: boolean;
	onChange: (change: UpdateLayer) => void;
	onReset: () => void;
}

/// What the layer's mask is doing, in the terms an operator would use at the desk.
///
/// "Selected but faded out" and "nothing selected" look the same on screen and are different
/// problems, so they read differently here.
function maskSummary(layer: LayerView): string {
	const { mask } = layer;
	if (mask.address.class === "blank") return "none";
	const address = addressLabel(mask.address.folder, mask.address.file);
	const how = `${mask.source}${mask.invert ? ", inverted" : ""}`;
	if (!mask.active) return `${address} · faded out`;
	return `${address} · ${percent(mask.opacity)} · ${how}`;
}

export function LayerCard({ output, layer, catalog, readOnly, onChange, onReset }: LayerCardProps) {
	const badge = sourceBadge(layer.sourceStatus);
	const { item } = resolveAddress(catalog, layer.address.folder, layer.address.file);
	const dimmerId = `${output.id}-layer-${layer.index}-dimmer`;

	return (
		<article className="media-layer-card" aria-label={layerName(layer)}>
			<header>
				<h3>{layerName(layer)}</h3>
				<span className={`media-badge is-${badge.tone}`}>{badge.label}</span>
			</header>

			<p className="media-layer-source">
				{item ? item.name : "Nothing selected"}
				{layer.drawing ? "" : " · not drawing"}
			</p>
			{badge.detail && (
				<p className="media-state is-error" role="alert">
					{badge.detail}
				</p>
			)}

			<MediaPicker
				catalog={catalog}
				folder={layer.address.folder}
				file={layer.address.file}
				disabled={readOnly}
				onSelect={onChange}
			/>

			<div className="media-layer-dimmer">
				<label htmlFor={dimmerId}>Dimmer</label>
				<input
					id={dimmerId}
					type="range"
					min={0}
					max={100}
					step={1}
					value={Math.round(layer.dimmer * 100)}
					disabled={readOnly}
					onChange={(event) => onChange({ dimmer: Number(event.target.value) / 100 })}
				/>
				<output htmlFor={dimmerId}>{percent(layer.dimmer)}</output>
			</div>

			<dl className="media-layer-facts">
				<dt>Play mode</dt>
				<dd>{layer.playMode}</dd>
				<dt>Mask</dt>
				<dd>{maskSummary(layer)}</dd>
				<dt>Scale</dt>
				<dd>
					{layer.scaleX.toFixed(2)} × {layer.scaleY.toFixed(2)}
				</dd>
				<dt>Rotation</dt>
				<dd>{layer.rotation.toFixed(1)}°</dd>
			</dl>

			<Button onClick={onReset} disabled={readOnly}>
				Restart media
			</Button>
		</article>
	);
}
