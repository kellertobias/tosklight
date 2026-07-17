import { useEffect, useRef, useState } from "react";
import { configuredServerUrl } from "../api/LightApiClient";
import { useServer } from "../api/ServerContext";
import type { DeskConfiguration, UpdateSettings } from "../api/types";
import {
	Button,
	FormField,
	FormLayout,
	ModalTitleBar,
	NumberField,
	SwitchField,
	TextField,
} from "../components/common";
import { DeskLockSettingsModal } from "../components/setup/DeskLockSettingsModal";
import { FixtureLibrarySetup } from "../components/setup/FixtureLibrarySetup";
import { MatterBridgeSettings } from "../components/setup/MatterBridgeSettings";
import { OutputRoutesSetup } from "../components/setup/OutputRoutesSetup";
import {
	loadRecordSettings,
	RecordDefaultsFields,
	type RecordSettings,
	saveRecordSettings,
	UpdateDefaultsFields,
} from "../components/setup/ProgrammerDefaults";
import { ScreensSetup } from "../components/setup/ScreensSetup";
import { ShowRecoveryFileManager } from "../components/setup/ShowRecoveryFileManager";
import { WindowHeader, WindowScrollArea } from "../components/window-kit";
import type { WindowProps } from "./windowTypes";
import { defaultUpdateSettings } from "../components/control/updateWorkflow";

const sections = [
	"Shows & recovery",
	"Users & sessions",
	"Programmer",
	"Outputs",
	"Timecode",
	"Network & Inputs",
	"Screens & playback",
];

