import { useState } from "react";
import type { ClientSummary } from "../../../api/types";
import { Button, ModalTitleBar } from "../../common";
import { WindowScrollArea } from "../../window-kit";

function defaultScreenGroupId(heading: string) {
	return `client-group-${heading.replaceAll(" ", "-").toLowerCase()}`;
}

function DefaultScreenClientGroup({
	heading,
	clients,
	currentClientId,
	currentDeskId,
	onSelect,
	onRemove,
}: {
	heading: string;
	clients: ClientSummary[];
	currentClientId?: string;
	currentDeskId?: string;
	onSelect: (id: string) => void;
	onRemove: (client: ClientSummary) => void;
}) {
	const headingId = defaultScreenGroupId(heading);
	return (
		<section
			className="default-screen-client-group"
			aria-labelledby={headingId}
		>
			<h3 id={headingId}>{heading}</h3>
			{clients.map((client) => {
				const currentClient = client.client_id === currentClientId;
				const currentDefault = client.desk.id === currentDeskId;
				const removeTitle = currentClient
					? "The current client cannot remove itself"
					: client.connected
						? "Disconnect this client before removing it"
						: !client.can_remove
							? "This screen configuration is in use by an active session"
							: undefined;
				return (
					<article key={client.client_id}>
						<div className="default-screen-client-details">
							<div className="default-screen-client-title">
								<b>{client.name}</b>
								{currentClient && <strong>Current client</strong>}
								{currentDefault && <strong>Current default screen</strong>}
							</div>
							<small>
								Client identity <code>{client.client_id}</code>
							</small>
							<small>
								{client.connected ? "Connected" : "Disconnected"} ·{" "}
								{client.last_connected_at
									? `Last connected ${new Date(client.last_connected_at).toLocaleString()}`
									: "Last connected unknown"}
							</small>
							<small>
								Screen {client.desk.name} · /{client.desk.osc_alias}/ ·{" "}
								{client.desk.columns}×{client.desk.rows} · {client.desk.buttons}{" "}
								buttons
							</small>
						</div>
						<div className="default-screen-client-actions">
							<Button
								disabled={currentDefault}
								variant={currentDefault ? "success" : "secondary"}
								onClick={() => onSelect(client.desk.id)}
							>
								{currentDefault
									? "Current default screen"
									: "Use as default screen"}
							</Button>
							<Button
								variant="danger"
								disabled={
									!client.can_remove || currentClient || client.connected
								}
								title={removeTitle}
								onClick={() => onRemove(client)}
							>
								Remove client
							</Button>
						</div>
					</article>
				);
			})}
		</section>
	);
}

function RemoveClientConfirmation({
	client,
	removing,
	onCancel,
	onConfirm,
}: {
	client: ClientSummary;
	removing: boolean;
	onCancel: () => void;
	onConfirm: () => void;
}) {
	return (
		<div className="stacked-modal-layer">
			<section
				className="nested-modal default-screen-remove-confirm"
				role="alertdialog"
				aria-modal="true"
				aria-label={`Remove client ${client.name}?`}
			>
				<ModalTitleBar title={`Remove client ${client.name}?`} />
				<p>
					Remove {client.name} and its client registration, default-screen
					configuration, per-show page and playback selection, desk lock, Update
					defaults, and virtual-playback exclusion settings.
				</p>
				<p>
					Portable shows, users, optional screens, other clients, and
					installation-wide configuration will not change.
				</p>
				<div className="modal-actions">
					<Button disabled={removing} onClick={onCancel}>
						Cancel
					</Button>
					<Button variant="danger" disabled={removing} onClick={onConfirm}>
						{removing ? "Removing…" : "Remove client"}
					</Button>
				</div>
			</section>
		</div>
	);
}

export function DefaultScreenPicker({
	clients,
	currentClientId,
	currentDeskId,
	onSelect,
	onRemove,
	onClose,
}: {
	clients: ClientSummary[];
	currentClientId?: string;
	currentDeskId?: string;
	onSelect: (id: string) => void;
	onRemove: (deskId: string) => Promise<boolean>;
	onClose: () => void;
}) {
	const [removeCandidate, setRemoveCandidate] = useState<ClientSummary | null>(
		null,
	);
	const [removing, setRemoving] = useState(false);
	const [removeError, setRemoveError] = useState<string | null>(null);
	const sorted = [...clients].sort(
		(left, right) =>
			Number(right.connected) - Number(left.connected) ||
			(right.last_connected_at ?? "").localeCompare(
				left.last_connected_at ?? "",
			) ||
			left.name.localeCompare(right.name) ||
			left.client_id.localeCompare(right.client_id),
	);
	const groups = [
		{
			heading: "Connected clients",
			clients: sorted.filter((client) => client.connected),
		},
		{
			heading: "Disconnected clients",
			clients: sorted.filter((client) => !client.connected),
		},
	].filter((group) => group.clients.length > 0);
	return (
		<div
			className="stacked-modal-layer"
			onPointerDown={(event) =>
				event.target === event.currentTarget && onClose()
			}
		>
			<section
				className="nested-modal default-screen-picker"
				role="dialog"
				aria-modal="true"
				aria-label="Choose default screen"
			>
				<ModalTitleBar
					title="Choose default screen"
					closeLabel="Close default screen chooser"
					onClose={onClose}
				/>
				<p>
					Choose which known client configuration this app should use as its
					default screen.
				</p>
				<WindowScrollArea className="default-screen-client-list">
					{groups.map((group) => (
						<DefaultScreenClientGroup
							key={group.heading}
							heading={group.heading}
							clients={group.clients}
							currentClientId={currentClientId}
							currentDeskId={currentDeskId}
							onSelect={onSelect}
							onRemove={(client) => {
								setRemoveError(null);
								setRemoveCandidate(client);
							}}
						/>
					))}
				</WindowScrollArea>
				{removeError && (
					<p className="default-screen-remove-error" role="alert">
						{removeError}
					</p>
				)}
			</section>
			{removeCandidate && (
				<RemoveClientConfirmation
					client={removeCandidate}
					removing={removing}
					onCancel={() => setRemoveCandidate(null)}
					onConfirm={() => {
						setRemoving(true);
						setRemoveError(null);
						void onRemove(removeCandidate.desk.id).then((removed) => {
							setRemoving(false);
							setRemoveCandidate(null);
							if (!removed) {
								setRemoveError(
									`${removeCandidate.name} could not be removed. It may have reconnected; disconnect it and try again.`,
								);
							}
						});
					}}
				/>
			)}
		</div>
	);
}
