import { FormLayout, NumberField, TextField } from "@tosklight/ui";
import { useState } from "react";
import type { UsbDmxEndpointSnapshot } from "../../api/client/deskManagement";
import { OutputRoutesSetup } from "../../components/setup/OutputRoutesSetup";
import { UsbDmxEndpointsSetup } from "../../components/setup/UsbDmxEndpointsSetup";
import { useDmxDiagnostics } from "../../features/dmxDiagnostics/DmxDiagnosticsContext";
import type { SetupWindowController } from "./controller";

export function OutputsSection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const { draft } = controller;
	const dmx = useDmxDiagnostics();
	const [usb, setUsb] = useState<UsbDmxEndpointSnapshot | null>(null);
	if (!draft) return null;
	return (
		<>
			<h2>Output engine</h2>
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
			<UsbDmxEndpointsSetup onSnapshot={setUsb} />
			<OutputRoutesSetup
				routes={dmx?.outputRoutes ?? []}
				onSave={dmx?.saveOutputRoute ?? (async () => false)}
				onCreateRange={dmx?.createOutputRouteRange ?? (async () => false)}
				onDelete={dmx?.deleteOutputRoute ?? (async () => false)}
				outputBindIp={draft.output_bind_ip}
				usbEndpoints={usb?.document.endpoints ?? []}
			/>
		</>
	);
}
