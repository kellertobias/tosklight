import "./PlaybackSoloRegionManager.css";
import { Button, ErrorAlert, TextField, WindowScrollArea } from "@tosklight/ui";
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
		<section className="solo-region-manager" aria-label="Manage Solo Regions">
			<p className="solo-region-manager-help">Select a Solo Region to rename, edit its Playback cells, or delete it.</p>
			{regions === null ? <p role="status">Loading Solo Regions…</p>
				: regions.length === 0 ? <p className="solo-region-manager-empty" role="status">No Solo Regions.</p>
					: <WindowScrollArea className="solo-region-table-scroll">
						<table className="solo-region-table" aria-label="Solo Regions">
							<thead><tr><th scope="col">Name</th><th scope="col">Playback members</th></tr></thead>
							<tbody>{regions.map((region) => (
								<tr key={region.id} className={selectedId === region.id ? "is-selected" : undefined}>
									<th scope="row">
										<Button aria-pressed={selectedId === region.id} active={selectedId === region.id}
											contentAlign="left" disabled={busy} onClick={() => {
												setSelectedId(region.id);
												setName(region.name);
											}}>{region.name}</Button>
									</th>
									<td>{region.members.map((address) => `${address.page}.${address.position}`).join(", ")}</td>
								</tr>
							))}</tbody>
						</table>
					</WindowScrollArea>}
			{selected && <div className="solo-region-selected-controls">
				<TextField label="Region name" value={name} disabled={busy}
					onChange={(event) => setName(event.target.value)} />
				<div className="solo-region-actions">
					<Button disabled={busy || !name.trim() || name.trim() === selected.name}
						onClick={() => void rename()}>Rename</Button>
					<Button disabled={busy || !editor.ready}
						onClick={() => start(true)}>Edit</Button>
					<Button disabled={busy} onClick={() => void remove()}>Delete</Button>
				</div>
			</div>}
			<footer className="solo-region-manager-footer">
				<Button disabled={!editor.ready || !editor.available || busy}
					onClick={() => start(false)}>Create New Solo Region</Button>
			</footer>
			{capability.error && <ErrorAlert role="alert">{capability.error}</ErrorAlert>}
		</section>
	);
}
