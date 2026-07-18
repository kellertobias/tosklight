import { useEffect, useRef, useState } from "react";
import { useScreens } from "../../features/screens/ScreensContext";
import type { ClientSummary, ControlDesk, ScreenConfiguration } from "../../api/types";
import { useApp } from "../../state/AppContext";
import {
	Button,
	FormLayout,
	ModalTitleBar,
	NumberField,
	SelectField,
	SwitchField,
	TextField,
} from "../common";
import { WindowScrollArea } from "../window-kit";
import { PlaybackLayoutModal } from "./PlaybackLayoutModal";
import {
	createScreenConfiguration,
	defaultDeskPlaybackLayout,
	playbackLayoutLegacyFields,
	screenPlaybackLayout,
} from "./screenConfiguration";
import type { PlaybackSurfaceLayout } from "../../api/types";

const tauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
async function invoke<T>(
	command: string,
	args?: Record<string, unknown>,
): Promise<T> {
	const api = await import("@tauri-apps/api/core");
	return api.invoke<T>(command, args);
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
	const [removeCandidate, setRemoveCandidate] = useState<ClientSummary | null>(null);
	const [removing, setRemoving] = useState(false);
	const [removeError, setRemoveError] = useState<string | null>(null);
	const sorted = [...clients].sort((left, right) =>
		Number(right.connected) - Number(left.connected) ||
		(right.last_connected_at ?? "").localeCompare(left.last_connected_at ?? "") ||
		left.name.localeCompare(right.name) || left.client_id.localeCompare(right.client_id));
	const groups = [
		{ heading: "Connected clients", clients: sorted.filter((client) => client.connected) },
		{ heading: "Disconnected clients", clients: sorted.filter((client) => !client.connected) },
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
					{groups.map((group) => <section className="default-screen-client-group" key={group.heading} aria-labelledby={`client-group-${group.heading.replaceAll(" ", "-").toLowerCase()}`}>
						<h3 id={`client-group-${group.heading.replaceAll(" ", "-").toLowerCase()}`}>{group.heading}</h3>
						{group.clients.map((client) => {
							const currentClient = client.client_id === currentClientId;
							const currentDefault = client.desk.id === currentDeskId;
							return <article key={client.client_id}>
								<div className="default-screen-client-details">
									<div className="default-screen-client-title"><b>{client.name}</b>{currentClient && <strong>Current client</strong>}{currentDefault && <strong>Current default screen</strong>}</div>
									<small>Client identity <code>{client.client_id}</code></small>
									<small>{client.connected ? "Connected" : "Disconnected"} · {client.last_connected_at ? `Last connected ${new Date(client.last_connected_at).toLocaleString()}` : "Last connected unknown"}</small>
									<small>Screen {client.desk.name} · /{client.desk.osc_alias}/ · {client.desk.columns}×{client.desk.rows} · {client.desk.buttons} buttons</small>
								</div>
								<div className="default-screen-client-actions">
									<Button disabled={currentDefault} variant={currentDefault ? "success" : "secondary"} onClick={() => onSelect(client.desk.id)}>{currentDefault ? "Current default screen" : "Use as default screen"}</Button>
									<Button variant="danger" disabled={!client.can_remove || currentClient || client.connected} title={currentClient ? "The current client cannot remove itself" : client.connected ? "Disconnect this client before removing it" : !client.can_remove ? "This screen configuration is in use by an active session" : undefined} onClick={() => { setRemoveError(null); setRemoveCandidate(client); }}>Remove client</Button>
								</div>
							</article>;
						})}
					</section>)}
				</WindowScrollArea>
				{removeError && <p className="default-screen-remove-error" role="alert">{removeError}</p>}
			</section>
			{removeCandidate && <div className="stacked-modal-layer"><section className="nested-modal default-screen-remove-confirm" role="alertdialog" aria-modal="true" aria-label={`Remove client ${removeCandidate.name}?`}>
				<ModalTitleBar title={`Remove client ${removeCandidate.name}?`}/>
				<p>Remove {removeCandidate.name} and its client registration, default-screen configuration, per-show page and playback selection, desk lock, Update defaults, and virtual-playback exclusion settings.</p>
				<p>Portable shows, users, optional screens, other clients, and installation-wide configuration will not change.</p>
				<div className="modal-actions"><Button disabled={removing} onClick={() => setRemoveCandidate(null)}>Cancel</Button><Button variant="danger" disabled={removing} onClick={() => { setRemoving(true); setRemoveError(null); void onRemove(removeCandidate.desk.id).then((removed) => { setRemoving(false); if (removed) setRemoveCandidate(null); else { setRemoveCandidate(null); setRemoveError(`${removeCandidate.name} could not be removed. It may have reconnected; disconnect it and try again.`); } }); }}>{removing ? "Removing…" : "Remove client"}</Button></div>
			</section></div>}
		</div>
	);
}

