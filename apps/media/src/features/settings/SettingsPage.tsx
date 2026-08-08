// What this server is, and what it is configured to expose.
//
// Editing configuration is a later slice; until the API carries it, this page reports the parts
// the server already publishes rather than pretending to own settings it cannot write.

import { ResourceState } from "../../app/ResourceState";
import type { Health, OutputView } from "../../shared/api/generated/media-wire";
import { useHealth, useOutputs } from "../../shared/api/queries";

const HEALTH_POLL_MS = 15_000;

export function SettingsPage() {
	const health = useHealth(HEALTH_POLL_MS);
	const outputs = useOutputs(HEALTH_POLL_MS);

	return (
		<section className="media-page">
			<ResourceState resource={health} subject="server settings">
				{(data) => <ServerSettings health={data} />}
			</ResourceState>

			<ResourceState
				resource={outputs}
				subject="output settings"
				isEmpty={(data) => data.length === 0}
				empty="No outputs are enabled."
			>
				{(data) => (
					<table className="media-table">
						<caption>Outputs</caption>
						<thead>
							<tr>
								<th scope="col">Name</th>
								<th scope="col">Layers</th>
								<th scope="col">Identifier</th>
							</tr>
						</thead>
						<tbody>
							{data.map((output: OutputView) => (
								<tr key={output.id}>
									<td>{output.name}</td>
									<td>{output.layerCount}</td>
									<td>
										<code>{output.id}</code>
									</td>
								</tr>
							))}
						</tbody>
					</table>
				)}
			</ResourceState>

			<p className="media-state is-notice">
				Outputs, network addresses, and the library folder are set in the server's
				configuration file. Editing them from here arrives with the configuration API.
			</p>
		</section>
	);
}

function ServerSettings({ health }: { health: Health }) {
	return (
		<dl className="media-facts">
			<dt>Instance</dt>
			<dd>
				<code>{health.instance}</code>
			</dd>
			<dt>Status</dt>
			<dd>{health.status}</dd>
			<dt>Library items</dt>
			<dd>{health.catalogItems}</dd>
			<dt>Library revision</dt>
			<dd>{health.catalogRevision}</dd>
		</dl>
	);
}
