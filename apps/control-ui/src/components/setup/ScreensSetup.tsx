import { useEffect, useRef, useState } from "react";
import { useServer } from "../../api/ServerContext";
import type { ControlDesk, ScreenConfiguration } from "../../api/types";
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
	desks,
	currentDeskId,
	onSelect,
	onClose,
}: {
	desks: ControlDesk[];
	currentDeskId?: string;
	onSelect: (id: string) => void;
	onClose: () => void;
}) {
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
					{desks.map((desk) => {
						const current = desk.id === currentDeskId;
						return (
							<article key={desk.id}>
								<span>
									<b>{desk.name}</b>
									<small>
										/{desk.osc_alias}/ · {desk.columns}×{desk.rows} ·{" "}
										{desk.buttons} buttons
									</small>
								</span>
								<Button
									disabled={current}
									variant={current ? "success" : "secondary"}
									onClick={() => onSelect(desk.id)}
								>
									{current ? "Current default screen" : "Use as default screen"}
								</Button>
							</article>
						);
					})}
				</WindowScrollArea>
			</section>
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

export function ScreensSetup({
	undoRef,
	onUndoAvailabilityChange,
}: {
	undoRef?: { current: (() => void) | null };
	onUndoAvailabilityChange?: (available: boolean) => void;
} = {}) {
	const server = useServer();
	const { state, dispatch } = useApp();
	const [displays, setDisplays] = useState<Array<{ id: string; name: string }>>(
		[],
	);
	const [deskPlaybackLayout, setDeskPlaybackLayout] = useState<PlaybackSurfaceLayout | null>(null);
	const [deskName, setDeskName] = useState("");
	const [deskAlias, setDeskAlias] = useState("");
	const [undoHistory, setUndoHistory] = useState<Array<{ desk: ControlDesk; regularNumberShortcuts: boolean }>>([]);
	const [defaultScreenPickerOpen, setDefaultScreenPickerOpen] = useState(false);
	const [defaultPlaybackModalOpen, setDefaultPlaybackModalOpen] = useState(false);
	const deskDraft = useRef<ControlDesk | null>(null);
	const deskSaveQueue = useRef(Promise.resolve());
	const pendingDeskSaves = useRef(0);
	const textEditRecorded = useRef({ name: false, osc_alias: false });
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
	const snapshot = () => {
		const desk = deskDraft.current ?? server.session?.desk;
		return desk ? { desk: structuredClone(desk), regularNumberShortcuts: state.regularNumberShortcuts } : null;
	};
	const rememberCurrent = () => {
		const current = snapshot();
		if (current) setUndoHistory((history) => [...history, current]);
	};
	const applyDesk = (next: ControlDesk, remember: boolean) => {
		const current = deskDraft.current ?? server.session?.desk;
		if (!current || JSON.stringify(current) === JSON.stringify(next)) return false;
		if (remember) rememberCurrent();
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
	const updateDesk = (changes: Partial<ControlDesk>, remember = true) => {
		const current = deskDraft.current ?? server.session?.desk;
		return current ? applyDesk({ ...current, ...changes }, remember) : false;
	};
	const updateText = (field: "name" | "osc_alias", value: string) => {
		const changed = updateDesk({ [field]: value }, !textEditRecorded.current[field]);
		if (changed) textEditRecorded.current[field] = true;
	};
	const updateKeyboardShortcuts = (value: boolean) => {
		if (value === state.regularNumberShortcuts) return;
		rememberCurrent();
		dispatch({ type: "SET_REGULAR_NUMBER_SHORTCUTS", value });
	};
	const undo = () => {
		const previous = undoHistory.at(-1);
		if (!previous) return;
		setUndoHistory((history) => history.slice(0, -1));
		applyDesk(previous.desk, false);
		if (previous.regularNumberShortcuts !== state.regularNumberShortcuts) {
			dispatch({ type: "SET_REGULAR_NUMBER_SHORTCUTS", value: previous.regularNumberShortcuts });
		}
	};
	if (undoRef) undoRef.current = undo;
	useEffect(() => {
		onUndoAvailabilityChange?.(undoHistory.length > 0);
	}, [undoHistory.length, onUndoAvailabilityChange]);
	useEffect(
		() => () => onUndoAvailabilityChange?.(false),
		[onUndoAvailabilityChange],
	);
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
						<div className="screen-settings-actions">
							<Button onClick={() => setDefaultPlaybackModalOpen(true)}>Configure Playbacks</Button>
							<Button onClick={() => setDefaultScreenPickerOpen(true)}>Choose default screen</Button>
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
							onFocus={() => { textEditRecorded.current.name = false; }}
							onBlur={() => { textEditRecorded.current.name = false; }}
							onChange={(event) => updateText("name", event.target.value)}
						/>
						<TextField
							label="OSC alias"
							value={deskAlias}
							onFocus={() => { textEditRecorded.current.osc_alias = false; }}
							onBlur={() => { textEditRecorded.current.osc_alias = false; }}
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
					desks={server.bootstrap?.desks ?? []}
					currentDeskId={server.session?.desk.id}
					onSelect={server.selectControlDesk}
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
