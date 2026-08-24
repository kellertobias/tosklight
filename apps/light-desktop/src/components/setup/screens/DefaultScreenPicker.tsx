import { Button, ModalRegistration, ModalTitleBar } from "@tosklight/ui";
import { WindowScrollArea } from "@tosklight/ui/window-kit";
import { useState } from "react";
import type { ClientSummary } from "../../../api/types";

/** Every window shares the one desk, so a window is named by its own identity. */
function windowLabel(client: ClientSummary) {
	return `Window ${client.client_id.slice(0, 8)}`;
}

function knownClientGroupId(heading: string) {
	return `client-group-${heading.replaceAll(" ", "-").toLowerCase()}`;
}

function KnownClientGroup({
	heading,
	clients,
	currentClientId,
	onRemove,
}: {
	heading: string;
	clients: ClientSummary[];
	currentClientId?: string;
	onRemove: (client: ClientSummary) => void;
}) {
	const headingId = knownClientGroupId(heading);
	return (
		<section
			className="default-screen-client-group"
			aria-labelledby={headingId}
		>
			<h3 id={headingId}>{heading}</h3>
			{clients.map((client) => {
				const currentClient = client.client_id === currentClientId;
				const removeTitle = currentClient
					? "This window cannot remove itself"
					: client.connected
						? "Disconnect this window before removing it"
						: undefined;
				return (
					<article key={client.client_id}>
						<div className="default-screen-client-details">
							<div className="default-screen-client-title">
								<b>{windowLabel(client)}</b>
								{currentClient && <strong>This window</strong>}
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
						</div>
						<div className="default-screen-client-actions">
							<Button
								variant="danger"
								disabled={
									!client.can_remove || currentClient || client.connected
								}
								title={removeTitle}
								onClick={() => onRemove(client)}
							>
								Forget window
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
		<ModalRegistration onClose={onCancel}>
			<div className="stacked-modal-layer">
				<section
					className="nested-modal default-screen-remove-confirm"
					role="alertdialog"
					aria-modal="true"
					aria-label={`Forget ${windowLabel(client)}?`}
				>
					<ModalTitleBar title={`Forget ${windowLabel(client)}?`} />
					<p>
						Remove this window's registration. It is only a record that the
						window has connected before.
					</p>
					<p>
						The desk itself is untouched: its page, playback selection, desk
						lock, Update defaults, screens, shows, and installation-wide
						configuration all stay as they are. The window can connect again at
						any time.
					</p>
					<div className="modal-actions">
						<Button disabled={removing} onClick={onCancel}>
							Cancel
						</Button>
						<Button variant="danger" disabled={removing} onClick={onConfirm}>
							{removing ? "Forgetting…" : "Forget window"}
						</Button>
					</div>
				</section>
			</div>
		</ModalRegistration>
	);
}

export function KnownClientsModal({
	clients,
	currentClientId,
	onRemove,
	onRemoveAll,
	onClose,
}: {
	clients: ClientSummary[];
	currentClientId?: string;
	onRemove: (client: ClientSummary) => Promise<boolean>;
	onRemoveAll: () => Promise<boolean>;
	onClose: () => void;
}) {
	const [removeCandidate, setRemoveCandidate] = useState<ClientSummary | null>(
		null,
	);
	const [removing, setRemoving] = useState(false);
	const [removeError, setRemoveError] = useState<string | null>(null);
	const [removeAllOpen, setRemoveAllOpen] = useState(false);
	const sorted = [...clients].sort(
		(left, right) =>
			Number(right.connected) - Number(left.connected) ||
			(right.last_connected_at ?? "").localeCompare(
				left.last_connected_at ?? "",
			) ||
			left.client_id.localeCompare(right.client_id),
	);
	const groups = [
		{
			heading: "Connected windows",
			clients: sorted.filter((client) => client.connected),
		},
		{
			heading: "Disconnected windows",
			clients: sorted.filter((client) => !client.connected),
		},
	].filter((group) => group.clients.length > 0);
	return (
		<ModalRegistration onClose={onClose}>
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
					aria-label="Known windows"
				>
					<ModalTitleBar
						title="Known windows"
						groups={[
							{
								id: "client-cleanup",
								actions: [
									{
										id: "forget-other-windows",
										label: "Forget other windows",
										variant: "danger",
										onPress: () => setRemoveAllOpen(true),
									},
								],
							},
						]}
						closeLabel="Close known windows"
						onClose={onClose}
					/>
					<p>
						Every window operates the one desk. This is which windows have
						connected to it; forget a disconnected one to drop its record.
					</p>
					<WindowScrollArea className="default-screen-client-list">
						{groups.map((group) => (
							<KnownClientGroup
								key={group.heading}
								heading={group.heading}
								clients={group.clients}
								currentClientId={currentClientId}
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
							void onRemove(removeCandidate).then((removed) => {
								setRemoving(false);
								setRemoveCandidate(null);
								if (!removed) {
									setRemoveError(
										`${windowLabel(removeCandidate)} could not be forgotten. It may have reconnected; disconnect it and try again.`,
									);
								}
							});
						}}
					/>
				)}
				{removeAllOpen && (
					<ModalRegistration onClose={() => setRemoveAllOpen(false)}>
						<div className="stacked-modal-layer">
							<section
								className="nested-modal default-screen-remove-confirm"
								role="alertdialog"
								aria-modal="true"
								aria-label="Forget other windows?"
							>
								<ModalTitleBar title="Forget other windows?" />
								<p>
									Forget every disconnected window. This window and connected
									windows are left alone.
								</p>
								<p>
									The desk, its configuration, shows, screens, and
									installation-wide settings will not change.
								</p>
								<div className="modal-actions">
									<Button
										disabled={removing}
										onClick={() => setRemoveAllOpen(false)}
									>
										Cancel
									</Button>
									<Button
										variant="danger"
										disabled={removing}
										onClick={() => {
											setRemoving(true);
											setRemoveError(null);
											void onRemoveAll().then((removed) => {
												setRemoving(false);
												setRemoveAllOpen(false);
												if (!removed)
													setRemoveError(
														"Some clients could not be removed because their state changed.",
													);
											});
										}}
									>
										{removing ? "Forgetting…" : "Forget other windows"}
									</Button>
								</div>
							</section>
						</div>
					</ModalRegistration>
				)}
			</div>
		</ModalRegistration>
	);
}
