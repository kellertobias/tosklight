// What the desk is doing to this server.
//
// This page answers one question an operator asks at a patch bay: is anything actually arriving,
// and is it landing where I think it is. It reports; it does not control.

import { ResourceState } from "../../app/ResourceState";
import { addressLabel } from "../../entities/catalog";
import { percent, sourceBadge } from "../../entities/output";
import type { OutputView } from "../../shared/api/generated/media-wire";
import { useOutputs } from "../../shared/api/queries";

const OUTPUT_POLL_MS = 1_000;

export function DmxPage() {
	const outputs = useOutputs(OUTPUT_POLL_MS);

	return (
		<section className="media-page">
			<ResourceState
				resource={outputs}
				subject="outputs"
				isEmpty={(data) => data.length === 0}
				empty="This server has no enabled outputs, so nothing is listening for a desk."
			>
				{(data) => data.map((output) => <OutputDmx key={output.id} output={output} />)}
			</ResourceState>
		</section>
	);
}

function OutputDmx({ output }: { output: OutputView }) {
	return (
		<section className="media-output" aria-label={`${output.name} DMX`}>
			<h2>{output.name}</h2>
			<p className={`media-badge is-${output.dmxActive ? "good" : "neutral"}`}>
				{output.dmxActive
					? "A desk is sending to this output"
					: "No desk has sent to this output recently"}
			</p>

			<table className="media-table">
				<caption className="media-visually-hidden">
					Layer values arriving on {output.name}
				</caption>
				<thead>
					<tr>
						<th scope="col">Layer</th>
						<th scope="col">Address</th>
						<th scope="col">Play mode</th>
						<th scope="col">Dimmer</th>
						<th scope="col">Source</th>
					</tr>
				</thead>
				<tbody>
					{output.layers.map((layer) => {
						const badge = sourceBadge(layer.sourceStatus);
						return (
							<tr key={layer.index}>
								<td>{layer.index + 1}</td>
								<td>{addressLabel(layer.address.folder, layer.address.file)}</td>
								<td>{layer.playMode}</td>
								<td>{percent(layer.dimmer)}</td>
								<td>{badge.label}</td>
							</tr>
						);
					})}
				</tbody>
			</table>

			<dl className="media-facts">
				<dt>Master dimmer</dt>
				<dd>{percent(output.master.dimmer)}</dd>
				<dt>Master volume</dt>
				<dd>{percent(output.master.volume)}</dd>
				<dt>Master tint</dt>
				<dd>
					R {percent(output.master.tintRed)} · G {percent(output.master.tintGreen)} · B{" "}
					{percent(output.master.tintBlue)}
				</dd>
				<dt>Flip and mirror</dt>
				<dd>{output.master.flipMirror}</dd>
				<dt>Master mask</dt>
				<dd>
					{output.master.mask.class === "blank"
						? "none"
						: addressLabel(output.master.mask.folder, output.master.mask.file)}
				</dd>
			</dl>
		</section>
	);
}
