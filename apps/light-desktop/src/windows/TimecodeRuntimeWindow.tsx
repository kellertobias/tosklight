import { Button } from "@tosklight/ui";
import { PoolCard, PoolGrid, type PoolSlotViewModel } from "@tosklight/ui/pools";
import { WindowHeader, WindowScrollArea } from "@tosklight/ui/window-kit";
import { useCallback, useEffect, useMemo, useState } from "react";
import { createLightApi } from "../api/client/api";
import { TimecodesApiClient, type TimecodeObjectRecord } from "../api/client/timecodes";
import type { TimecodeDefinition, TimecodeTransportAction, TimecodeTransportSnapshot } from "../api/generated/light-wire";
import { useActiveShowId } from "../features/deskSnapshot/DeskSnapshotState";
import { useTimecodeActions } from "../features/timecode/TimecodeActionsContext";
import type { WindowProps } from "./windowTypes";
import "./TimecodeRuntimeWindow.css";

const FPS = 44;
const TIMECODE_POOL_SIZE = 100;

export function TimecodeRuntimeWindow({ active = true, compact = false }: WindowProps) {
	const showId = useActiveShowId();
	const fallback = useMemo(() => new TimecodesApiClient(createLightApi().runtime.capabilityTransport()), []);
	const api = useTimecodeActions() ?? fallback;
	const [objects, setObjects] = useState<TimecodeObjectRecord[]>([]);
	const [runtime, setRuntime] = useState<Map<string, TimecodeTransportSnapshot>>(new Map());
	const [editing, setEditing] = useState<TimecodeObjectRecord | NewTimecode | null>(null);
	const [error, setError] = useState<string | null>(null);

	const refresh = useCallback(async () => {
		if (!showId) return;
		const [collection, snapshots] = await Promise.all([api.objects(showId), api.runtime(showId)]);
		setObjects(collection.objects);
		setRuntime(new Map(snapshots.map((snapshot) => [snapshot.timecode_id, snapshot])));
	}, [api, showId]);

	useEffect(() => {
		if (!active || !showId) return;
		let cancelled = false;
		const update = () => void refresh().catch((reason) => !cancelled && setError(String(reason)));
		update();
		const timer = window.setInterval(update, 250);
		return () => { cancelled = true; window.clearInterval(timer); };
	}, [active, refresh, showId]);

	if (editing) {
		return <TimecodeEditor showId={showId} item={editing} api={api} snapshot={runtime.get(editing.definition.id)} onClose={() => setEditing(null)} onSaved={async () => { await refresh(); setEditing(null); }} />;
	}

	const byNumber = new Map(objects.map((object) => [object.definition.number, object]));
	const slots: PoolSlotViewModel<number>[] = objects.map((object) => ({
		id: object.definition.number,
		position: object.definition.number - 1,
		card: { number: object.definition.number, primary: object.definition.name },
	}));
	return <section className="timecode-window">
		{!compact && <WindowHeader title="Timecode" info={{ primary: `${objects.length} Timecodes` }} actions={[]} />}
		{error && <p className="timecode-error" role="alert">{error}</p>}
		<WindowScrollArea>
			<PoolGrid slots={slots} slotCount={Math.max(TIMECODE_POOL_SIZE, ...objects.map((object) => object.definition.number))}
				emptySlot={(index) => ({ id: index + 1, position: index, card: { number: index + 1, primary: "Empty", states: ["empty"] } })}
				renderSlot={(_, index) => {
					const number = index + 1;
					const item = byNumber.get(number);
					const snapshot = item ? runtime.get(item.definition.id) : undefined;
					return <PoolCard key={number} aria-label={item ? `Timecode ${number} ${item.definition.name}` : `Empty Timecode ${number}`}
						model={{ number, primary: item?.definition.name ?? "Empty", secondary: snapshot ? `${formatFrame(snapshot.frame)} · ${snapshot.state}` : item ? "Not running" : "Tap to create", color: "#9365d8", states: [...(!item ? ["empty" as const] : []), ...(snapshot?.state === "playing" ? ["active" as const] : [])] }}
						onClick={() => setEditing(item ?? newTimecode(number))} />;
				}} />
		</WindowScrollArea>
	</section>;
}

interface NewTimecode { revision: 0; definition: TimecodeDefinition; isNew: true }

function newTimecode(number: number): NewTimecode {
	return { revision: 0, isNew: true, definition: { id: crypto.randomUUID(), number, name: `Timecode ${number}`, duration_frame: FPS * 60, transport_offset_frame: 0, auto_start: false, markers: [], lanes: [] } };
}

