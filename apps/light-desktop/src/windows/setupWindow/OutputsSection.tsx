import { FormLayout, NumberField, TextField } from "../../components/common";
import { useDmxDiagnostics } from "../../features/dmxDiagnostics/DmxDiagnosticsContext";
import { OutputRoutesSetup } from "../../components/setup/OutputRoutesSetup";
import type { SetupWindowController } from "./controller";

export function OutputsSection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const { draft } = controller;
	const dmx = useDmxDiagnostics();
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
					max="44"
					value={draft.frame_rate_hz}
					onChange={(event) =>
						controller.editDraft({
							...draft,
							frame_rate_hz: Number(event.target.value),
						})
					}
					description="40–44 Hz"
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
				<NumberField
					label="Autosave interval"
					min="5"
					max="3600"
					value={draft.autosave_interval_seconds}
					onChange={(event) =>
						controller.editDraft({
							...draft,
							autosave_interval_seconds: Number(event.target.value),
						})
					}
					description="5–3600 s between recovery checkpoints"
				/>
			</FormLayout>
			<OutputRoutesSetup
				routes={dmx?.outputRoutes ?? []}
				onSave={dmx?.saveOutputRoute ?? (async () => false)}
				onDelete={dmx?.deleteOutputRoute ?? (async () => false)}
				outputBindIp={draft.output_bind_ip}
			/>
		</>
	);
}
