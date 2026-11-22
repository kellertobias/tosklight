import { Button, NumberField, SelectField, SwitchField, TextField } from "@tosklight/ui/controls";
import { useEffect, useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { MediaErrorToast } from "../../app/ToastContext";
import { MediaScreenHeader } from "../../operator/MediaServerSurface";
import { useOptionalPlaybackTakeover } from "../../operator/PlaybackTakeoverContext";
import { automationApi, changedFields, type AutomationOutput, type AutomationView, type Playlist, type ScheduleEvent, type ScheduleTrigger, useAutomation, useAutomationStatus } from "../../shared/api/automation";
import { useEditing } from "../../shared/api/editing";
import { useOutputs } from "../../shared/api/queries";
import { sendTelemetryAction } from "../../shared/api/telemetry";
import { SettingsSaveState } from "../settings/SettingsSaveState";
import { AutomationNow, CalendarField } from "./AutomationShared";
import "./automation.css";

const DAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const NEW = "new";
export function emptyEvent(outputId: string, playlistId: string): Omit<ScheduleEvent, "id"> {
	return { name: "New event", enabled: true, outputId, playlistId, trigger: { kind: "fixed", start: "", end: null } };
}

export function SchedulePage() {
	const automation = useAutomation();
	const outputs = useOutputs();
	const playback = useOptionalPlaybackTakeover();
	const live = useAutomationStatus(automation.data);
	const editing = useEditing(automation.reload);
	const [outputId, setOutputId] = useState<string | null>(null);
	const [selectedId, setSelectedId] = useState<string | null>(null);
	const [actionFailure, setActionFailure] = useState<string | null>(null);
	const selectedOutput = outputs.data?.find((output) => output.id === (playback?.selectedOutputId || outputId)) ?? outputs.data?.[0];
	return <section className="media-page pixel-automation-page">
		<MediaScreenHeader title="Schedule" detail="Dated and weekly playback, in the computer’s local time" actions={<Button variant="primary" disabled={!selectedOutput || !automation.data?.configuration.playlists.length} onClick={() => setSelectedId(NEW)}>New event</Button>} />
		{(editing.failure || actionFailure) && <MediaErrorToast message={editing.failure?.message ?? actionFailure ?? "Scheduling action failed"} onDismiss={() => { editing.dismiss(); setActionFailure(null); }} />}
		<ResourceState resource={automation} subject="the schedule">{(data) => {
			if (!selectedOutput) return <p className="media-state">Create an output in Settings before scheduling playlists.</p>;
			const selectedOutputId = selectedOutput.id;
			const events = data.configuration.events.filter((event) => event.outputId === selectedOutputId);
			const selected = events.find((event) => event.id === selectedId) ?? (selectedId === NEW ? undefined : events[0]);
			const settings = data.configuration.outputs.find((output) => output.outputId === selectedOutputId) ?? { outputId: selectedOutputId, enabled: false, fallbackPlaylistId: null, dmxSilenceSeconds: 10 };
			return <>
				<div className="pixel-schedule-output-row"><SelectField label="Scheduled output" ariaLabel="Scheduled output" value={selectedOutputId} options={(outputs.data ?? []).map((output) => ({ value: output.id, label: output.name }))} onChange={(id) => { setOutputId(id); playback?.selectOutput(id); setSelectedId(null); }} />
					<p className="media-settings-note">Computer time zone: <strong>{data.timeZone}</strong>. Higher playlist priority wins overlaps.</p>
				</div>
				<div className="pixel-schedule-summary"><OutputAutomationEditor key={selectedOutputId} settings={settings} playlists={data.configuration.playlists} reload={automation.reload} />
					<AutomationNow snapshot={data} status={live.status.find((status) => status.outputId === selectedOutputId)} connected={live.connected} onFailure={setActionFailure} />
				</div>
				<div className="pixel-automation-workspace">
					<nav className="pixel-object-list" aria-label="Scheduled events">
						{events.map((event) => <Button key={event.id} contentAlign="left" active={selected?.id === event.id && selectedId !== NEW} onClick={() => setSelectedId(event.id)}><span>{event.name}<small>{eventSummary(event)}{event.enabled ? "" : " · Disabled"}</small></span></Button>)}
						{!events.length && <p>No events for this output. Add a dated or weekly event.</p>}
					</nav>
					{selectedId === NEW ? <EventEditor key={`new:${selectedOutputId}`} initial={emptyEvent(selectedOutputId, data.configuration.playlists[0]?.id ?? "")} playlists={data.configuration.playlists} outputId={selectedOutputId} reload={automation.reload} onCreated={(id) => setSelectedId(id)} onCancel={() => setSelectedId(null)} /> : selected ? <EventEditor key={selected.id} initial={selected} playlists={data.configuration.playlists} outputId={selectedOutputId} reload={automation.reload} onCreated={setSelectedId} onDelete={() => void editing.save(async () => {
						if (!globalThis.confirm(`Delete scheduled event “${selected.name}”?`)) return;
						await automationApi.deleteEvent(selected.id); setSelectedId(null);
					})} /> : <div className="pixel-empty-detail"><h2>Schedule a playlist</h2><p>A start time plays once. Add an end time to loop until that time.</p>{!data.configuration.playlists.length && <p>Create a playlist in Playlists first.</p>}</div>}
				</div>
				<UpcomingEvents data={data} outputId={selectedOutputId} />
			</>;
		}}</ResourceState>
	</section>;
}

function OutputAutomationEditor({ settings, playlists, reload }: { settings: AutomationOutput; playlists: Playlist[]; reload: () => void }) {
	const [draft, setDraft] = useState(settings);
	const editing = useEditing(reload);
	useEffect(() => setDraft((current) => ({ ...current, enabled: settings.enabled })), [settings.enabled]);
	function update(change: Partial<Pick<AutomationOutput, "fallbackPlaylistId" | "dmxSilenceSeconds">>) {
		setDraft((current) => ({ ...current, ...change }));
		editing.saveLive(() => automationApi.updateOutput(settings.outputId, change), `output:${Object.keys(change).join(",")}`);
	}
	return <section className="pixel-automation-settings" aria-label="Scheduling settings"><div className="pixel-editor-heading"><h2>Automatic playback</h2><SettingsSaveState busy={editing.busy} failed={Boolean(editing.failure)} /></div>
		{editing.failure && <MediaErrorToast message={editing.failure.message} onDismiss={editing.dismiss} />}
		<SwitchField offLabel="Stopped" onLabel="Running" label="Scheduling enabled" checked={draft.enabled} disabled={editing.busy} onChange={(event) => {
			const enabled = event.target.checked;
			void editing.save(async () => {
				try {
					await sendTelemetryAction({ type: "automation-set-enabled", outputId: settings.outputId, enabled });
					setDraft((current) => ({ ...current, enabled }));
				} finally { reload(); }
			});
		}} />
		<SelectField label="Fallback playlist" ariaLabel="Fallback playlist" value={draft.fallbackPlaylistId ?? ""} options={[{ value: "", label: "No fallback" }, ...playlists.map((playlist) => ({ value: playlist.id, label: `${playlist.name}${playlist.enabled ? "" : " · Disabled"}` }))]} onChange={(id) => update({ fallbackPlaylistId: id || null })} />
		<NumberField label="DMX silence timeout (s)" value={draft.dmxSilenceSeconds} min={3} step={1} onChange={(event) => { const seconds = Number(event.target.value); if (Number.isInteger(seconds) && seconds >= 3) update({ dmxSilenceSeconds: seconds }); }} />
		<p className="media-settings-note">DMX suspends automation immediately. After this silence timeout, the current scheduled playlist or fallback restarts from its first clip.</p>
	</section>;
}

function EventEditor({ initial, playlists, outputId, reload, onCreated, onDelete, onCancel }: { initial: ScheduleEvent | Omit<ScheduleEvent, "id">; playlists: Playlist[]; outputId: string; reload: () => void; onCreated: (id: string) => void; onDelete?: () => void; onCancel?: () => void }) {
	const [draft, setDraft] = useState(initial);
	const editing = useEditing(reload);
	const isNew = !("id" in initial);
	function update(change: Partial<Omit<ScheduleEvent, "id">>) {
		const next = { ...draft, ...change };
		setDraft(next);
		if ("id" in initial && eventValid(next)) {
			const fields = changedFields(next, initial);
			editing.saveLive(() => automationApi.updateEvent(initial.id, fields), `event:${initial.id}`);
		}
	}
	const trigger = draft.trigger;
	const looping = trigger.kind === "fixed" ? trigger.end !== null : trigger.endTime !== null;
	const playlist = playlists.find((candidate) => candidate.id === draft.playlistId);
	const valid = eventValid(draft);
	function changeTrigger(next: ScheduleTrigger) { update({ trigger: next }); }
	return <article className="pixel-event-editor" aria-label={isNew ? "New event" : `Edit ${initial.name}`}>
		<div className="pixel-editor-heading"><h2>{isNew ? "New event" : draft.name}</h2>{!isNew && <SettingsSaveState busy={editing.busy} failed={Boolean(editing.failure) || !valid} />}</div>
		{editing.failure && <MediaErrorToast message={editing.failure.message} onDismiss={editing.dismiss} />}
		<div className="pixel-form-grid">
			<TextField label="Event name" value={draft.name} onChange={(event) => update({ name: event.target.value })} />
			<SelectField label="Playlist" ariaLabel="Event playlist" value={draft.playlistId} options={playlists.map((playlist) => ({ value: playlist.id, label: `${playlist.name} · Priority ${playlist.priority}${playlist.enabled ? "" : " · Disabled"}` }))} onChange={(playlistId) => update({ playlistId })} />
			<SwitchField offLabel="Disabled" onLabel="Enabled" label="Event enabled" checked={draft.enabled} onChange={(event) => update({ enabled: event.target.checked })} />
			<SelectField label="Event type" ariaLabel="Event type" value={trigger.kind} options={[{ value: "fixed", label: "Fixed date" }, { value: "weekly", label: "Weekly" }]} onChange={(kind) => changeTrigger(kind === "fixed" ? { kind: "fixed", start: "", end: null } : { kind: "weekly", weekdays: [0], startTime: "09:00:00", endTime: null, endDayOffset: 0, firstDate: null, lastDate: null })} />
		</div>
		<p className="media-settings-note">Playlist priority: {playlist?.priority ?? "Unavailable"}. Priority is shared by every event using this playlist.</p>
		{trigger.kind === "fixed" ? <div className="pixel-form-grid"><CalendarField label="Start date and time" type="datetime-local" value={trigger.start} onChange={(start) => changeTrigger({ ...trigger, start: normalizeDateTime(start) })} />
			{looping && <CalendarField label="End date and time" type="datetime-local" value={trigger.end ?? ""} onChange={(end) => changeTrigger({ ...trigger, end: normalizeDateTime(end) })} />}</div> : <>
			<fieldset className="pixel-weekdays"><legend>Weekdays</legend>{DAYS.map((day, index) => <Button key={day} active={trigger.weekdays.includes(index)} aria-pressed={trigger.weekdays.includes(index)} onClick={() => changeTrigger({ ...trigger, weekdays: trigger.weekdays.includes(index) ? trigger.weekdays.filter((value) => value !== index) : [...trigger.weekdays, index].sort() })}>{day}</Button>)}</fieldset>
			<div className="pixel-form-grid"><CalendarField label="Start time" type="time" value={trigger.startTime} onChange={(startTime) => changeTrigger({ ...trigger, startTime: normalizeTime(startTime) })} />
				{looping && <><CalendarField label="End time" type="time" value={trigger.endTime ?? ""} onChange={(endTime) => changeTrigger({ ...trigger, endTime: normalizeTime(endTime) })} /><NumberField label="End day offset" description="0: same day; 1: next day" value={trigger.endDayOffset} min={0} max={7} step={1} onChange={(event) => changeTrigger({ ...trigger, endDayOffset: Number(event.target.value) })} /></>}
				<CalendarField label="First date (optional)" type="date" value={trigger.firstDate ?? ""} onChange={(firstDate) => changeTrigger({ ...trigger, firstDate: firstDate || null })} />
				<CalendarField label="Last date (optional)" type="date" value={trigger.lastDate ?? ""} onChange={(lastDate) => changeTrigger({ ...trigger, lastDate: lastDate || null })} />
			</div>
		</>}
		<SwitchField offLabel="Play once" onLabel="Loop" label="Loop until an end time" checked={looping} onChange={(event) => {
			if (trigger.kind === "fixed") changeTrigger({ ...trigger, end: event.target.checked ? trigger.start : null });
			else changeTrigger({ ...trigger, endTime: event.target.checked ? "17:00:00" : null });
		}} />
		<p className="media-settings-note">{looping ? "The playlist repeats until the exact end time, including across midnight when configured." : "The playlist runs once from start to finish, including the configured plays of each clip."} Missed and suppressed starts are skipped.</p>
		{!valid && <p className="media-settings-note" role="status">Choose a playlist and a valid start. Looping events need an end after their start.</p>}
		<div className="pixel-editor-footer">{isNew ? <><Button variant="primary" disabled={!valid || editing.busy} onClick={() => void editing.save(async () => {
			const before = new Set((await automationApi.read()).configuration.events.map((event) => event.id));
			const result = await automationApi.createEvent({ ...draft, outputId });
			const created = result.configuration.events.find((event) => !before.has(event.id) && event.outputId === outputId && event.playlistId === draft.playlistId);
			if (created) onCreated(created.id);
		})}>Create event</Button><Button onClick={onCancel}>Cancel</Button></> : <Button variant="danger" onClick={onDelete}>Delete event</Button>}</div>
	</article>;
}

export function normalizeTime(value: string) { return value.length === 5 ? `${value}:00` : value; }
function eventValid(event: Omit<ScheduleEvent, "id">): boolean {
	const trigger = event.trigger;
	return Boolean(event.name.trim() && event.playlistId && (trigger.kind === "fixed" ? trigger.start && (!trigger.end || trigger.end > trigger.start) : trigger.weekdays.length && trigger.startTime && (!trigger.endTime || trigger.endDayOffset > 0 || trigger.endTime > trigger.startTime)));
}
export function normalizeDateTime(value: string) { return value.length === 16 ? `${value}:00` : value; }
export function eventSummary(event: ScheduleEvent) {
	const trigger = event.trigger;
	if (trigger.kind === "fixed") return `${trigger.start.replace("T", " ")}${trigger.end ? ` → ${trigger.end.replace("T", " ")}` : " · Once"}`;
	return `${trigger.weekdays.map((day) => DAYS[day]?.slice(0, 3)).join(", ")} ${trigger.startTime}${trigger.endTime ? ` → ${trigger.endTime}${trigger.endDayOffset ? ` (+${trigger.endDayOffset}d)` : ""}` : " · Once"}`;
}

function UpcomingEvents({ data, outputId }: { data: AutomationView; outputId: string }) {
	const upcoming = data.upcoming.filter((event) => event.outputId === outputId);
	return <section className="pixel-editor-section pixel-upcoming" aria-label="Upcoming events"><h2>Upcoming events</h2>
		{upcoming.length ? <div className="pixel-upcoming-scroll"><table><thead><tr><th>Event</th><th>Start</th><th>End</th><th>Priority</th><th>Resolution</th></tr></thead><tbody>{upcoming.map((event) => <tr key={`${event.eventId}:${event.start}`}><td>{event.name}</td><td>{event.start}</td><td>{event.end ?? "Once"}</td><td>{event.priority}</td><td>{event.reason || (event.winning ? "Wins" : "Suppressed")}</td></tr>)}</tbody></table></div> : <p className="media-settings-note">No upcoming events for this output.</p>}
	</section>;
}
