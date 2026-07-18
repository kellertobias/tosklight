import { FormLayout, NumberField, TextField } from "../../components/common";
import { OutputRoutesSetup } from "../../components/setup/OutputRoutesSetup";
import type { SetupWindowController } from "./controller";

export function OutputsSection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const { draft, server } = controller;
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
			</FormLayout>
			<OutputRoutesSetup
				routes={server.outputRoutes}
				onSave={server.saveOutputRoute}
				onDelete={server.deleteOutputRoute}
				outputBindIp={draft.output_bind_ip}
			/>
		</>
	);
}
