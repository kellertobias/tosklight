// The network settings.
//
// Listen addresses and destinations are shown as two groups, because they are two different things
// and confusing them is the mistake this panel exists to prevent: a listen address is somewhere
// this server waits, and `0.0.0.0` means every interface on this machine. A destination is
// somewhere it sends, and `0.0.0.0` is never one.
//
// What was typed and what this run actually bound are both shown, because the same-computer preset
// makes them differ on purpose.

import { Button, CheckboxField, TextField } from "@tosklight/ui/controls";
import { useState } from "react";
import { requestId } from "../../shared/api/editing";
import type {
	NetworkView,
	UpdateNetwork,
} from "../../shared/api/generated/media-wire";

export interface NetworkEditorProps {
	network: NetworkView;
	busy: boolean;
	onSave: (edit: UpdateNetwork) => void;
	onCancel: () => void;
}

/// Each listen address, with what it is for. The order is the order an operator meets them in.
const LISTENERS = [
	{
		field: "artNetListen",
		label: "Art-Net",
		description: "UDP 6454. A desk sends layer values here.",
	},
	{
		field: "sacnListen",
		label: "sACN",
		description: "UDP 5568. The same values over E1.31.",
	},
	{
		field: "citpListen",
		label: "CITP",
		description:
			"TCP 4809. A console discovers this server and watches its preview here.",
	},
	{
		field: "httpListen",
		label: "This interface",
		description: "The administration interface you are reading.",
	},
] as const;

type ListenField = (typeof LISTENERS)[number]["field"];

export function NetworkEditor({
	network,
	busy,
	onSave,
	onCancel,
}: NetworkEditorProps) {
	const [preset, setPreset] = useState(network.sameComputerPreset);
	const [listeners, setListeners] = useState<Record<ListenField, string>>({
		artNetListen: network.stored.artNetListen,
		sacnListen: network.stored.sacnListen,
		citpListen: network.stored.citpListen,
		httpListen: network.stored.httpListen,
	});
	const [endpoint, setEndpoint] = useState(
		network.stored.speedGroupEndpoint ?? "",
	);

	return (
		<form
			className="media-settings-form"
			onSubmit={(event) => {
				event.preventDefault();
				onSave({
					requestId: requestId(),
					sameComputerPreset: preset,
					...listeners,
					// An empty field means "no destination", which the API takes as an explicit
					// null rather than as a field left alone.
					speedGroupEndpoint: endpoint.trim() === "" ? null : endpoint.trim(),
				});
			}}
		>
			<fieldset>
				<legend>Where this server listens</legend>
				<p className="media-settings-note">
					An address and a port, such as <code>0.0.0.0:6454</code> for every
					interface on this machine, or one interface's own address to listen
					only there.
				</p>
				{LISTENERS.map((listener) => (
					<TextField
						key={listener.field}
						label={listener.label}
						description={listener.description}
						value={listeners[listener.field]}
						onChange={(event) =>
							setListeners((current) => ({
								...current,
								[listener.field]: event.target.value,
							}))
						}
					/>
				))}
				<CheckboxField
					label="Light and Media are on this computer"
					stateLabel="Listen on 127.0.0.1"
					description="Listens on 127.0.0.1 for this run without changing the addresses above, so they come back when you turn it off."
					checked={preset}
					onChange={(event) => setPreset(event.target.checked)}
				/>
			</fieldset>

			<fieldset>
				<legend>Where this server sends</legend>
				<TextField
					label="Speed Group stream"
					description="Where the Light desk publishes its Speed Groups. Leave empty when Media is not following one. A real address, never 0.0.0.0."
					value={endpoint}
					onChange={(event) => setEndpoint(event.target.value)}
				/>
			</fieldset>

			<div className="media-settings-actions">
				<Button type="submit" variant="primary" loading={busy}>
					Save
				</Button>
				<Button onClick={onCancel}>Cancel</Button>
			</div>
		</form>
	);
}

/// What this run bound, beside what was typed, so an operator can always see the difference.
export function BoundAddresses({ network }: { network: NetworkView }) {
	return (
		<table className="media-table">
			<caption>Network</caption>
			<thead>
				<tr>
					<th scope="col">Protocol</th>
					<th scope="col">Configured</th>
					<th scope="col">In use now</th>
				</tr>
			</thead>
			<tbody>
				{LISTENERS.map((listener) => (
					<tr key={listener.field}>
						<th scope="row">{listener.label}</th>
						<td>
							<code>{network.stored[listener.field]}</code>
						</td>
						<td>
							<code>{network.resolved[listener.field]}</code>
						</td>
					</tr>
				))}
				<tr>
					<th scope="row">Speed Group stream</th>
					<td>
						<code>
							{network.stored.speedGroupEndpoint ?? "not following one"}
						</code>
					</td>
					<td>
						<code>
							{network.resolved.speedGroupEndpoint ?? "not following one"}
						</code>
					</td>
				</tr>
			</tbody>
		</table>
	);
}
