import { Button, FormField, FormLayout, TextField } from "@tosklight/ui";
import { configuredServerUrl } from "../../api/client/serverLocation";
import { MatterBridgeSettings } from "../../components/setup/MatterBridgeSettings";
import { SoundInputSettings } from "../../components/setup/SoundInputSettings";
import type { SetupWindowController } from "./controller";

function NetworkInputs({ controller }: { controller: SetupWindowController }) {
	const { draft } = controller;
	return (
		<section
			className="network-settings-group"
			aria-labelledby="control-inputs"
		>
			<h3 id="control-inputs">Control inputs</h3>
			<div className="setup-list network-input-list">
				<article>
					<b>MIDI inputs</b>
					<span>
						{draft?.midi_inputs.length
							? draft.midi_inputs.join(", ")
							: "No MIDI inputs selected"}
					</span>
				</article>
				<article>
					<b>OSC</b>
					<span>{draft?.osc_bind ?? "Disabled"}</span>
				</article>
				<article>
					<b>RTP-MIDI</b>
					<span>{draft?.rtp_midi_bind ?? "Disabled"}</span>
				</article>
			</div>
		</section>
	);
}

export function NetworkSection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	return (
		<>
			<h2>Network &amp; Inputs</h2>
			<section
				className="network-settings-group"
				aria-labelledby="network-connection"
			>
				<h3 id="network-connection">ToskLight server connection</h3>
				<FormLayout className="configuration-form" labelPlacement="side">
					<TextField
						label="Light server URL"
						value={controller.serverUrl}
						onChange={(event) => controller.setServerUrl(event.target.value)}
						description="Tauri can use this desk or a remote Light server."
					/>
					<FormField label="">
						<Button
							onClick={() => controller.applyServerUrl(controller.serverUrl)}
						>
							Connect to server
						</Button>
					</FormField>
				</FormLayout>
				<div className="setup-cards">
					<section>
						<b>{configuredServerUrl()}</b>
						<small>Active REST and WebSocket server</small>
					</section>
					<section>
						<b>REST /api/v2</b>
						<small>Initial and coarse-grained state</small>
					</section>
					<section>
						<b>WebSocket connected</b>
						<small>Live events and control</small>
					</section>
				</div>
			</section>
			<NetworkInputs controller={controller} />
			<section className="network-settings-group" aria-labelledby="sound-input">
				<h3 id="sound-input">Sound input</h3>
				<SoundInputSettings />
			</section>
			<section
				className="network-settings-group"
				aria-labelledby="matter-bridge"
			>
				<h3 id="matter-bridge">Matter bridge</h3>
				<MatterBridgeSettings />
			</section>
		</>
	);
}
