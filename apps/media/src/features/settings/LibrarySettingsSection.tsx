import { Button } from "@tosklight/ui/controls";
import { NumberField } from "@tosklight/ui/forms";
import { useEffect, useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { useFailureToast } from "../../app/ToastContext";
import { api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type {
	DataFolderChangeView,
	Health,
	LibrarySettingsView,
	PlaybackView,
	RunningServerView,
	TimeView,
} from "../../shared/api/generated/media-wire";
import {
	useHealth,
	useLibrarySettings,
	usePlayback,
	useRuntime,
	useTime,
} from "../../shared/api/queries";
import { DataFolderPicker } from "./DataFolderPicker";
import { SettingsSaveState } from "./SettingsSaveState";

export function LibrarySettingsSection() {
	const health = useHealth(15_000);
	const runtime = useRuntime();
	const librarySettings = useLibrarySettings();
	const time = useTime();
	const playback = usePlayback();
	const libraryEditing = useEditing(librarySettings.reload);
	const timeEditing = useEditing(time.reload);
	const playbackEditing = useEditing(playback.reload);
	useFailureToast(libraryEditing.failure);
	useFailureToast(timeEditing.failure);
	useFailureToast(playbackEditing.failure);

	return (
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
							timeEditing.saveLive(() =>
								api.updateTime({
									requestId: requestId(),
									utcOffsetMinutes: minutes,
								}),
							)
						}
					/>
				)}
			</ResourceState>
			<ResourceState
				resource={librarySettings}
				subject="the media library settings"
			>
				{(data) => (
					<LibraryDirectory
						settings={data}
						busy={libraryEditing.busy}
						failed={libraryEditing.failure !== undefined}
						onSave={(directory) =>
							void libraryEditing.save(() =>
								api.updateLibrarySettings({
									requestId: requestId(),
									directory,
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
							playbackEditing.saveLive(() =>
								api.updatePlayback({
									requestId: requestId(),
									switchHoldMillis: millis,
								}),
							)
						}
					/>
				)}
			</ResourceState>
		</section>
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
	const valid = (text: string) =>
		wholeNumberIn(text, 0, playback.maximumSwitchHoldMillis);
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
			<NumberField
				label="Hold previous clip for (ms)"
				step={50}
				min={0}
				max={playback.maximumSwitchHoldMillis}
				unit="ms"
				value={draft}
				error={
					unfinished(draft) || valid(draft)
						? undefined
						: `Enter whole milliseconds from 0 to ${playback.maximumSwitchHoldMillis}.`
				}
				onChange={(event) => {
					const next = event.target.value;
					setDraft(next);
					if (valid(next) && Number(next) !== playback.switchHoldMillis)
						onSave(Number(next));
				}}
			/>
		</article>
	);
}

/// Whether text is a whole number inside an inclusive range.
function wholeNumberIn(text: string, minimum: number, maximum: number) {
	const value = Number(text);
	return (
		text.trim() !== "" &&
		Number.isInteger(value) &&
		value >= minimum &&
		value <= maximum
	);
}

/// Text an operator is still typing, which is not yet worth an error.
function unfinished(text: string) {
	return text.trim() === "" || text === "-";
}

function LibraryDirectory({
	settings,
	busy,
	failed,
	onSave,
}: {
	settings: LibrarySettingsView;
	busy: boolean;
	failed: boolean;
	onSave: (directory: string) => void;
}) {
	const [draft, setDraft] = useState(settings.storedDirectory);
	useEffect(() => setDraft(settings.storedDirectory), [settings.storedDirectory]);
	const directory = draft.trim();
	return (
		<article
			className="media-settings-section"
			aria-label="Media library directory"
		>
			<div className="media-settings-section-heading">
				<h2>Media library directory</h2>
				<SettingsSaveState busy={busy} failed={failed} restartBound />
			</div>
			<p>
				Choose a folder on the Media Server computer. Pixel uses it after the
				next restart; existing media is not moved automatically.
			</p>
			<label className="media-field">
				<span>Library directory</span>
				<input
					type="text"
					autoComplete="off"
					spellCheck={false}
					value={draft}
					onChange={(event) => setDraft(event.target.value)}
				/>
			</label>
			<div className="media-settings-actions">
				<Button
					disabled={
						busy ||
						directory.length === 0 ||
						directory === settings.storedDirectory
					}
					onClick={() => onSave(directory)}
				>
					Save library directory
				</Button>
				{settings.pendingRestart && (
					<Button
						disabled={busy}
						onClick={() => onSave(settings.activeDirectory)}
					>
						Revert to current directory
					</Button>
				)}
			</div>
			{settings.pendingRestart && settings.takesEffectOnRestart && (
				<p className="media-state is-notice">
					Pixel is still using <code>{settings.activeDirectory}</code>. Restart
					the server to use the saved directory.
				</p>
			)}
		</article>
	);
}

function PortableDataFolder({ runtime }: { runtime: RunningServerView }) {
	const [opening, setOpening] = useState(false);
	const [failure, setFailure] = useState<string>();
	const [picking, setPicking] = useState(false);
	const [changed, setChanged] = useState<DataFolderChangeView>();
	const changeFolder = (
		<Button disabled={changed !== undefined} onClick={() => setPicking(true)}>
			Change folder…
		</Button>
	);
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
										error instanceof Error
											? error.message
											: "The folder could not be opened.",
									);
								} finally {
									setOpening(false);
								}
							}}
						>
							{opening ? "Opening on Media Server…" : "Show folder on Media Server"}
						</Button>
						{changeFolder}
					</div>
					{failure && <p role="alert">{failure}</p>}
				</>
			) : (
				<>
					<p role="status">
						The media library is outside the configuration folder. Move it
						beside the configuration before copying this server, or choose a
						folder that holds both.
					</p>
					<div className="media-settings-actions">{changeFolder}</div>
				</>
			)}
			{changed && <RestartNotice change={changed} />}
			{picking && (
				<DataFolderPicker
					onClose={() => setPicking(false)}
					onChanged={(change) => {
						setPicking(false);
						setChanged(change);
					}}
				/>
			)}
		</article>
	);
}

