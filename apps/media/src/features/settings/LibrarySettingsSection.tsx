import { Button } from "@tosklight/ui/controls";
import { useEffect, useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { useFailureToast } from "../../app/ToastContext";
import { api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type {
	Health,
	LibrarySettingsView,
	RunningServerView,
	TimeView,
} from "../../shared/api/generated/media-wire";
import {
	useHealth,
	useLibrarySettings,
	useRuntime,
	useTime,
} from "../../shared/api/queries";
import { SettingsSaveState } from "./SettingsSaveState";

export function LibrarySettingsSection() {
	const health = useHealth(15_000);
	const runtime = useRuntime();
	const librarySettings = useLibrarySettings();
	const time = useTime();
	const libraryEditing = useEditing(librarySettings.reload);
	const timeEditing = useEditing(time.reload);
	useFailureToast(libraryEditing.failure);
	useFailureToast(timeEditing.failure);

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
		</section>
	);
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
