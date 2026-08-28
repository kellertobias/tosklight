import { FormLayout, NumberField, SwitchField, TextField } from "@tosklight/ui";
import type { PsnConfiguration, PsnEdit } from "../../api/client/psn";

/**
 * Where the desk listens, and whether it is listening at all.
 *
 * Each field commits when it loses focus rather than on every keystroke: a half-typed multicast
 * address is not an address, and refusing one letter at a time would be nothing but noise.
 */
export function PsnSourceForm({
	configuration,
	busy,
	onEdit,
}: {
	configuration: PsnConfiguration;
	busy: boolean;
	onEdit: (edit: PsnEdit) => void;
}) {
	const edit = onEdit;
	return (
	<FormLayout columns={2}>
		<SwitchField
			label="Receive PosiStageNet"
			offLabel="Off"
			onLabel="Listening"
			checked={configuration.enabled}
			disabled={busy}
			onChange={(event) => void edit({ enabled: event.target.checked })}
		/>
		<TextField
			label="Multicast group"
			defaultValue={configuration.group}
			key={`group-${configuration.group}`}
			onBlur={(event) => {
				const group = event.target.value.trim();
				if (group && group !== configuration.group) void edit({ group });
			}}
		/>
		<NumberField
			label="Port"
			defaultValue={configuration.port}
			key={`port-${configuration.port}`}
			onBlur={(event) => {
				const port = Number(event.target.value);
				if (port && port !== configuration.port) void edit({ port });
			}}
		/>
		<NumberField
			label="Stale after (ms)"
			description="How long without a packet before a tracker is called stale. A stale tracker still holds its last position."
			defaultValue={configuration.staleAfterMillis}
			key={`stale-${configuration.staleAfterMillis}`}
			onBlur={(event) => {
				const staleAfterMillis = Number(event.target.value);
				if (
					staleAfterMillis &&
					staleAfterMillis !== configuration.staleAfterMillis
				)
					void edit({ staleAfterMillis });
			}}
		/>
	</FormLayout>
	);
}
