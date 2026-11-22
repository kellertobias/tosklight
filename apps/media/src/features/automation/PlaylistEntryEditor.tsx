import { Button, NumberField } from "@tosklight/ui/controls";
import type { PlaylistEntry } from "../../shared/api/automation";
import { api } from "../../shared/api/client";
import type { CatalogView, PlaylistTransformView } from "../../shared/api/generated/media-wire";
import { AssetPicker, assetName, findAsset, SecondsField } from "./AutomationShared";
import { DEFAULT_PLAYLIST_TRANSFORM } from "./playlistTransform";

const FRAME_FIELDS: Array<{ key: keyof PlaylistTransformView; label: string; min: number; max: number; step: number }> = [
	{ key: "scaleX", label: "scale X", min: 0, max: 10, step: 0.01 },
	{ key: "scaleY", label: "scale Y", min: 0, max: 10, step: 0.01 },
	{ key: "positionX", label: "position X", min: -2, max: 2, step: 0.01 },
	{ key: "positionY", label: "position Y", min: -2, max: 2, step: 0.01 },
	{ key: "rotation", label: "rotation (°)", min: -360, max: 360, step: 1 },
];

export function PlaylistEntryEditor({ entry, index, catalog, count, onChange, onMove, onRemove, onPreview, previewing = false, previewDisabled = true }: {
	entry: PlaylistEntry; index: number; catalog: CatalogView | undefined; count: number;
	onChange: (change: Partial<PlaylistEntry>) => void; onMove: (offset: number) => void; onRemove: () => void;
	onPreview?: () => void; previewing?: boolean; previewDisabled?: boolean;
}) {
	const found = findAsset(catalog, entry.assetId);
	const transform = entry.transform ?? DEFAULT_PLAYLIST_TRANSFORM;
	return <li className={`pixel-playlist-entry${previewing ? " is-previewing" : ""}`} aria-label={`Clip ${index + 1}`}>
		<div className="pixel-clip-identity"><span className="pixel-clip-number">{index + 1}</span>{found && <img src={api.thumbnailUrl(found.folder.folder, found.item.file)} alt="" loading="lazy" />}<strong>{assetName(catalog, entry.assetId)}</strong>{previewing && <span className="pixel-step-previewing">Previewing</span>}</div>
		<AssetPicker label={`Clip ${index + 1} media`} catalog={catalog} assetId={entry.assetId} onChange={(assetId, item) => { if (assetId) onChange({ assetId, holdMillis: item?.kind === "image" ? entry.holdMillis ?? 10000 : null }); }} />
		<div className="pixel-form-grid">
			<NumberField label={`Clip ${index + 1} plays`} value={entry.plays} min={1} step={1} onChange={(event) => { const plays = Number(event.target.value); if (Number.isInteger(plays) && plays > 0) onChange({ plays }); }} />
			{(found?.item.kind === "image" || entry.holdMillis !== null) && <SecondsField label={`Clip ${index + 1} image duration (s)`} millis={entry.holdMillis ?? 10000} min={0.1} onChange={(holdMillis) => onChange({ holdMillis })} />}
			<SecondsField label={`Clip ${index + 1} fade to next (s)`} millis={entry.fadeMillis} onChange={(fadeMillis) => onChange({ fadeMillis })} />
		</div>
		<fieldset className="pixel-playlist-frame"><legend>Frame</legend><div className="pixel-form-grid">
			{FRAME_FIELDS.map(({ key, label, min, max, step }) => <NumberField key={key} label={`Clip ${index + 1} ${label}`} value={transform[key]} min={min} max={max} step={step} onChange={(event) => {
				if (!event.target.value.trim()) return;
				const value = Number(event.target.value);
				if (Number.isFinite(value) && value >= min && value <= max) onChange({ transform: { ...transform, [key]: value } });
			}} />)}
			<div className="pixel-frame-reset"><Button size="compact" aria-label={`Reset clip ${index + 1} frame`} disabled={FRAME_FIELDS.every(({ key }) => transform[key] === DEFAULT_PLAYLIST_TRANSFORM[key])} onClick={() => onChange({ transform: { ...DEFAULT_PLAYLIST_TRANSFORM } })}>Reset frame</Button></div>
		</div></fieldset>
		<div className="pixel-row-actions"><Button disabled={previewDisabled || !found || found.item.enabled === false} active={previewing} aria-label={`Preview clip ${index + 1}`} onClick={onPreview}>Preview step</Button><Button disabled={index === 0} aria-label={`Move clip ${index + 1} up`} onClick={() => onMove(-1)}>Move up</Button><Button disabled={index === count - 1} aria-label={`Move clip ${index + 1} down`} onClick={() => onMove(1)}>Move down</Button><Button aria-label={`Remove clip ${index + 1}`} onClick={onRemove}>Remove</Button></div>
	</li>;
}
