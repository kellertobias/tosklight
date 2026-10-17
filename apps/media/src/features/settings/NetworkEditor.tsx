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
	/** Explicit Revert updates only restart-bound draft fields before another operator edit. */
	revertRevision?: number;
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
	revertRevision = 0,
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
	const intent = useRef(0);
	const editedFields = useRef(new Map<string, number>());
	const markIntent = (fields: string[]) => {
		const version = ++intent.current;
		for (const field of fields) editedFields.current.set(field, version);
	};
	const [editRevision, setEditRevision] = useState(0);
	const editedByUser = (field: string) => {
		markIntent([field]);
		setEditRevision((value) => value + 1);
	};
	useEffect(() => {
		const versions = new Map(editedFields.current);
		const adopt = <T,>(field: string, current: T, next: T): T => {
			const version = editedFields.current.get(field);
			if (version === undefined) return next;
			if (!busy && version === versions.get(field) && current === next)
				editedFields.current.delete(field);
			return current;
		};
		setPreset((current) =>
			adopt("sameComputerPreset", current, network.sameComputerPreset),
		);
		setListeners(
			(current) =>
				Object.fromEntries(
					LISTENERS.map(({ field }) => [
						field,
						adopt(field, current[field], network.stored[field]),
					]),
				) as Record<ListenField, string>,
		);
		setEndpoint((current) =>
			adopt(
				"speedGroupEndpoint",
				current,
				network.stored.speedGroupEndpoint ?? "",
			),
		);
	}, [network]);
	const reverted = useRef(revertRevision);
	const reverting = reverted.current !== revertRevision;
	useEffect(() => {
		if (reverted.current === revertRevision) return;
		reverted.current = revertRevision;
		queued.current = JSON.stringify({
			sameComputerPreset: network.activeSameComputerPreset,
			...listeners,
			citpListen: network.activeStored.citpListen,
			httpListen: network.activeStored.httpListen,
			speedGroupEndpoint: endpoint.trim() === "" ? null : endpoint.trim(),
		});
		markIntent(["sameComputerPreset", "citpListen", "httpListen"]);
		setPreset(network.activeSameComputerPreset);
		setListeners((current) => ({
			...current,
			citpListen: network.activeStored.citpListen,
			httpListen: network.activeStored.httpListen,
		}));
	}, [revertRevision, network]);
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
	const draft = {
		sameComputerPreset: preset,
		...listeners,
		speedGroupEndpoint: endpoint.trim() === "" ? null : endpoint.trim(),
	};
	const draftKey = JSON.stringify(draft);
	const queued = useRef<string | null>(null);
	const baseline = useRef(network);
	const attempted = useRef(0);
	const save = useRef({ onSave, draft });
	save.current = { onSave, draft };
	useEffect(() => {
		// The prop-sync effect above retires an acknowledged draft; do not enqueue its old render.
		if (reverting) return;
		if (baseline.current !== network) {
			baseline.current = network;
			queued.current = null;
			return;
		}
		if (attempted.current === editRevision) return;
		attempted.current = editRevision;
		if (
			showActions ||
			(!edited && queued.current === null) ||
			queued.current === draftKey ||
			!form.current?.checkValidity()
		)
			return;
		queued.current = draftKey;
		onChanged?.();
		save.current.onSave({ requestId: requestId(), ...save.current.draft });
	}, [
		network,
		showActions,
		edited,
		draftKey,
		editRevision,
		onChanged,
		reverting,
	]);

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
						onChange={(event) => {
							setListeners((current) => ({
								...current,
								[listener.field]: event.target.value,
							}));
							editedByUser(listener.field);
						}}
					/>
				))}
				<TextField
					label="Speed Groups"
					description="UDP, usually 0.0.0.0:4810. Tos Light Control sends its Speed Group tempos here over OSC. Leave empty to not follow a desk's Speed Groups. Applies immediately."
					value={endpoint}
					onChange={(event) => {
						setEndpoint(event.target.value);
						editedByUser("speedGroupEndpoint");
					}}
				/>
				<CheckboxField
					label="Light and Media are on this computer"
					stateLabel="Listen on 127.0.0.1"
					description="Listens on 127.0.0.1 without changing the addresses above, so they come back when you turn it off. Art-Net, sACN and Speed Groups move immediately; CITP and this interface on restart."
					checked={preset}
					onChange={(event) => {
						setPreset(event.target.checked);
						editedByUser("sameComputerPreset");
					}}
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