export function ScreenSettingsCard({
	screen,
	displays,
	save,
	remove,
}: {
	screen: ScreenConfiguration;
	displays: Array<{ id: string; name: string }>;
	save: (screen: ScreenConfiguration) => Promise<void>;
	remove: (screen: ScreenConfiguration) => Promise<void>;
}) {
	const [draft, setDraft] = useState(screen);
	const [playbackModalOpen, setPlaybackModalOpen] = useState(false);
	const draftRef = useRef(screen);
	const saveQueue = useRef(Promise.resolve());
	const pending = useRef(0);
	useEffect(() => {
		if (pending.current === 0) {
			draftRef.current = screen;
			setDraft(screen);
		}
	}, [screen]);
	const update = (changes: Partial<ScreenConfiguration>) => {
		const next = { ...draftRef.current, ...changes };
		draftRef.current = next;
		setDraft(next);
		pending.current += 1;
		saveQueue.current = saveQueue.current
			.then(() => save(next))
			.finally(() => {
				pending.current -= 1;
			});
	};
	return (
		<article className="screen-settings-card">
			<header className="screen-settings-header">
				<TextField
					aria-label="Screen name"
					value={draft.name}
					onChange={(event) => update({ name: event.target.value })}
				/>
				<div className="screen-settings-actions">
					<Button onClick={() => setPlaybackModalOpen(true)}>
						Configure Playbacks
					</Button>
					<Button
						variant={draft.desired_open ? "warning" : "success"}
						onClick={() => update({ desired_open: !draft.desired_open })}
					>
						{draft.desired_open ? "Close Screen" : "Open Screen"}
					</Button>
					<Button variant="danger" onClick={() => void remove(draft)}>
						Remove Screen
					</Button>
				</div>
			</header>
			<div className="screen-settings-columns">
				<section>
					<h3>Layout</h3>
					<div className="screen-settings-fields">
						<SwitchField
							label="Show Dock"
							checked={draft.show_dock}
							onChange={(event) => update({ show_dock: event.target.checked })}
						/>
						<SwitchField
							label="Show Playbacks"
							checked={draft.show_playbacks}
							onChange={(event) =>
								update({ show_playbacks: event.target.checked })
							}
						/>
						<SwitchField
							label="Show Page Controls"
							checked={draft.show_page_controls}
							onChange={(event) =>
								update({ show_page_controls: event.target.checked })
							}
						/>
					</div>
				</section>
				<section>
					<h3>Placement</h3>
					<div className="screen-settings-fields">
						<SelectField
							label="Physical Display"
							value={draft.display_id ?? ""}
							onChange={(value) => update({ display_id: value || null })}
							options={[
								{ value: "", label: "Choose when opened" },
								...displays.map((display) => ({
									value: display.id,
									label: display.name,
								})),
							]}
						/>
						<SwitchField
							label="Fullscreen"
							checked={draft.fullscreen}
							onChange={(event) => update({ fullscreen: event.target.checked })}
						/>
					</div>
				</section>
				<section>
					<h3>Playbacks</h3>
					<div className="screen-settings-fields">
						<p className="playback-layout-summary">
							{screenPlaybackLayout(draft).rows.length} rows · {screenPlaybackLayout(draft).playbacks_per_row} playbacks per row · {draft.page_mode === "follow_main" ? "Follow Main" : "Dedicated Page"}
						</p>
					</div>
				</section>
			</div>
			{playbackModalOpen && (
				<PlaybackLayoutModal
					initialLayout={screenPlaybackLayout(draft)}
					pageMode={draft.page_mode}
					onClose={() => setPlaybackModalOpen(false)}
					onSave={(playback_layout, page_mode) => {
						const legacy = playbackLayoutLegacyFields(playback_layout);
						update({ playback_layout, page_mode, playback_count: legacy.playback_count, playback_rows: legacy.playback_rows, first_playback_slot: legacy.first_playback_slot });
						setPlaybackModalOpen(false);
					}}
				/>
			)}
		</article>
	);
}

