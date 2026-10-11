import { Button, ErrorAlert, TextField } from "@tosklight/ui";
import { useState } from "react";
import type { PlaybackSoloEditor } from "../../features/playbackSolo/PlaybackSoloEditorContext";
import type {
	PlaybackSoloCapability,
	PlaybackSoloRegion,
} from "../../features/playbackSolo/contracts";

export function PlaybackSoloRegionManager({
	regions, capability, editor, onClose,
}: {
	regions: readonly PlaybackSoloRegion[] | null;
	capability: PlaybackSoloCapability;
	editor: PlaybackSoloEditor;
	onClose?: () => void;
}) {
	const [selectedId, setSelectedId] = useState<string | null>(null);
	const [name, setName] = useState("");
	const [pending, setPending] = useState(false);
	const selected = regions?.find((region) => region.id === selectedId);
	const busy = pending || capability.isSaving() || editor.saving;
	const start = (editing: boolean) => {
		if (editing && selected) editor.beginEdit(selected);
		else if (!editing) editor.begin();
		else return;
		onClose?.();
	};
	const rename = async () => {
		if (!selected || !regions || !name.trim() || busy) return;
		setPending(true);
		try {
			await capability.save(regions.map((region) =>
				region.id === selected.id ? { ...region, name: name.trim() } : region,
			));
		} finally {
			setPending(false);
		}
	};
	const remove = async () => {
		if (!selected || !regions || busy) return;
		setPending(true);
		try {
			if (await capability.save(regions.filter((region) => region.id !== selected.id))) {
				setSelectedId(null);
				setName("");
			}
		} finally {
			setPending(false);
		}
	};
	return (
		<section aria-label="Manage Solo Regions">
			<p>Select a Solo Region to rename, edit its Playback cells, or delete it.</p>
			{regions === null ? <p role="status">Loading Solo Regions…</p> : regions.map((region) => (
				<article key={region.id}>
					<Button aria-pressed={selectedId === region.id} disabled={busy}
						onClick={() => {
							setSelectedId(region.id);
							setName(region.name);
						}}>
						{region.name}
					</Button>
					<span>{region.members.map((address) => `${address.page}.${address.position}`).join(", ")}</span>
				</article>
			))}
			<Button disabled={!editor.ready || !editor.available || busy}
				onClick={() => start(false)}>Create Solo Region</Button>
			{selected && <>
				<TextField label="Region name" value={name}
					onChange={(event) => setName(event.target.value)} />
				<Button disabled={busy || !name.trim() || name.trim() === selected.name}
					onClick={() => void rename()}>Rename</Button>
				<Button disabled={busy || !editor.ready}
					onClick={() => start(true)}>Edit</Button>
				<Button disabled={busy} onClick={() => void remove()}>Delete</Button>
			</>}
			{capability.error && <ErrorAlert role="alert">{capability.error}</ErrorAlert>}
		</section>
	);
}
