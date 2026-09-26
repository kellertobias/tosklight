// What this server is, what it listens on, and what it sends to.
//
// Every setting saves automatically. Most of them reach the running server at once; the few that
// need a restart — a window, a device, a personality, CITP and this interface's own address — say
// so beside the section heading only while such a change is actually waiting.
// Selecting another monitor moves an open output while Pixel is running.

import { Button } from "@tosklight/ui/controls";
import { useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { useFailureToast } from "../../app/ToastContext";
import {
	MediaSettingsLayout,
	type MediaSettingsSection,
} from "../../operator/MediaServerSurface";
import { api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type { NetworkView } from "../../shared/api/generated/media-wire";
import { useNetwork, useOutputs } from "../../shared/api/queries";
import { LogsPage } from "../logs/LogsPage";
import { LibrarySettingsSection } from "./LibrarySettingsSection";
import { NetworkEditor } from "./NetworkEditor";
import { OutputSettings } from "./OutputSettings";
import { SettingsSaveState } from "./SettingsSaveState";
import { SpeedGroupStatus } from "./SpeedGroupStatus";

const HEALTH_POLL_MS = 15_000;

export function SettingsPage() {
	const outputs = useOutputs(HEALTH_POLL_MS);
	const network = useNetwork();
	const editing = useEditing(network.reload);
	// `section=dmx` is the address DMX diagnostics link to; DMX input lives with Network now.
	const initialSection: MediaSettingsSection =
		window.location.pathname === "/logs" ? "logs" : "network";
	const [section, setSection] = useState<MediaSettingsSection>(initialSection);
	useFailureToast(editing.failure);

	return (
		<MediaSettingsLayout active={section} onSelect={setSection}>
			{section === "libraries" && <LibrarySettingsSection />}

			{section === "network" && (
				<section className="media-page">
					<ResourceState resource={network} subject="the network settings">
						{(data) => (
							<Network
								network={data}
								busy={editing.busy}
								failed={editing.failure !== undefined}
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
								id="dmx-input"
								className="media-settings-group"
								aria-labelledby="dmx-inputs-heading"
							>
								<h2 id="dmx-inputs-heading">DMX input</h2>
								<p className="media-settings-note">
									Each output reads its personality from the Art-Net or sACN
									address above. Choose the protocol, universe, and start
									address here.
								</p>
								{data.map((output) => (
									<OutputSettings
										key={output.id}
										outputId={output.id}
										outputName={output.name}
										mode="dmx"
										direct
									/>
								))}
							</section>
						)}
					</ResourceState>
				</section>
			)}

			{(section === "picture-output" || section === "sound-output") && (
				<ResourceState
					resource={outputs}
					subject={
						section === "picture-output" ? "picture settings" : "sound settings"
					}
					isEmpty={(data) => data.length === 0}
					empty="No outputs are enabled."
				>
					{(data) => (
						<section
							className="media-settings-group"
							aria-labelledby="outputs-heading"
						>
							<h2 id="outputs-heading">
								{section === "picture-output" ? "Picture" : "Sound"}
							</h2>
							{data.map((output) => (
								<OutputSettings
									key={output.id}
									outputId={output.id}
									outputName={output.name}
									mode={section === "picture-output" ? "picture" : "sound"}
									direct
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
	formId,
	network,
	busy,
	failed,
	onSave,
}: {
	formId?: string;
	network: NetworkView;
	busy: boolean;
	failed: boolean;
	onSave: (edit: Parameters<typeof api.updateNetwork>[0]) => void;
}) {
	return (
		<article className="media-settings-section" aria-label="Network">
			<div className="media-settings-section-heading">
				<h2>Network</h2>
				<SettingsSaveState
					busy={busy}
					failed={failed}
					restartBound={network.pendingRestart}
				/>
			</div>
			<NetworkEditor
				formId={formId}
				network={network}
				busy={busy}
				onSave={onSave}
				showActions={false}
			/>
			<SpeedGroupStatus />
			{network.warnings.map((warning) => (
				<p key={warning} className="media-state is-error" role="alert">
					{warning}
				</p>
			))}
			{network.pendingRestart && network.takesEffectOnRestart && (
				<>
					<p className="media-state is-notice">
						The saved CITP or interface address is used the next time this
						server starts; until then it keeps{" "}
						<code>{network.resolved.citpListen}</code> and{" "}
						<code>{network.resolved.httpListen}</code>. Art-Net, sACN and Speed
						Groups already use the saved addresses.
					</p>
					<div className="media-settings-actions">
						<Button
							onClick={() =>
								onSave({
									requestId: requestId(),
									sameComputerPreset: network.activeSameComputerPreset,
									citpListen: network.activeStored.citpListen,
									httpListen: network.activeStored.httpListen,
								})
							}
						>
							Revert to current settings
						</Button>
					</div>
				</>
			)}
		</article>
	);
}