export function ScreensSetup() {
	const server = useScreens();
	const { state, dispatch } = useApp();
	const [displays, setDisplays] = useState<Array<{ id: string; name: string }>>(
		[],
	);
	const [deskPlaybackLayout, setDeskPlaybackLayout] = useState<PlaybackSurfaceLayout | null>(null);
	const [deskName, setDeskName] = useState("");
	const [deskAlias, setDeskAlias] = useState("");
	const [defaultScreenPickerOpen, setDefaultScreenPickerOpen] = useState(false);
	const [defaultPlaybackModalOpen, setDefaultPlaybackModalOpen] = useState(false);
	const deskDraft = useRef<ControlDesk | null>(null);
	const deskSaveQueue = useRef(Promise.resolve());
	const pendingDeskSaves = useRef(0);
	useEffect(() => {
		if (tauri)
			void invoke<Array<{ id: string; name: string }>>(
				"list_console_displays",
			).then(setDisplays);
	}, []);
	useEffect(() => {
		const desk = server.session?.desk;
		if (!desk || pendingDeskSaves.current > 0) return;
		deskDraft.current = desk;
		setDeskName(desk.name);
		setDeskAlias(desk.osc_alias);
		setDeskPlaybackLayout(defaultDeskPlaybackLayout(desk));
		dispatch({
			type: "SET_PLAYBACK_LAYOUT",
			columns: desk.columns,
			rows: desk.rows,
		});
	}, [server.session?.desk, dispatch]);
	const applyDesk = (next: ControlDesk) => {
		const current = deskDraft.current ?? server.session?.desk;
		if (!current || JSON.stringify(current) === JSON.stringify(next)) return false;
		deskDraft.current = next;
		setDeskName(next.name);
		setDeskAlias(next.osc_alias);
		const layout = defaultDeskPlaybackLayout(next);
		setDeskPlaybackLayout(layout);
		dispatch({ type: "SET_PLAYBACK_LAYOUT", columns: layout.playbacks_per_row, rows: layout.rows.length });
		pendingDeskSaves.current += 1;
		deskSaveQueue.current = deskSaveQueue.current
			.then(() => server.updateControlDesk(next))
			.finally(() => { pendingDeskSaves.current -= 1; });
		return true;
	};
	const updateDesk = (changes: Partial<ControlDesk>) => {
		const current = deskDraft.current ?? server.session?.desk;
		return current ? applyDesk({ ...current, ...changes }) : false;
	};
	const updateText = (field: "name" | "osc_alias", value: string) => {
		updateDesk({ [field]: value });
	};
	const updateKeyboardShortcuts = (value: boolean) => {
		if (value === state.regularNumberShortcuts) return;
		dispatch({ type: "SET_REGULAR_NUMBER_SHORTCUTS", value });
	};
	const create = () =>
		void server.saveScreen(
			createScreenConfiguration(server.screens?.screens ?? [], {
				desks: state.desks,
				activeDeskId: state.activeDeskId,
			}),
		);
	const remove = async (screen: ScreenConfiguration) => {
		await invoke("close_console_screen", { screenId: screen.id });
		await server.deleteScreen(screen.id);
	};
	return (
		<div className="screens-playback-setup">
			<header>
				<div>
					<h2>Screens & playback</h2>
					<p>
						Configure the default desk surface, installation-wide remote
						control, then optional operator screens.
					</p>
				</div>
				{tauri && (
					<Button variant="primary" onClick={create}>
						+ Add screen
					</Button>
				)}
			</header>
			<div className="screens-setup-list">
				<article className="default-screen-settings">
					<header>
						<div>
							<b>Default screen</b>
							<small>Primary desk window</small>
						</div>
					</header>
					<FormLayout
						className="screen-settings-grid"
						columns={3}
						minColumnWidth={180}
					>
						<TextField
							label="Name"
							value={deskName}
							onChange={(event) => updateText("name", event.target.value)}
						/>
						<TextField
							label="OSC alias"
							value={deskAlias}
							onChange={(event) => updateText("osc_alias", event.target.value)}
						/>
						<div className="playback-layout-summary">
							<b>Playback surface</b>
							<small>{deskPlaybackLayout?.rows.length ?? state.playbackRows} rows · {deskPlaybackLayout?.playbacks_per_row ?? state.playbackColumns} playbacks per row</small>
						</div>
						<SwitchField
							label="Enable software keyboard shortcuts"
							checked={state.regularNumberShortcuts}
							description="Keyboard shortcuts are always disabled while hardware controls are connected."
							onChange={(event) => updateKeyboardShortcuts(event.target.checked)}
						/>
					</FormLayout>
					<footer className="default-screen-status">
						<small>
							{state.playbackColumns * state.playbackRows} playback slots · OSC
							/light/{deskAlias || "desk"}/
						</small>
						<div className="screen-settings-actions default-screen-bottom-actions">
							<Button onClick={() => setDefaultPlaybackModalOpen(true)}>Configure Playbacks</Button>
							<Button onClick={() => setDefaultScreenPickerOpen(true)}>Choose default screen</Button>
						</div>
					</footer>
				</article>
				{!tauri && (
					<p>
						Additional console screens are available in the ToskLight desktop
						app.
					</p>
				)}
				{tauri &&
					(server.screens?.screens ?? []).map((screen) => (
						<ScreenSettingsCard
							key={screen.id}
							screen={screen}
							displays={displays}
							save={server.saveScreen}
							remove={remove}
						/>
					))}
			</div>
			{defaultScreenPickerOpen && (
				<DefaultScreenPicker
					clients={server.bootstrap?.clients ?? []}
					currentClientId={server.session?.client_id}
					currentDeskId={server.session?.desk.id}
					onSelect={server.selectControlDesk}
					onRemove={server.removeClient}
					onClose={() => setDefaultScreenPickerOpen(false)}
				/>
			)}
			{defaultPlaybackModalOpen && deskPlaybackLayout && (
				<PlaybackLayoutModal
					initialLayout={deskPlaybackLayout}
					pageMode="follow_main"
					pageModeLocked
					onClose={() => setDefaultPlaybackModalOpen(false)}
					onSave={(layout) => {
						const legacy = playbackLayoutLegacyFields(layout);
						updateDesk({
							columns: legacy.columns,
							rows: legacy.rows,
							buttons: legacy.buttons,
							playback_layout: layout,
						});
						setDefaultPlaybackModalOpen(false);
					}}
				/>
			)}
		</div>
	);
}
