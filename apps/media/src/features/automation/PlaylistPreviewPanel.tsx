import { Button, SelectField } from "@tosklight/ui/controls";
import { useEffect, useState } from "react";
import { MediaErrorToast } from "../../app/ToastContext";
import type { useOptionalPlaybackTakeover, useOptionalPlaylistStepPreview } from "../../operator/PlaybackTakeoverContext";
import type { Playlist } from "../../shared/api/automation";
import { api } from "../../shared/api/client";
import type { OutputView } from "../../shared/api/generated/media-wire";

export function PlaylistPreviewPanel({ outputs, playlists, playback, preview }: { outputs: OutputView[]; playlists: Playlist[]; playback: ReturnType<typeof useOptionalPlaybackTakeover>; preview: ReturnType<typeof useOptionalPlaylistStepPreview> }) {
	const active = preview?.active;
	const selected = outputs.find((output) => output.id === playback?.selectedOutputId) ?? outputs[0];
	const playlist = playlists.find((candidate) => candidate.id === active?.playlistId);
	const step = playlist?.entries.findIndex((entry) => entry.id === active?.entryId) ?? -1;
	const activeOutput = outputs.find((output) => output.id === active?.outputId);
	const layer = playlist && step >= 0 ? playlist.layers[step % 2] - 1 : -1;
	const source = activeOutput?.layers?.[layer]?.sourceStatus;
	return <section className="pixel-playlist-preview-panel" aria-label="Playlist step preview">
		<div className="pixel-playlist-preview-controls">
			<SelectField label="Preview output" ariaLabel="Preview output" value={selected?.id ?? ""} disabled={!playback || !outputs.length || preview?.busy} options={outputs.map((output) => ({ value: output.id, label: output.name }))} onChange={(id) => playback?.selectOutput(id)} />
			<div className="pixel-playlist-preview-status" role="status" aria-live="polite"><strong>{active ? `Previewing ${playlist?.name ?? "playlist"}${step >= 0 ? ` · Step ${step + 1}` : ""}` : preview?.busy ? "Preparing step preview…" : "Preview a playlist step"}</strong>
				<p>{active ? `Output: ${activeOutput?.name ?? "Selected output"}. Automatic scheduling is suspended during playback takeover.` : preview ? "Preview step takes over the selected output and holds that step until you stop or choose another one." : "Connect this editor to a Pixel output to preview a step."}</p>
				{active && preview?.busy && <p>Updating preview…</p>}
				{active && source?.state === "loading" && <p>Loading clip…</p>}
			</div>
			<Button disabled={!preview || (!active && !preview.busy && !preview.needsHandoff?.())} onClick={() => void preview?.stop().catch(() => undefined)}>Stop preview</Button>
		</div>
		{preview?.failure && <MediaErrorToast message={preview.failure.message} onDismiss={preview.dismissFailure} />}
		{active && source?.state === "failed" && <div className="pixel-playlist-preview-source-error" role="alert"><p>Clip failed to load: {source.failure ?? "The selected source could not be opened."} Choose another clip or retry this step.</p><Button size="compact" disabled={preview?.busy || !playlist} onClick={() => playlist && void preview?.show(playlist, active.entryId).catch(() => undefined)}>Retry step</Button></div>}
		{active && <LiveStepPicture key={active.outputId} outputId={active.outputId} outputName={activeOutput?.name ?? "Selected output"} />}
	</section>;
}

function LiveStepPicture({ outputId, outputName }: { outputId: string; outputName: string }) {
	const [revision, setRevision] = useState(0);
	const [failed, setFailed] = useState(false);
	useEffect(() => {
		const timer = window.setInterval(() => setRevision((current) => current + 1), 1000);
		return () => window.clearInterval(timer);
	}, []);
	return <figure className="pixel-playlist-preview-picture"><img src={api.outputPreviewUrl(outputId, revision)} alt={`Live playlist step preview on ${outputName}`} onLoad={() => setFailed(false)} onError={() => setFailed(true)} />
		<figcaption>{failed ? "The output is previewing, but its browser image is unavailable." : `Live output · ${outputName}`}{failed && <Button size="compact" onClick={() => setRevision((current) => current + 1)}>Retry image</Button>}</figcaption>
	</figure>;
}
