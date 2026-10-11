import { Button, ErrorAlert, ModalRegistration, ModalTitleBar, TextField } from "@tosklight/ui";
import { useState } from "react";
import { createPortal } from "react-dom";
import { usePaneChromeTargets } from "../components/shell/PaneChromeContext";
import { usePlaybackSoloEditor } from "../features/playbackSolo/PlaybackSoloEditorContext";
import { usePlaybackPagesView } from "../features/playbackTopology/PlaybackTopologyView";
import { useScreens } from "../features/screens/ScreensContext";

/** Operating page and Solo drafts occupy the pane's existing title, never its bank. */
export function PlaybackPaneTitleControls({ surfaceId }: { surfaceId: string }) {
	const chrome = usePaneChromeTargets();
	const screens = useScreens();
	const editor = usePlaybackSoloEditor(surfaceId);
	const topology = usePlaybackPagesView();
	const [picker, setPicker] = useState(false);
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const configuration = screens.surfaces?.playback_surfaces[surfaceId];
	const page = screens.surfaces?.resolved_pages[surfaceId];
	const custom = configuration?.page_source.type !== "follow";
	const choose = async (number: number) => {
		if (!configuration || busy) return;
		setBusy(true); setError(null);
		try {
			if (configuration.page_source.type === "pinned")
				await screens.updatePlaybackSurface(surfaceId, { page_source: { type: "own", page: number } });
			else await screens.setSurfacePage(surfaceId, number);
			await screens.selectSurface(surfaceId);
			setPicker(false);
		} catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
		finally { setBusy(false); }
	};
	const controls = <div className="playback-pane-title-controls">
		{custom ? <Button disabled={!topology.ready || busy} onClick={() => setPicker(true)} aria-label={`Select Playback page ${page ?? "unknown"}`}>Page {page ?? "—"} ▾</Button>
			: <span>Page {page ?? "—"}</span>}
		{editor?.selecting ? <>
			<TextField label="Solo Region name" value={editor.name} onChange={event => editor.setName(event.target.value)} disabled={editor.saving} />
			<Button disabled={editor.saving} onClick={editor.cancel}>Cancel</Button>
			<Button disabled={editor.saving || editor.selected.length < 2} onClick={() => void editor.save()}>Save Solo Region</Button>
		</> : <Button disabled={!editor?.available || !editor.ready} onClick={() => {
			void screens.selectSurface(surfaceId).then(() => editor?.begin()).catch(reason => setError(String(reason)));
		}}>Create Solo Region</Button>}
		{(error || editor?.error) && <ErrorAlert role="alert">{error ?? editor?.error}</ErrorAlert>}
	</div>;
	return <>
		{chrome?.toolbar ? createPortal(controls, chrome.toolbar) : controls}
		{picker && <ModalRegistration onClose={() => setPicker(false)}><div className="stacked-modal-layer"><section className="nested-modal" role="dialog" aria-modal="true" aria-label="Playback pages">
			<ModalTitleBar title="Playback pages" onClose={() => setPicker(false)} />
			{topology.pages.map(item => <Button key={item.id} disabled={busy} onClick={() => void choose(item.body.number)}>{item.body.number} · {item.body.name}</Button>)}
			{error && <ErrorAlert role="alert">{error}</ErrorAlert>}
		</section></div></ModalRegistration>}
	</>;
}
