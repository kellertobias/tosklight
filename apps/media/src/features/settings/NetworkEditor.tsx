// The network settings.
//
// Every address here is somewhere this server waits, and `0.0.0.0` means every interface on this
// machine. The Media Server sends nothing a desk configures here: it only receives, including the
// Speed Groups a Light desk publishes.
//
// What was typed and what this run actually bound are both shown, because the same-computer preset
// makes them differ on purpose.
//
// Art-Net, sACN and Speed Groups are receive-only UDP listeners the server moves as soon as a
// change is saved. CITP and this interface keep their sockets until the next start: consoles hold
// TCP sessions to CITP, and this interface is the page making the edit.

import { Button, CheckboxField, TextField } from "@tosklight/ui/controls";
import { useEffect, useRef, useState } from "react";
import { requestId } from "../../shared/api/editing";
import type {
	NetworkView,
	UpdateNetwork,
} from "../../shared/api/generated/media-wire";

export interface NetworkEditorProps {
	formId?: string;
	network: NetworkView;
	busy: boolean;
	onSave: (edit: UpdateNetwork) => void;
	onCancel?: () => void;
	showActions?: boolean;
	onChanged?: () => void;
}

/// Each listen address, with what it is for. The order is the order an operator meets them in.
const LISTENERS = [
	{
		field: "artNetListen",
		label: "Art-Net",
		description:
			"UDP 6454. A desk sends layer values here. Applies immediately.",
	},
	{
		field: "sacnListen",
		label: "sACN",
		description: "UDP 5568. The same values over E1.31. Applies immediately.",
	},
	{
		field: "citpListen",
		label: "CITP",
		description:
			"TCP 4809. A console discovers this server and watches its preview here. Applies on restart.",
	},
	{
		field: "httpListen",
		label: "This interface",
		description:
			"The administration interface you are reading. Applies on restart.",
	},
] as const;

type ListenField = (typeof LISTENERS)[number]["field"];

export function NetworkEditor({
	formId,
	network,
	busy,
	onSave,
	onCancel,
	showActions = true,
	onChanged,
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
	const form = useRef<HTMLFormElement>(null);
	useEffect(() => {
		setPreset(network.sameComputerPreset);
		setListeners({
			artNetListen: network.stored.artNetListen,
			sacnListen: network.stored.sacnListen,
			citpListen: network.stored.citpListen,
			httpListen: network.stored.httpListen,
		});
		setEndpoint(network.stored.speedGroupEndpoint ?? "");
	}, [network]);
	// Only a draft that differs from what the server stored is saved. Comparing values rather than
	// counting renders keeps a page load, a reload after a save, or React's development double
	// effects from sending an edit nobody made.
	const edited =
		preset !== network.sameComputerPreset ||
		LISTENERS.some(
			(listener) =>
				listeners[listener.field] !== network.stored[listener.field],
		) ||
		endpoint.trim() !== (network.stored.speedGroupEndpoint ?? "");
	useEffect(() => {
		if (showActions || !edited) return;
		onChanged?.();
		const timer = window.setTimeout(() => form.current?.requestSubmit(), 350);
		return () => window.clearTimeout(timer);
	}, [preset, listeners, endpoint, edited, onChanged, showActions]);

	return (
		<form
			ref={form}
			id={formId}
			className="media-settings-form"
			onSubmit={(event) => {
				event.preventDefault();
				onSave({
					requestId: requestId(),
					sameComputerPreset: preset,
					...listeners,
					// An empty field turns Speed Group reception off, which the API takes as an
					// explicit null rather than as a field left alone.
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
				<TextField
					label="Speed Groups"
					description="UDP, usually 0.0.0.0:4810. Tos Light Control sends its Speed Group tempos here over OSC. Leave empty to not follow a desk's Speed Groups. Applies immediately."
					value={endpoint}
					onChange={(event) => setEndpoint(event.target.value)}
				/>
				<CheckboxField
					label="Light and Media are on this computer"
					stateLabel="Listen on 127.0.0.1"
					description="Listens on 127.0.0.1 without changing the addresses above, so they come back when you turn it off. Art-Net, sACN and Speed Groups move immediately; CITP and this interface on restart."
					checked={preset}
					onChange={(event) => setPreset(event.target.checked)}
				/>
			</fieldset>

			{showActions && (
				<div className="media-settings-actions">
					<Button type="submit" variant="primary" loading={busy}>
						Save network settings
					</Button>
					{onCancel && <Button onClick={onCancel}>Cancel</Button>}
				</div>
			)}
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
					<th scope="row">Speed Groups</th>
					<td>
						<code>{network.stored.speedGroupEndpoint ?? "not receiving"}</code>
					</td>
					<td>
						<code>
							{network.resolved.speedGroupEndpoint ?? "not receiving"}
						</code>
					</td>
				</tr>
			</tbody>
		</table>
	);
}
