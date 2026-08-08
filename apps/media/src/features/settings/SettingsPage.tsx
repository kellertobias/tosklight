// What this server is, what it listens on, and what it sends to.
//
// The network settings are editable here; the outputs are not, because what an output *is* — its
// monitor, its size, its presentation mode — is settled when its surface opens, and pretending
// otherwise would show an operator a change that never happened.

import { Button } from "@tosklight/ui/controls";
import { ResourceState } from "../../app/ResourceState";
import type { Health, NetworkView, OutputView } from "../../shared/api/generated/media-wire";
import { api } from "../../shared/api/client";
import { useEditing } from "../../shared/api/editing";
import { useHealth, useNetwork, useOutputs } from "../../shared/api/queries";
import { BoundAddresses, NetworkEditor } from "./NetworkEditor";

const HEALTH_POLL_MS = 15_000;

export function SettingsPage() {
	const health = useHealth(HEALTH_POLL_MS);
	const outputs = useOutputs(HEALTH_POLL_MS);
	const network = useNetwork();
	const editing = useEditing(network.reload);

	return (
		<section className="media-page">
			<ResourceState resource={health} subject="server settings">
				{(data) => <ServerSettings health={data} />}
			</ResourceState>

			{editing.failure && (
				<p className="media-state is-error" role="alert">
					{editing.failure.message}{" "}
					<Button size="compact" onClick={editing.dismiss}>
						Dismiss
					</Button>
				</p>
			)}

			<ResourceState resource={network} subject="the network settings">
				{(data) => (
					<Network
						network={data}
						editing={editing.editing === "network"}
						busy={editing.busy}
						onEdit={() => editing.begin("network")}
						onCancel={editing.cancel}
						onSave={(edit) => void editing.save(() => api.updateNetwork(edit))}
					/>
				)}
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
				Outputs and the library folder are set in the server's configuration file.
			</p>
		</section>
	);
}

function Network({
	network,
	editing,
	busy,
	onEdit,
	onCancel,
	onSave,
}: {
	network: NetworkView;
	editing: boolean;
	busy: boolean;
	onEdit: () => void;
	onCancel: () => void;
	onSave: (edit: Parameters<typeof api.updateNetwork>[0]) => void;
}) {
	return (
		<article className="media-settings-section" aria-label="Network">
			{editing ? (
				<NetworkEditor
					network={network}
					busy={busy}
					onSave={onSave}
					onCancel={onCancel}
				/>
			) : (
				<>
					<BoundAddresses network={network} />
					<div className="media-settings-actions">
						<Button onClick={onEdit}>Change network settings</Button>
					</div>
				</>
			)}
			{network.takesEffectOnRestart && (
				<p className="media-state is-notice">
					A saved change to these addresses is used the next time this server starts. The
					sockets it is using now stay as they are.
				</p>
			)}
		</article>
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