/// Says what happens while the server restarts into the chosen folder, then reloads the page.
function RestartNotice({ change }: { change: DataFolderChangeView }) {
	const [late, setLate] = useState(false);
	useEffect(() => {
		let stopped = false;
		const started = Date.now();
		const poll = window.setInterval(async () => {
			if (Date.now() - started > 30_000) setLate(true);
			try {
				const running = await api.runtime();
				if (!stopped && running.dataDirectory === change.directory)
					window.location.reload();
			} catch {
				// The server is still restarting.
			}
		}, 1_000);
		return () => {
			stopped = true;
			window.clearInterval(poll);
		};
	}, [change.directory]);
	return (
		<p className="media-state is-notice" role="status">
			{change.loadedExisting
				? "Loading the configuration found in "
				: "Using a new configuration in "}
			<code>{change.directory}</code>. Pixel is restarting; this page reloads
			when it is back.
			{late &&
				" Pixel has not come back yet. Start it again on the Media Server computer if it stays away."}
		</p>
	);
}

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
	const limit = time.maximumUtcOffsetMinutes;
	const valid = (text: string) => wholeNumberIn(text, -limit, limit);
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
			<NumberField
				label="UTC offset in minutes"
				step={15}
				min={-limit}
				max={limit}
				unit="min"
				value={draft}
				error={
					unfinished(draft) || valid(draft)
						? undefined
						: `Enter whole minutes from -${limit} to ${limit}.`
				}
				onChange={(event) => {
					const next = event.target.value;
					setDraft(next);
					if (valid(next) && Number(next) !== time.utcOffsetMinutes)
						onSave(Number(next));
				}}
			/>
		</article>
	);
}

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
