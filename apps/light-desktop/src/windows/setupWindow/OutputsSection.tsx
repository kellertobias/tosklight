import { FormLayout, NumberField, TextField } from "@tosklight/ui";
import { OutputRoutesSetup } from "../../components/setup/OutputRoutesSetup";
import { useUsbDmxDiscovery } from "../../components/setup/UsbDmxEndpointsSetup";
import { useDmxDiagnostics } from "../../features/dmxDiagnostics/DmxDiagnosticsContext";
import { AudioOutputSection } from "./AudioOutputSection";
import type { SetupWindowController } from "./controller";

export function OutputsSection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const { draft } = controller;
	const dmx = useDmxDiagnostics();
	const usb = useUsbDmxDiscovery();
	if (!draft) return null;
	return (
		<>
			<h2>Outputs</h2>
			{controller.outputsTab === "engine" && (
				<FormLayout
					className="configuration-form"
					columns={3}
					minColumnWidth={190}
				>
					<NumberField
						label="Frame rate"
						min="40"
						max="60"
						value={draft.frame_rate_hz}
						onChange={(event) =>
							controller.editDraft({
								...draft,
								frame_rate_hz: Number(event.target.value),
							})
						}
						description="40–60 Hz"
					/>
					<TextField
						label="Output bind address"
						value={draft.output_bind_ip}
						onChange={(event) =>
							controller.editDraft({
								...draft,
								output_bind_ip: event.target.value,
							})
						}
					/>
					<NumberField
						label="Backup retention"
						min="1"
						max="1000"
						value={draft.backup_retention}
						onChange={(event) =>
							controller.editDraft({
								...draft,
								backup_retention: Number(event.target.value),
							})
						}
					/>
				</FormLayout>
			)}
			{controller.outputsTab === "routes" && (
				<OutputRoutesSetup
					routes={dmx?.outputRoutes ?? []}
					onSave={dmx?.saveOutputRoute ?? (async () => false)}
					onCreateRange={dmx?.createOutputRouteRange ?? (async () => false)}
					onDelete={dmx?.deleteOutputRoute ?? (async () => false)}
					outputBindIp={draft.output_bind_ip}
					usbEndpoints={usb.snapshot?.document.endpoints ?? []}
					usbDevices={usb.devices}
					usbBusy={usb.busy}
					usbError={usb.error}
					onScanUsbDevices={usb.scan}
					onProvisionUsbDevice={usb.provision}
				/>
			)}
			{controller.outputsTab === "audio" && (
				<AudioOutputSection controller={controller} />
			)}
		</>
	);
}
