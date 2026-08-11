// What this server is, what it listens on, and what it sends to.
//
// Output identity is editable as stored configuration. The running surface is deliberately left
// alone, and the page says so rather than pretending a monitor or resolution changed live.

import { Button } from "@tosklight/ui/controls";
import { useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import {
	MediaSettingsLayout,
	type MediaSettingsSection,
} from "../../operator/MediaServerSurface";
import { api } from "../../shared/api/client";
import { useEditing } from "../../shared/api/editing";
import type {
	Health,
	NetworkView,
} from "../../shared/api/generated/media-wire";
import { useHealth, useNetwork, useOutputs } from "../../shared/api/queries";
import { AudioPage } from "../audio/AudioPage";
import { DmxPage } from "../dmx/DmxPage";
import { LogsPage } from "../logs/LogsPage";
import { BoundAddresses, NetworkEditor } from "./NetworkEditor";
import { OutputSettings } from "./OutputSettings";

const HEALTH_POLL_MS = 15_000;

export function SettingsPage() {
	const health = useHealth(HEALTH_POLL_MS);
	const outputs = useOutputs(HEALTH_POLL_MS);
	const network = useNetwork();
	const editing = useEditing(network.reload);
	const initialSection =
		window.location.pathname === "/logs"
			? "logs"
			: window.location.pathname === "/audio" ||
					window.location.pathname === "/dmx"
				? "network-inputs"
				: "libraries";
	const [section, setSection] = useState<MediaSettingsSection>(initialSection);

	return (
		<MediaSettingsLayout active={section} onSelect={setSection}>
			{section === "libraries" && (
				<section className="media-page">
					<ResourceState resource={health} subject="server settings">
						{(data) => (
							<article className="media-settings-section" aria-label="Server">
								<h2>Server</h2>
								<ServerSettings health={data} />
							</article>
						)}
					</ResourceState>

					<article className="media-settings-section" aria-label="Libraries">
						<h2>Libraries</h2>
						<p className="media-state is-notice">
							The library folder is the <code>library.root</code> value in this
							server's configuration file. Restart the server after changing it.
						</p>
					</article>
				</section>
			)}

			{section === "network-inputs" && (
				<section className="media-page">
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
								onSave={(edit) =>
									void editing.save(() => api.updateNetwork(edit))
								}
							/>
						)}
					</ResourceState>
					<ResourceState
						resource={outputs}
						subject="DMX input settings"
						isEmpty={(data) => data.length === 0}
						empty="No outputs are enabled."
					>
						{(data) => (
							<section
								className="media-settings-group"
								aria-labelledby="dmx-inputs-heading"
							>
								<h2 id="dmx-inputs-heading">DMX</h2>
								{data.map((output) => (
									<OutputSettings
										key={output.id}
										outputId={output.id}
										outputName={output.name}
										mode="dmx"
									/>
								))}
							</section>
						)}
					</ResourceState>
					<section
						className="media-settings-group"
						aria-labelledby="audio-input-heading"
					>
						<h2 id="audio-input-heading">Audio input</h2>
						<AudioPage />
					</section>
					<details className="media-settings-section">
						<summary>DMX diagnostics and fixture downloads</summary>
						<DmxPage />
					</details>
				</section>
			)}

			{section === "outputs" && (
				<ResourceState
					resource={outputs}
					subject="output settings"
					isEmpty={(data) => data.length === 0}
					empty="No outputs are enabled."
				>
					{(data) => (
						<section
							className="media-settings-group"
							aria-labelledby="outputs-heading"
						>
							<h2 id="outputs-heading">Outputs</h2>
							{data.map((output) => (
								<OutputSettings
									key={output.id}
									outputId={output.id}
									outputName={output.name}
									mode="picture"
								/>
							))}
						</section>
					)}
				</ResourceState>
			)}
			{section === "logs" && <LogsPage />}
		</MediaSettingsLayout>
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
			<h2>Network</h2>
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
					A saved change to these addresses is used the next time this server
					starts. The sockets it is using now stay as they are.
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
