import { Button, FormField, FormLayout, TextField } from "@tosklight/ui";
import { useCallback, useEffect, useState } from "react";
import type { ExtensionRuntimeSnapshot } from "../../api/client/deskManagement";
import { configuredServerUrl } from "../../api/client/serverLocation";
import { MatterBridgeSettings } from "../../components/setup/MatterBridgeSettings";
import { SoundInputSettings } from "../../components/setup/SoundInputSettings";
import { useExtensionRuntimeActions } from "../../features/extensions/ExtensionRuntimeActions";
import type { SetupWindowController } from "./controller";

function NetworkInputs({ controller }: { controller: SetupWindowController }) {
	const { draft } = controller;
	const extensionActions = useExtensionRuntimeActions();
	const loadExtensionSnapshot = extensionActions?.load;
	const requestExtensionRescan = extensionActions?.rescan;
	const [extensions, setExtensions] = useState<ExtensionRuntimeSnapshot | null>(
		null,
	);
	const [extensionError, setExtensionError] = useState<string | null>(null);
	const [rescanning, setRescanning] = useState(false);
	const loadExtensions = useCallback(async () => {
		if (!loadExtensionSnapshot) return;
		try {
			setExtensionError(null);
			setExtensions(await loadExtensionSnapshot());
		} catch (reason) {
			setExtensionError(errorMessage(reason));
		}
	}, [loadExtensionSnapshot]);
	useEffect(() => {
		void loadExtensions();
	}, [loadExtensions]);
	const rescanExtensions = async () => {
		if (!requestExtensionRescan || rescanning) return;
		setRescanning(true);
		try {
			setExtensionError(null);
			setExtensions(await requestExtensionRescan());
		} catch (reason) {
			setExtensionError(errorMessage(reason));
		} finally {
			setRescanning(false);
		}
	};
	return (
		<section
			className="network-settings-group"
			aria-labelledby="control-inputs"
		>
			<h3 id="control-inputs">Control inputs</h3>
			<div className="setup-list network-input-list">
				<article className="native-extension-status">
					<div>
						<b>Native extensions</b>
						<span>{extensionSummary(extensions, extensionError)}</span>
					</div>
					<Button
						disabled={!extensionActions || rescanning}
						onClick={() => void rescanExtensions()}
					>
						{rescanning ? "Rescanning…" : "Rescan extensions"}
					</Button>
					{extensions && (
						<details>
							<summary>Extension diagnostics</summary>
							<dl>
								<dt>Extensions folder</dt>
								<dd>{extensions.extensions_directory}</dd>
								<dt>Configuration file</dt>
								<dd>{extensions.configuration_path}</dd>
							</dl>
							{extensions.configuration_diagnostic && (
								<p role="alert">{extensions.configuration_diagnostic}</p>
							)}
							{extensions.packages.map((extensionPackage) => (
								<div
									key={extensionPackage.directory}
									className="native-extension-package"
								>
									<b>
										{extensionPackage.name ??
											extensionPackage.id ??
											"Invalid package"}
									</b>
									<span>
										{extensionPackage.version
											? `${extensionPackage.version} · `
											: ""}
										{extensionPackage.readiness}
									</span>
									{extensionPackage.diagnostics.map((diagnostic) => (
										<small key={`${diagnostic.code}:${diagnostic.detail}`}>
											{diagnostic.code}: {diagnostic.detail}
										</small>
									))}
								</div>
							))}
							{extensions.instances.map((instance) => (
								<div key={instance.id} className="native-extension-instance">
									<b>{instance.id}</b>
									<span>
										{instance.state}
										{instance.last_error ? ` · ${instance.last_error}` : ""}
									</span>
								</div>
							))}
							{extensions.instance_diagnostics.map((diagnostic) => (
								<p key={`${diagnostic.instance_id}:${diagnostic.code}`} role="alert">
									{diagnostic.instance_id} · {diagnostic.code}: {diagnostic.detail}
								</p>
							))}
						</details>
					)}
				</article>
				<article>
					<b>OSC</b>
					<span>{draft?.osc_bind ?? "Disabled"}</span>
				</article>
			</div>
		</section>
	);
}

function extensionSummary(
	snapshot: ExtensionRuntimeSnapshot | null,
	error: string | null,
): string {
	if (error) return `Unavailable: ${error}`;
	if (!snapshot) return "Loading extension status…";
	const running = snapshot.instances.filter(
		(instance) => instance.state === "running",
	).length;
	return `${snapshot.packages.length} package${snapshot.packages.length === 1 ? "" : "s"} · ${running} running instance${running === 1 ? "" : "s"}`;
}

function errorMessage(reason: unknown): string {
	return reason instanceof Error ? reason.message : String(reason);
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