function TimecodeEditor({ showId, item, api, snapshot, onClose, onSaved }: {
	showId: string | null; item: TimecodeObjectRecord | NewTimecode; api: TimecodesApiClient;
	snapshot?: TimecodeTransportSnapshot; onClose(): void; onSaved(): Promise<void>;
}) {
	const [draft, setDraft] = useState(item.definition);
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const isNew = "isNew" in item;
	const act = async (action: TimecodeTransportAction) => {
		if (!showId || isNew) return;
		setBusy(true); setError(null);
		try { await api.transportAction(showId, draft.id, action); } catch (reason) { setError(String(reason)); } finally { setBusy(false); }
	};
	const save = async () => {
		if (!showId) return;
		setBusy(true); setError(null);
		try {
			if (isNew) await api.create(showId, draft);
			else await api.update(showId, draft.id, item.revision, {
				number: draft.number, name: draft.name, duration_frame: draft.duration_frame,
				transport_offset_frame: draft.transport_offset_frame, auto_start: draft.auto_start,
				markers: draft.markers, lanes: draft.lanes,
			});
			await onSaved();
		} catch (reason) { setError(String(reason)); } finally { setBusy(false); }
	};
	const remove = async () => {
		if (!showId || isNew) return;
		setBusy(true);
		try { await api.delete(showId, draft.id, item.revision); await onSaved(); } catch (reason) { setError(String(reason)); setBusy(false); }
	};
	const duration = draft.duration_frame ?? 0;
	const frame = Math.min(snapshot?.frame ?? 0, duration);
	return <section className="timecode-window timecode-editor" aria-busy={busy}>
		<WindowHeader title={`Timecode ${draft.number}`} info={{ primary: snapshot ? `${formatFrame(snapshot.frame)} · ${snapshot.state}` : "Stopped" }} actions={[]} />
		{error && <p className="timecode-error" role="alert">{error}</p>}
		<div className="timecode-editor-toolbar">
			<Button onClick={onClose}>Back</Button><Button onClick={() => void save()} disabled={busy}>Save</Button>
			{!isNew && <Button onClick={() => void remove()} disabled={busy}>Delete</Button>}
		</div>
		<div className="timecode-transport" aria-label="Timecode transport">
			<Button onClick={() => void act({ type: "go" })} disabled={isNew || busy}>Go</Button>
			<Button onClick={() => void act({ type: "pause" })} disabled={isNew || busy}>Pause</Button>
			<Button onClick={() => void act({ type: "stop" })} disabled={isNew || busy}>Stop</Button>
			<Button onClick={() => void act({ type: "rewind" })} disabled={isNew || busy}>Rewind</Button>
			<strong>{formatFrame(snapshot?.frame ?? 0)}</strong>
		</div>
		<div className="timecode-fields">
			<label>Number<input type="number" min={1} value={draft.number} onChange={(event) => setDraft({ ...draft, number: Number(event.currentTarget.value) })} /></label>
			<label>Name<input value={draft.name} onChange={(event) => setDraft({ ...draft, name: event.currentTarget.value })} /></label>
			<label>Duration frames<input type="number" min={1} value={duration} onChange={(event) => setDraft({ ...draft, duration_frame: Number(event.currentTarget.value) })} /></label>
			<label>Transport offset<input type="number" min={0} value={draft.transport_offset_frame} onChange={(event) => setDraft({ ...draft, transport_offset_frame: Number(event.currentTarget.value) })} /></label>
			<label className="timecode-checkbox"><input type="checkbox" checked={draft.auto_start} onChange={(event) => setDraft({ ...draft, auto_start: event.currentTarget.checked })} />Auto-start</label>
		</div>
		<div className="timecode-timeline">
			<div className="timecode-playhead" style={{ left: `${duration ? (frame / duration) * 100 : 0}%` }} />
			{draft.markers.map((marker) => <button key={marker.id} type="button" className="timecode-marker" style={{ left: `${duration ? (marker.frame / duration) * 100 : 0}%` }} title={marker.name} onClick={() => void act({ type: "seek", frame: marker.frame })}>{marker.name}</button>)}
		</div>
		<input className="timecode-seek" aria-label="Seek Timecode" type="range" min={0} max={Math.max(1, duration)} value={frame} disabled={isNew || busy} onChange={(event) => void act({ type: "seek", frame: Number(event.currentTarget.value) })} />
		<div className="timecode-lanes"><h2>Lanes</h2>{draft.lanes.length ? draft.lanes.map((lane) => <article key={lane.id}><b>{lane.name}</b><span>{lane.content.kind.replaceAll("_", " ")}</span></article>) : <p>No lanes configured.</p>}</div>
	</section>;
}

export function formatFrame(frame: number): string {
	const seconds = Math.floor(frame / FPS);
	return `${String(Math.floor(seconds / 3600)).padStart(2, "0")}:${String(Math.floor(seconds / 60) % 60).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}:${String(frame % FPS).padStart(2, "0")}`;
}
