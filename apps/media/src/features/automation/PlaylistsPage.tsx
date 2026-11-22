import { Button, NumberField, SelectField, SwitchField, TextField } from "@tosklight/ui/controls";
import { useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { MediaErrorToast } from "../../app/ToastContext";
import { MediaScreenHeader } from "../../operator/MediaServerSurface";
import { automationApi, changedFields, type AuxiliaryLayer, type Playlist, type PlaylistEntry, useAutomation } from "../../shared/api/automation";
import { api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type { CatalogView } from "../../shared/api/generated/media-wire";
import { useCatalog, useOutputs } from "../../shared/api/queries";
import { SettingsSaveState } from "../settings/SettingsSaveState";
import { AssetPicker, assetName, findAsset, SecondsField } from "./AutomationShared";
import { AuxiliaryLayerEditor } from "./AuxiliaryLayerEditor";
import "./automation.css";

export const emptyPlaylist = (): Omit<Playlist, "id"> => ({
	name: "New playlist", enabled: false, priority: 0, layers: [2, 3], masterDimmer: 1, masterVolume: 1,
	defaultFadeMillis: 1000, entries: [], auxiliaryLayers: [],
});

export function PlaylistsPage() {
	const automation = useAutomation();
	const catalog = useCatalog();
	const outputs = useOutputs();
	const editing = useEditing(automation.reload);
	const [selectedId, setSelectedId] = useState<string | null>(null);
	return <section className="media-page pixel-automation-page">
		<MediaScreenHeader title="Playlists" detail="Ordered clips, transitions and fixed layers" actions={<Button variant="primary" disabled={editing.busy} onClick={() => void editing.save(async () => {
			const before = new Set(automation.data?.configuration.playlists.map((playlist) => playlist.id));
			const result = await automationApi.createPlaylist(emptyPlaylist());
			setSelectedId(result.configuration.playlists.find((playlist) => !before.has(playlist.id))?.id ?? null);
		})}>New playlist</Button>} />
		{editing.failure && <MediaErrorToast message={editing.failure.message} onDismiss={editing.dismiss} />}
		<ResourceState resource={automation} subject="playlists">{(data) => {
			const playlists = data.configuration.playlists;
			const selected = playlists.find((playlist) => playlist.id === selectedId) ?? playlists[0];
			return <div className="pixel-automation-workspace">
				<nav className="pixel-object-list" aria-label="Playlists">
					{playlists.map((playlist) => <Button key={playlist.id} contentAlign="left" active={selected?.id === playlist.id} onClick={() => setSelectedId(playlist.id)}>
						<span>{playlist.name}<small>{playlist.entries.length} clips · Priority {playlist.priority}{playlist.enabled ? "" : " · Disabled"}</small></span>
					</Button>)}
					{playlists.length === 0 && <p>Create a playlist to arrange clips from your library.</p>}
				</nav>
				{selected ? <PlaylistEditor key={selected.id} playlist={selected} catalog={catalog.data} layerCount={Math.max(2, ...(outputs.data ?? []).map((output) => output.layerCount))}
					reload={automation.reload} onDelete={() => void editing.save(async () => {
						if (!globalThis.confirm(`Delete playlist “${selected.name}”? Events and fallback references must be removed first.`)) return;
						await automationApi.deletePlaylist(selected.id); setSelectedId(null);
					})} /> : <div className="pixel-empty-detail"><h2>Clips and fixed layers</h2><p>Build a reusable playlist, then assign it to dated or weekly events in Schedule.</p></div>}
			</div>;
		}}</ResourceState>
	</section>;
}

function PlaylistEditor({ playlist, catalog, layerCount, reload, onDelete }: { playlist: Playlist; catalog: CatalogView | undefined; layerCount: number; reload: () => void; onDelete: () => void }) {
	const [draft, setDraft] = useState(playlist);
	const editing = useEditing(reload);
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
				onChange={(change) => updateEntries(draft.entries.map((candidate) => candidate.id === entry.id ? { ...candidate, ...change } : candidate))}
				onMove={(offset) => { const next = [...draft.entries]; [next[index], next[index + offset]] = [next[index + offset], next[index]]; updateEntries(next); }}
				onRemove={() => updateEntries(draft.entries.filter((candidate) => candidate.id !== entry.id))} />)}</ol>
			<div className="pixel-add-clip"><AssetPicker label="Add clip" catalog={catalog} assetId={addAssetId} onChange={setAddAssetId} />
				<Button disabled={!addAssetId} onClick={() => {
					if (!addAssetId) return;
					const item = findAsset(catalog, addAssetId)?.item;
					updateEntries([...draft.entries, { id: requestId(), assetId: addAssetId, plays: 1, holdMillis: item?.kind === "image" ? 10000 : null, fadeMillis: draft.defaultFadeMillis }]);
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

function PlaylistEntryEditor({ entry, index, catalog, count, onChange, onMove, onRemove }: { entry: PlaylistEntry; index: number; catalog: CatalogView | undefined; count: number;
	onChange: (change: Partial<PlaylistEntry>) => void; onMove: (offset: number) => void; onRemove: () => void }) {
	const found = findAsset(catalog, entry.assetId);
	return <li className="pixel-playlist-entry" aria-label={`Clip ${index + 1}`}>
		<div className="pixel-clip-identity"><span className="pixel-clip-number">{index + 1}</span>{found && <img src={api.thumbnailUrl(found.folder.folder, found.item.file)} alt="" loading="lazy" />}<strong>{assetName(catalog, entry.assetId)}</strong></div>
		<AssetPicker label={`Clip ${index + 1} media`} catalog={catalog} assetId={entry.assetId} onChange={(assetId, item) => { if (assetId) onChange({ assetId, holdMillis: item?.kind === "image" ? entry.holdMillis ?? 10000 : null }); }} />
		<div className="pixel-form-grid">
			<NumberField label={`Clip ${index + 1} plays`} value={entry.plays} min={1} step={1} onChange={(event) => { const plays = Number(event.target.value); if (Number.isInteger(plays) && plays > 0) onChange({ plays }); }} />
			{(found?.item.kind === "image" || entry.holdMillis !== null) && <SecondsField label={`Clip ${index + 1} image duration (s)`} millis={entry.holdMillis ?? 10000} min={0.1} onChange={(holdMillis) => onChange({ holdMillis })} />}
			<SecondsField label={`Clip ${index + 1} fade to next (s)`} millis={entry.fadeMillis} onChange={(fadeMillis) => onChange({ fadeMillis })} />
		</div>
		<div className="pixel-row-actions"><Button disabled={index === 0} aria-label={`Move clip ${index + 1} up`} onClick={() => onMove(-1)}>Move up</Button><Button disabled={index === count - 1} aria-label={`Move clip ${index + 1} down`} onClick={() => onMove(1)}>Move down</Button><Button aria-label={`Remove clip ${index + 1}`} onClick={onRemove}>Remove</Button></div>
	</li>;
}