export function SetupWindow(_: WindowProps) {
	const server = useServer();
	const [section, setSection] = useState(0);
	const [draft, setDraft] = useState<DeskConfiguration | null>(
		server.configuration,
	);
	const [recordSettings, setRecordSettings] =
		useState<RecordSettings>(loadRecordSettings);
	const [updateSettings, setUpdateSettings] =
		useState<UpdateSettings>(defaultUpdateSettings);
	const [programmerSettingsLoaded, setProgrammerSettingsLoaded] =
		useState(false);
	const [programmerSettingsError, setProgrammerSettingsError] = useState<
		string | null
	>(null);
	const [restartRequired, setRestartRequired] = useState(false);
	const [serverUrl, setServerUrl] = useState(configuredServerUrl());
	const [fixtureLibraryOpen, setFixtureLibraryOpen] = useState(false);
	const [deskLockSettingsOpen, setDeskLockSettingsOpen] = useState(false);
	const draftRevision = useRef(0);
	const draftDirty = useRef(false);
	const pendingConfigurationSave = useRef<{
		revision: number;
		configuration: DeskConfiguration;
	} | null>(null);
	useEffect(() => {
		const pending = pendingConfigurationSave.current;
		if (
			pending &&
			JSON.stringify(pending.configuration) ===
				JSON.stringify(server.configuration)
		) {
			pendingConfigurationSave.current = null;
			if (draftRevision.current === pending.revision) {
				draftDirty.current = false;
				setDraft(server.configuration);
			}
			return;
		}
		if (!draftDirty.current) setDraft(server.configuration);
	}, [server.configuration]);
	useEffect(() => {
		if (section !== 2) return;
		let active = true;
		setRecordSettings(loadRecordSettings());
		setProgrammerSettingsError(null);
		void server.updateSettings().then((settings) => {
			if (!active) return;
			setUpdateSettings(settings ?? defaultUpdateSettings);
			setProgrammerSettingsLoaded(true);
			if (!settings) {
				setProgrammerSettingsError(
					"Update defaults could not be loaded; deterministic defaults are shown.",
				);
			}
		});
		return () => {
			active = false;
		};
	}, [section]);
	const editDraft = (next: DeskConfiguration) => {
		draftRevision.current += 1;
		draftDirty.current = true;
		setDraft(next);
	};

	const save = async () => {
		if (!draft) return;
		pendingConfigurationSave.current = {
			revision: draftRevision.current,
			configuration: draft,
		};
		const [requiresRestart, updateSaved] = await Promise.all([
			server.saveConfiguration(draft),
			section === 2 && programmerSettingsLoaded
				? server.saveUpdateSettings(updateSettings)
				: Promise.resolve(true),
		]);
		if (section === 2) saveRecordSettings(recordSettings);
		setRestartRequired(requiresRestart);
		setProgrammerSettingsError(
			updateSaved ? null : "Update defaults were not saved.",
		);
	};
	return (
		<div className="setup-window">
			<WindowHeader
				title="Desk Setup"
				info={{
					primary: sections[section],
					secondary: restartRequired ? "Restart required" : undefined,
				}}
				actions={section === 6 ? [[
						{ id: "desk-lock", label: "Desk Lock", onClick: () => setDeskLockSettingsOpen(true) },
					]] : [[
						{
							id: "save",
							label: "Save changes",
							disabled: !draft,
							onClick: () => void save(),
						},
					]]}
			/>
			<div className="setup-window-body">
				<nav>
					{sections.map((name, index) => (
						<Button
							onClick={() => setSection(index)}
							className={index === section ? "active" : ""}
							key={name}
						>
							{name}
						</Button>
					))}
				</nav>
				<main>
					<WindowScrollArea className="setup-content-scroll">
						<div className="setup-content">
							{section === 0 && (
								<>
									<h2>Shows & recovery</h2>
									<div className="setup-cards">
										<section>
											<b>
												{server.bootstrap?.active_show?.name ??
													"No show loaded"}
											</b>
											<small>
												{server.bootstrap?.active_show?.updated_at ??
													"Choose a show from the library"}
											</small>
										</section>
										<section>
											<b>{server.shows.length} library shows</b>
											<small>Portable SQLite files</small>
										</section>
										<section>
											<b>{server.status}</b>
											<small>
												{server.bootstrap?.active_show
													? "Autosave active"
													: "No active show"}
											</small>
										</section>
									</div>
									<ShowRecoveryFileManager onOpenFixtureLibrary={() => setFixtureLibraryOpen(true)} />
								</>
							)}
							{section === 1 && (
								<>
									<h2>Users & sessions</h2>
									<div className="setup-list">
										{server.bootstrap?.users.map((user) => (
											<article key={user.id}>
												<b>{user.name}</b>
												<span>{user.enabled ? "Enabled" : "Disabled"}</span>
												<small>
													{user.id === server.session?.user.id
														? "Current operator"
														: user.id}
												</small>
												{user.enabled &&
													user.id !== server.session?.user.id && (
														<Button
															onClick={() => server.switchUser(user.name)}
														>
															Use this operator
														</Button>
													)}
											</article>
										))}
									</div>
								</>
							)}
							{section === 2 && (
								<>
									<h2>Programmer</h2>
									<div className="setup-list programmer-setup-list">
										<article>
											<header>
												<b>Record defaults</b>
												<small>Also available by holding Record.</small>
											</header>
											<RecordDefaultsFields
												settings={recordSettings}
												onChange={setRecordSettings}
											/>
										</article>
										<article>
											<header>
												<b>Update defaults</b>
												<small>Also available by holding Update.</small>
											</header>
											<UpdateDefaultsFields
												settings={updateSettings}
												onChange={setUpdateSettings}
											/>
										</article>
										<h3 className="programmer-setup-section-title">Preload</h3>
										{draft && (
											<article>
												<header>
													<b>Preload capture</b>
												</header>
												<FormLayout labelPlacement="side">
													<SwitchField
														label="Preload programmer changes"
														checked={draft.preload_programmer_changes}
														onChange={(event) =>
															editDraft({
																...draft,
																preload_programmer_changes: event.target.checked,
															})
														}
													/>
													<SwitchField
														label="Preload physical playback actions"
														checked={draft.preload_physical_playback_actions}
														onChange={(event) =>
															editDraft({
																...draft,
																preload_physical_playback_actions:
																	event.target.checked,
															})
														}
													/>
													<SwitchField
														label="Preload virtual playback actions"
														checked={draft.preload_virtual_playback_actions}
														onChange={(event) =>
															editDraft({
																...draft,
																preload_virtual_playback_actions:
																	event.target.checked,
															})
														}
													/>
												</FormLayout>
											</article>
										)}
										{programmerSettingsError && (
											<p className="modal-error" role="alert">
												{programmerSettingsError}
											</p>
										)}
									</div>
								</>
							)}
							{section === 3 && draft && (
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
												editDraft({
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
												editDraft({
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
												editDraft({
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
									/>
								</>
							)}
							{section === 4 && (
								<>
									<h2>Timecode</h2>
									<div className="setup-list">
										{draft?.timecode_sources.map((source) => (
											<article key={source.source_prefix}>
												<b>{source.source_prefix}</b>
												<span>Priority {source.priority}</span>
												<small>
													{source.fallback
														? "Fallback allowed"
														: "Explicit source only"}
												</small>
											</article>
										))}
									</div>
								</>
							)}
							{section === 5 && (
								<>
									<h2>Network & Inputs</h2>
									<FormLayout
										className="configuration-form"
										labelPlacement="side"
									>
										<TextField
											label="Light server URL"
											value={serverUrl}
											onChange={(event) => setServerUrl(event.target.value)}
											description="Tauri can use this desk or a remote Light server."
										/>
										<FormField label="">
											<Button onClick={() => server.setServerUrl(serverUrl)}>
												Connect to server
											</Button>
										</FormField>
									</FormLayout>
									<div className="setup-cards">
										<section>
											<b>{configuredServerUrl()}</b>
											<small>Active REST and WebSocket server</small>
										</section>
										<section>
											<b>REST /api/v1</b>
											<small>Initial and coarse-grained state</small>
										</section>
										<section>
											<b>WebSocket connected</b>
											<small>Live events and control</small>
										</section>
									</div>
									<div className="setup-list network-input-list">
										<article><b>MIDI inputs</b><span>{draft?.midi_inputs.length ? draft.midi_inputs.join(", ") : "No MIDI inputs selected"}</span></article>
										<article><b>OSC</b><span>{draft?.osc_bind ?? "Disabled"}</span></article>
										<article><b>RTP-MIDI</b><span>{draft?.rtp_midi_bind ?? "Disabled"}</span></article>
									</div>
									<MatterBridgeSettings />
								</>
							)}
							<div hidden={section !== 6}>
								<ScreensSetup />
							</div>
							{server.error && <p className="modal-error">{server.error}</p>}
						</div>
					</WindowScrollArea>
				</main>
			</div>
			{fixtureLibraryOpen && (
				<div className="stacked-modal-layer fixture-library-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && setFixtureLibraryOpen(false)}>
					<section className="fixture-library-modal" role="dialog" aria-modal="true" aria-label="Fixture Library">
						<ModalTitleBar title="Fixture Library" search={<div id="setup-section-search" className="setup-section-search" />} actions={<div id="setup-section-actions" className="setup-section-actions" />} closeLabel="Close Fixture Library" onClose={() => setFixtureLibraryOpen(false)} />
						<div className="fixture-library-modal-body"><FixtureLibrarySetup /></div>
					</section>
				</div>
			)}
			{deskLockSettingsOpen && <DeskLockSettingsModal onClose={() => setDeskLockSettingsOpen(false)} />}
		</div>
	);
}
