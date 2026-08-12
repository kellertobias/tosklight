// What is running, right now, in as few words as possible.

import { ResourceState } from "../../app/ResourceState";
import { addressLabel } from "../../entities/catalog";
import { layersDrawing, percent, sourceBadge } from "../../entities/output";
import { useDeskShowName } from "../../operator/DeskIdentityContext";
import type { Health, OutputView } from "../../shared/api/generated/media-wire";
import { useHealth, useOutputs } from "../../shared/api/queries";

const OUTPUT_POLL_MS = 1_000;
const HEALTH_POLL_MS = 5_000;

export function DashboardPage() {
	const health = useHealth(HEALTH_POLL_MS);
	const outputs = useOutputs(OUTPUT_POLL_MS);
	const showName = useDeskShowName();

	return (
		<section className="media-page">
			<div
				className={`media-desk-identity ${showName ? "is-connected" : "is-waiting"}`}
				role="status"
			>
				<strong>Light Desk</strong>
				<span>{showName ?? "Not identified"}</span>
				<small>{showName ? "Active show" : "Waiting for CITP identity"}</small>
			</div>
			<ResourceState resource={health} subject="server status">
				{(data) => <ServerFacts health={data} />}
			</ResourceState>

			<ResourceState
				resource={outputs}
				subject="outputs"
				isEmpty={(data) => data.length === 0}
				empty="This server has no enabled outputs."
			>
				{(data) => (
					<div className="media-output-grid">
						{data.map((output) => (
							<OutputSummary key={output.id} output={output} />
						))}
					</div>
				)}
			</ResourceState>
		</section>
	);
}

function ServerFacts({ health }: { health: Health }) {
	return (
		<dl className="media-facts">
			<dt>Instance</dt>
			<dd>{health.instance}</dd>
			<dt>Outputs</dt>
			<dd>{health.outputs}</dd>
			<dt>Library</dt>
			<dd>
				{health.catalogItems} items · revision {health.catalogRevision}
			</dd>
		</dl>
	);
}

function OutputSummary({ output }: { output: OutputView }) {
	return (
		<article className="media-output-card" aria-label={output.name}>
			<header>
				<h2>{output.name}</h2>
				<span
					className={`media-badge is-${output.dmxActive ? "good" : "neutral"}`}
				>
					{output.dmxActive ? "Desk connected" : "No desk"}
				</span>
			</header>
			<p>
				{layersDrawing(output)} of {output.layerCount} layers drawing · master{" "}
				{percent(output.master.dimmer)}
			</p>
			<ul className="media-layer-list">
				{output.layers.map((layer) => {
					const badge = sourceBadge(layer.sourceStatus);
					return (
						<li key={layer.index}>
							<span>Layer {layer.index + 1}</span>
							<span>
								{addressLabel(layer.address.folder, layer.address.file)}
							</span>
							<span className={`media-badge is-${badge.tone}`}>
								{badge.label}
							</span>
						</li>
					);
				})}
			</ul>
		</article>
	);
}
