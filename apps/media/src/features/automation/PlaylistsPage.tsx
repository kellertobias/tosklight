import { Button, NumberField, SelectField, SwitchField, TextField } from "@tosklight/ui/controls";
import { useEffect, useRef, useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { MediaErrorToast } from "../../app/ToastContext";
import { MediaScreenHeader } from "../../operator/MediaServerSurface";
import { useOptionalPlaybackTakeover, useOptionalPlaylistStepPreview } from "../../operator/PlaybackTakeoverContext";
import { acceptAutomationSnapshot, automationApi, changedFields, type AuxiliaryLayer, type Playlist, type PlaylistEntry, useAutomation } from "../../shared/api/automation";
import { ApiFailure } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type { CatalogView } from "../../shared/api/generated/media-wire";
import { useCatalog, useOutputs } from "../../shared/api/queries";
import { SettingsSaveState } from "../settings/SettingsSaveState";
import { AssetPicker, findAsset, SecondsField } from "./AutomationShared";
import { AuxiliaryLayerEditor } from "./AuxiliaryLayerEditor";
import { PlaylistEntryEditor } from "./PlaylistEntryEditor";
import { PlaylistPreviewPanel } from "./PlaylistPreviewPanel";
import { DEFAULT_PLAYLIST_TRANSFORM } from "./playlistTransform";
import "./automation.css";

export const emptyPlaylist = (): Omit<Playlist, "id"> => ({
	name: "New playlist", enabled: false, priority: 0, layers: [2, 3], masterDimmer: 1, masterVolume: 1,
	defaultFadeMillis: 1000, entries: [], auxiliaryLayers: [],
});

export function PlaylistsPage() {
	const automation = useAutomation();
	const catalog = useCatalog();
	const outputs = useOutputs();
	const playback = useOptionalPlaybackTakeover();
	const preview = useOptionalPlaylistStepPreview();
	const editing = useEditing(automation.reload);
	const [selectedId, setSelectedId] = useState<string | null>(null);
	const [creating, setCreating] = useState(false);
	return <section className="media-page pixel-automation-page">
		<MediaScreenHeader title="Playlists" detail="Ordered clips, transitions and fixed layers" groups={[{ id: "playlist-actions", actions: [{ id: "new-playlist", label: creating ? "Creating…" : "New playlist", ariaLabel: "New playlist", loading: creating, disabled: editing.busy || !automation.data || automation.loading, onPress: () => {
			setCreating(true);
			void editing.save(async () => {
				if (preview?.active || preview?.busy) await preview.stop();
				const before = new Set(automation.data?.configuration.playlists.map((playlist) => playlist.id));
				const result = await automationApi.createPlaylist(emptyPlaylist());
				// The creation route appends its object. Another browser may have added one since our read.
				const created = result.configuration.playlists.filter((playlist) => !before.has(playlist.id)).at(-1);
				if (!created) throw new ApiFailure("playlist-create-unconfirmed", "The server did not return the new playlist. Reload the list before trying again.", 500);
				acceptAutomationSnapshot(result);
				setSelectedId(created.id);
			}).finally(() => setCreating(false));
		} }] }]} />
		{editing.failure && <MediaErrorToast message={editing.failure.message} onDismiss={editing.dismiss} />}
		<PlaylistPreviewPanel outputs={outputs.data ?? []} playlists={automation.data?.configuration.playlists ?? []} playback={playback} preview={preview} />
		<ResourceState resource={automation} subject="playlists">{(data) => {
			const playlists = data.configuration.playlists;
			const selected = selectedId ? playlists.find((playlist) => playlist.id === selectedId) : playlists[0];
			return <div className="pixel-automation-workspace">
				<nav className="pixel-object-list" aria-label="Playlists">
					{playlists.map((playlist) => <Button key={playlist.id} contentAlign="left" active={selected?.id === playlist.id} onClick={() => {
						if (preview?.active && preview.active.playlistId !== playlist.id) void preview.stop().catch(() => undefined);
						setSelectedId(playlist.id);
					}}>
						<span>{playlist.name}<small>{playlist.entries.length} clips · Priority {playlist.priority}{playlist.enabled ? "" : " · Disabled"}</small></span>
					</Button>)}
					{playlists.length === 0 && <p>Create a playlist to arrange clips from your library.</p>}
				</nav>
				{selected ? <PlaylistEditor key={selected.id} playlist={selected} catalog={catalog.data} layerCount={Math.max(2, ...(outputs.data ?? []).map((output) => output.layerCount))}
					preview={preview} previewOutputAvailable={Boolean(playback?.selectedOutputId)}
					reload={automation.reload} onDelete={() => void editing.save(async () => {
						if (!globalThis.confirm(`Delete playlist “${selected.name}”? Events and fallback references must be removed first.`)) return;
						if (preview?.active?.playlistId === selected.id) await preview.stop();
						await automationApi.deletePlaylist(selected.id); setSelectedId(null);
					})} /> : <div className="pixel-empty-detail"><h2>{selectedId ? "Choose a playlist" : "Clips and fixed layers"}</h2><p>{selectedId ? "The selected playlist is no longer available. Choose another playlist or create one." : "Build a reusable playlist, then assign it to dated or weekly events in Schedule."}</p></div>}
			</div>;
		}}</ResourceState>
	</section>;
}

function PlaylistEditor({ playlist, catalog, layerCount, reload, onDelete, preview, previewOutputAvailable }: { playlist: Playlist; catalog: CatalogView | undefined; layerCount: number; reload: () => void; onDelete: () => void; preview: ReturnType<typeof useOptionalPlaylistStepPreview>; previewOutputAvailable: boolean }) {
	const [draft, setDraft] = useState(playlist);
	const editing = useEditing(reload);
	const previousDraft = useRef(draft);
	const activeEntry = preview?.active?.playlistId === playlist.id ? preview.active.entryId : null;
	useEffect(() => {
		if (previousDraft.current === draft) return;
		previousDraft.current = draft;
		if (!activeEntry || !preview) return;
		if (!draft.entries.some((entry) => entry.id === activeEntry)) {
			void preview.stop().catch(() => undefined);
		} else if (playlistValid(draft)) {
			void preview.show(draft, activeEntry).catch(() => undefined);
		}
	}, [draft, activeEntry, preview]);
	function update(change: Partial<Omit<Playlist, "id">>) {
		const next = { ...draft, ...change };
		setDraft(next);
		if (!playlistValid(next)) return;
		const fields = changedFields(next, playlist);
		editing.saveLive(() => automationApi.updatePlaylist(playlist.id, fields), `playlist:${playlist.id}`);
	}
	function updateEntries(entries: PlaylistEntry[]) { update({ entries }); }
	function updateAuxiliary(next: AuxiliaryLayer) {
		update({ auxiliaryLayers: [...draft.auxiliaryLayers.filter((entry) => entry.layer !== next.layer), next].sort((a, b) => a.layer - b.layer) });
	}
	const layers = Array.from({ length: layerCount }, (_, index) => ({ value: String(index + 1), label: `Layer ${index + 1}` }));
	const unused = Array.from({ length: layerCount }, (_, index) => index + 1).filter((layer) => !draft.layers.includes(layer));
	const [addAssetId, setAddAssetId] = useState<string | null>(null);
	return <article className="pixel-playlist-editor" aria-label={`Edit ${playlist.name}`}>
		<div className="pixel-editor-heading"><h2>{draft.name || "Playlist"}</h2><SettingsSaveState busy={editing.busy} failed={Boolean(editing.failure) || !playlistValid(draft)} /></div>
		{editing.failure && <MediaErrorToast message={editing.failure.message} onDismiss={editing.dismiss} />}
		{!playlistValid(draft) && <p className="media-settings-note" role="status">Changes are not saved yet. Enter a playlist name, assign content to each fixed layer, and add a clip before enabling.</p>}
		<div className="pixel-form-grid">
			<TextField label="Playlist name" value={draft.name} onChange={(event) => update({ name: event.target.value })} />
			<NumberField label="Priority" value={draft.priority} step={1} onChange={(event) => update({ priority: Math.trunc(Number(event.target.value)) })} />
			<SwitchField offLabel="Disabled" onLabel="Enabled" label="Playlist enabled" checked={draft.enabled} disabled={!draft.entries.length && !draft.enabled} onChange={(event) => update({ enabled: event.target.checked })} />
		</div>
		<section className="pixel-editor-section" aria-label="Playlist playback">
			<h3>Playback layers</h3>
			<div className="pixel-form-grid">
				<SelectField label="Playback layer A" ariaLabel="Playback layer A" value={String(draft.layers[0])} options={layers.map((layer) => ({ ...layer, disabled: Number(layer.value) === draft.layers[1] }))} onChange={(value) => {
					const next: [number, number] = [Number(value), draft.layers[1]];
					update({ layers: next, auxiliaryLayers: draft.auxiliaryLayers.filter((layer) => !next.includes(layer.layer)) });
				}} />
				<SelectField label="Playback layer B" ariaLabel="Playback layer B" value={String(draft.layers[1])} options={layers.map((layer) => ({ ...layer, disabled: Number(layer.value) === draft.layers[0] }))} onChange={(value) => {
					const next: [number, number] = [draft.layers[0], Number(value)];
					update({ layers: next, auxiliaryLayers: draft.auxiliaryLayers.filter((layer) => !next.includes(layer.layer)) });
				}} />
				<SecondsField label="Default fade (s)" millis={draft.defaultFadeMillis} onChange={(defaultFadeMillis) => update({ defaultFadeMillis })} />
				<NumberField label="Master dimmer (%)" value={Math.round(draft.masterDimmer * 100)} min={0} max={100} onChange={(event) => update({ masterDimmer: Number(event.target.value) / 100 })} />
				<NumberField label="Master volume (%)" value={Math.round(draft.masterVolume * 100)} min={0} max={100} onChange={(event) => update({ masterVolume: Number(event.target.value) / 100 })} />
			</div>
			<p className="media-settings-note">Clips alternate between the selected layers. A two-layer output needs layers 1 and 2. The selected pair must exist on every output using this playlist.</p>
		</section>
		<section className="pixel-editor-section" aria-label="Playlist clips"><div className="pixel-editor-heading"><h3>Clips in playback order</h3><span>{draft.entries.length} clips</span></div>
			{draft.entries.length === 0 && <p className="media-settings-note">Add a clip before enabling this playlist.</p>}
			<ol className="pixel-playlist-entries">{draft.entries.map((entry, index) => <PlaylistEntryEditor key={entry.id} entry={entry} index={index} catalog={catalog} count={draft.entries.length}
				onPreview={() => void preview?.show(draft, entry.id).catch(() => undefined)} previewing={activeEntry === entry.id} previewDisabled={!preview || !previewOutputAvailable || preview.busy || !playlistValid(draft)}
				onChange={(change) => updateEntries(draft.entries.map((candidate) => candidate.id === entry.id ? { ...candidate, ...change } : candidate))}
				onMove={(offset) => { const next = [...draft.entries]; [next[index], next[index + offset]] = [next[index + offset], next[index]]; updateEntries(next); }}
				onRemove={() => updateEntries(draft.entries.filter((candidate) => candidate.id !== entry.id))} />)}</ol>
			<div className="pixel-add-clip"><AssetPicker label="Add clip" catalog={catalog} assetId={addAssetId} onChange={setAddAssetId} />
				<Button disabled={!addAssetId} onClick={() => {
					if (!addAssetId) return;
					const item = findAsset(catalog, addAssetId)?.item;
					updateEntries([...draft.entries, { id: requestId(), assetId: addAssetId, plays: 1, holdMillis: item?.kind === "image" ? 10000 : null, fadeMillis: draft.defaultFadeMillis, transform: { ...DEFAULT_PLAYLIST_TRANSFORM } }]);
				}}>Add to playlist</Button></div>
		</section>
		<section className="pixel-editor-section" aria-label="Fixed layers"><h3>Other layers</h3><p className="media-settings-note">Fixed content stays in place as the playlist advances. Choose Off to clear a layer, or Leave unchanged to retain its current look.</p>
			{unused.map((layer) => <AuxiliaryLayerEditor key={layer} catalog={catalog} value={draft.auxiliaryLayers.find((candidate) => candidate.layer === layer) ?? { layer, mode: "unchanged", assetId: null, controls: {} }} onChange={updateAuxiliary} />)}
		</section>
		<div className="pixel-editor-footer"><Button variant="danger" onClick={onDelete}>Delete playlist</Button></div>
	</article>;
}

function playlistValid(playlist: Playlist): boolean {
	return Boolean(playlist.name.trim() && (!playlist.enabled || playlist.entries.length) && playlist.auxiliaryLayers.every((layer) => layer.mode !== "fixed" || layer.assetId));
}
