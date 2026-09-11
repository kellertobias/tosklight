// What this server is, what it listens on, and what it sends to.
//
// Output identity is editable as stored configuration. The running surface is deliberately left
// alone, and the page says so rather than pretending a monitor or resolution changed live.

import { Button } from "@tosklight/ui/controls";
import { useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { useFailureToast } from "../../app/ToastContext";
import {
	MediaSettingsLayout,
	type MediaSettingsSection,
} from "../../operator/MediaServerSurface";
import { api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type {
	Health,
	NetworkView,
	PlaybackView,
	TimeView,
} from "../../shared/api/generated/media-wire";
import {
	useHealth,
	useNetwork,
	useOutputs,
	usePlayback,
	useRuntime,
	useTime,
} from "../../shared/api/queries";
import { LogsPage } from "../logs/LogsPage";
import { NetworkEditor } from "./NetworkEditor";
import { OutputPixelMap } from "../pixelmap/OutputPixelMap";
import { OutputSettings } from "./OutputSettings";
import { SettingsSaveState } from "./SettingsSaveState";

const HEALTH_POLL_MS = 15_000;

export function SettingsPage() {
	const health = useHealth(HEALTH_POLL_MS);
	const runtime = useRuntime();
	const outputs = useOutputs(HEALTH_POLL_MS);
	const network = useNetwork();
	const time = useTime();
	const playback = usePlayback();
	const editing = useEditing(network.reload);
	const timeEditing = useEditing(time.reload);
	const playbackEditing = useEditing(playback.reload);
	const initialSection: MediaSettingsSection =
		window.location.pathname === "/logs"
			? "logs"
			: new URLSearchParams(window.location.search).get("section") === "dmx"
				? "dmx"
				: "network";
	const [section, setSection] = useState<MediaSettingsSection>(initialSection);
	useFailureToast(editing.failure);

	return (
		<MediaSettingsLayout active={section} onSelect={setSection}>
			{section === "libraries" && (
				<section className="media-page">
					<ResourceState resource={health} subject="server settings">
						{(data) => (
							<article className="media-settings-section" aria-label="Server">
								<h2>Server</h2>
								<ServerSettings health={data} />
							</article>
						)}
					</ResourceState>

					<ResourceState resource={runtime} subject="the data folder">
						{(data) => <PortableDataFolder runtime={data} />}
					</ResourceState>

					<ResourceState resource={time} subject="the server time">
						{(data) => (
							<ServerTime
								time={data}
								busy={timeEditing.busy}
								failed={timeEditing.failure !== undefined}
								onSave={(minutes) =>
									void timeEditing.save(() =>
										api.updateTime({
											requestId: requestId(),
											utcOffsetMinutes: minutes,
										}),
									)
								}
							/>
						)}
					</ResourceState>

					<ResourceState resource={playback} subject="the clip switch hold">
						{(data) => (
							<ClipSwitch
								playback={data}
								busy={playbackEditing.busy}
								failed={playbackEditing.failure !== undefined}
								onSave={(millis) =>
									void playbackEditing.save(() =>
										api.updatePlayback({
											requestId: requestId(),
											switchHoldMillis: millis,
										}),
									)
								}
							/>
						)}
					</ResourceState>

					<article className="media-settings-section" aria-label="Libraries">
						<div className="media-settings-section-heading">
							<h2>Libraries</h2>
							<SettingsSaveState busy={false} failed={false} />
						</div>
						<p>
							The library folder is the <code>library.root</code> value in this
							server's configuration file. Restart the server after changing it.
						</p>
					</article>
				</section>
			)}

			{section === "network" && (
				<section className="media-page">
					<ResourceState resource={network} subject="the network settings">
						{(data) => (
							<Network
								network={data}
								busy={editing.busy}
								failed={editing.failure !== undefined}
								onSave={(edit) =>
									void editing.save(() => api.updateNetwork(edit))
								}
							/>
						)}
					</ResourceState>
				</section>
			)}

			{section === "dmx" && (
				<section className="media-page">
					<ResourceState
						resource={outputs}
						subject="DMX input settings"
						isEmpty={(data) => data.length === 0}
						empty="No outputs are enabled."
					>
						{(data) => (
							<section
								className="media-settings-group"
								aria-labelledby="dmx-inputs-heading"
							>
								<h2 id="dmx-inputs-heading">DMX input</h2>
								{data.map((output) => (
									<OutputSettings
										key={output.id}
										outputId={output.id}
										outputName={output.name}
										mode="dmx"
										direct
									/>
								))}
							</section>
						)}
					</ResourceState>
				</section>
			)}

			{(section === "picture-output" || section === "sound-output") && (
				<ResourceState
					resource={outputs}
					subject={
						section === "picture-output" ? "picture settings" : "sound settings"
					}
					isEmpty={(data) => data.length === 0}
					empty="No outputs are enabled."
				>
					{(data) => (
						<section
							className="media-settings-group"
							aria-labelledby="outputs-heading"
						>
							<h2 id="outputs-heading">
								{section === "picture-output" ? "Picture" : "Sound"}
							</h2>
							{data.map((output) => (
								<OutputSettings
									key={output.id}
									outputId={output.id}
									outputName={output.name}
									mode={section === "picture-output" ? "picture" : "sound"}
									direct
								/>
							))}
						</section>
					)}
				</ResourceState>
			)}
			{section === "pixel-map" && (
				<ResourceState
					resource={outputs}
					subject="pixel mapping"
					isEmpty={(data) => data.length === 0}
					empty="No outputs are enabled."
				>
					{(data) => (
						<section
							className="media-settings-group"
							aria-labelledby="pixel-map-heading"
						>
							<h2 id="pixel-map-heading">Pixel mapping</h2>
							{data.map((output) => (
								<OutputPixelMap
									key={output.id}
									outputId={output.id}
									outputName={output.name}
								/>
							))}
						</section>
					)}
				</ResourceState>
			)}
			{section === "logs" && <LogsPage />}
		</MediaSettingsLayout>
	);
}

function PortableDataFolder({
	runtime,
}: {
	runtime: import("../../shared/api/generated/media-wire").RunningServerView;
}) {
	const [opening, setOpening] = useState(false);
	const [failure, setFailure] = useState<string>();
	return (
		<article className="media-settings-section" aria-label="Portable data folder">
			<h2>Media and configuration folder</h2>
			{runtime.dataDirectory ? (
				<>
					<p>
						Copy this whole folder to carry the Media Server configuration and
						all media to another computer.
					</p>
					<code className="media-data-directory">{runtime.dataDirectory}</code>
					<div className="media-settings-actions">
						<Button
							disabled={opening}
							onClick={async () => {
								setOpening(true);
								setFailure(undefined);
								try {
									await api.openDataDirectory();
								} catch (error) {
									setFailure(
										error instanceof Error ? error.message : "The folder could not be opened.",
									);
								} finally {
									setOpening(false);
								}
							}}
						>
							{opening ? "Opening on Media Server…" : "Show folder on Media Server"}
						</Button>
					</div>
					{failure && <p role="alert">{failure}</p>}
				</>
			) : (
				<p role="status">
					The media library is outside the configuration folder. Move it beside
					the configuration before copying this server.
				</p>
			)}
		</article>
	);
}

function Network({
	formId,
	network,
	busy,
	failed,
	onSave,
}: {
	formId?: string;
	network: NetworkView;
	busy: boolean;
	failed: boolean;
	onSave: (edit: Parameters<typeof api.updateNetwork>[0]) => void;
}) {
	return (
		<article className="media-settings-section" aria-label="Network">
			<div className="media-settings-section-heading">
				<h2>Network</h2>
				<SettingsSaveState busy={busy} failed={failed} restartBound />
			</div>
			<NetworkEditor
				formId={formId}
				network={network}
				busy={busy}
				onSave={onSave}
				showActions={false}
			/>
			{network.warnings.map((warning) => (
				<p key={warning} className="media-state is-error" role="alert">
					{warning}
				</p>
			))}
			{network.pendingRestart && network.takesEffectOnRestart && (
				<>
					<p className="media-state is-notice">
						A saved change to these addresses is used the next time this server
						starts. The sockets it is using now stay as they are.
					</p>
					<div className="media-settings-actions">
						<Button
							onClick={() =>
								onSave({
									requestId: requestId(),
									sameComputerPreset: network.activeSameComputerPreset,
									...network.activeStored,
								})
							}
						>
							Revert to current settings
						</Button>
					</div>
				</>
			)}
		</article>
	);
}

/// The offset every clock and clock-derived text follows unless it carries one of its own.
function ServerTime({
	time,
	busy,
	failed,
	onSave,
}: {
	time: TimeView;
	busy: boolean;
	failed: boolean;
	onSave: (utcOffsetMinutes: number) => void;
}) {
	const [draft, setDraft] = useState(String(time.utcOffsetMinutes));
	const minutes = Number(draft);
	const valid =
		draft.trim() !== "" &&
		Number.isInteger(minutes) &&
		Math.abs(minutes) <= time.maximumUtcOffsetMinutes;
	return (
		<article className="media-settings-section" aria-label="Server time">
			<div className="media-settings-section-heading">
				<h2>Server time</h2>
				<SettingsSaveState busy={busy} failed={failed} />
			</div>
			<p>
				Minutes east of UTC, for every clock and countdown this server draws. A
				clock with its own offset keeps it. Currently{" "}
				<strong>{offsetLabel(time.utcOffsetMinutes)}</strong>.
			</p>
			<label className="media-field">
				<span>UTC offset in minutes</span>
				<input
					type="number"
					step={15}
					min={-time.maximumUtcOffsetMinutes}
					max={time.maximumUtcOffsetMinutes}
					value={draft}
					onChange={(event) => setDraft(event.target.value)}
				/>
			</label>
			<Button
				disabled={busy || !valid || minutes === time.utcOffsetMinutes}
				onClick={() => onSave(minutes)}
			>
				Save server time
			</Button>
		</article>
	);
}

/// How long a layer keeps its previous clip while a newly selected one loads.
function ClipSwitch({
	playback,
	busy,
	failed,
	onSave,
}: {
	playback: PlaybackView;
	busy: boolean;
	failed: boolean;
	onSave: (switchHoldMillis: number) => void;
}) {
	const [draft, setDraft] = useState(String(playback.switchHoldMillis));
	const millis = Number(draft);
	const valid =
		draft.trim() !== "" &&
		Number.isInteger(millis) &&
		millis >= 0 &&
		millis <= playback.maximumSwitchHoldMillis;
	return (
		<article className="media-settings-section" aria-label="Clip switch">
			<div className="media-settings-section-heading">
				<h2>Clip switch</h2>
				<SettingsSaveState busy={busy} failed={failed} />
			</div>
			<p>
				While a newly selected clip loads, a layer keeps playing the clip it
				showed before instead of going black. After this time it lets go and
				shows nothing until the new clip is ready. 0 turns the hold off.
			</p>
			<label className="media-field">
				<span>Hold previous clip for (ms)</span>
				<input
					type="number"
					step={50}
					min={0}
					max={playback.maximumSwitchHoldMillis}
					value={draft}
					onChange={(event) => setDraft(event.target.value)}
				/>
			</label>
			<Button
				disabled={busy || !valid || millis === playback.switchHoldMillis}
				onClick={() => onSave(millis)}
			>
				Save clip switch
			</Button>
		</article>
	);
}

/// `+02:00`, which is how an operator reads a timezone.
export function offsetLabel(minutes: number): string {
	const sign = minutes < 0 ? "-" : "+";
	const absolute = Math.abs(minutes);
	return `${sign}${String(Math.floor(absolute / 60)).padStart(2, "0")}:${String(
		absolute % 60,
	).padStart(2, "0")}`;
}

function ServerSettings({ health }: { health: Health }) {
	return (
		<dl className="media-facts">
			<dt>Instance</dt>
			<dd>
				<code>{health.instance}</code>
			</dd>
			<dt>Status</dt>
			<dd>{health.status}</dd>
			<dt>Library items</dt>
			<dd>{health.catalogItems}</dd>
			<dt>Library revision</dt>
			<dd>{health.catalogRevision}</dd>
		</dl>
	);
}
