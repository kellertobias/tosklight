import { configuredServerUrl } from "../../api/LightApiClient";
import {
	Button,
	FormField,
	FormLayout,
	TextField,
} from "../../components/common";
import { MatterBridgeSettings } from "../../components/setup/MatterBridgeSettings";
import type { SetupWindowController } from "./controller";

function NetworkInputs({ controller }: { controller: SetupWindowController }) {
	const { draft } = controller;
	return (
		<>
			<h3 className="setup-subsection-title">Inputs</h3>
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
		</>
	);
}

export function NetworkSection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	return (
		<>
			<h2>Network</h2>
			<FormLayout className="configuration-form" labelPlacement="side">
				<TextField
					label="Light server URL"
					value={controller.serverUrl}
					onChange={(event) => controller.setServerUrl(event.target.value)}
					description="Tauri can use this desk or a remote Light server."
				/>
				<FormField label="">
					<Button
						onClick={() => controller.server.setServerUrl(controller.serverUrl)}
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
					<b>REST /api/v1</b>
					<small>Initial and coarse-grained state</small>
				</section>
				<section>
					<b>WebSocket connected</b>
					<small>Live events and control</small>
				</section>
			</div>
			<NetworkInputs controller={controller} />
			<h3 className="setup-subsection-title">Services</h3>
			<MatterBridgeSettings />
		</>
	);
}
