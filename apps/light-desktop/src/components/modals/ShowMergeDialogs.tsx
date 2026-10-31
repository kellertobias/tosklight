import {
	Button,
	ErrorAlert,
	formatErrorDetails,
	ModalTitleBar,
	SelectField,
	TextInput,
} from "@tosklight/ui";
import { useEffect, useMemo, useState } from "react";
import type {
	ShowMergeConflict,
	ShowMergePreview,
	ShowMergeResolution,
} from "../../api/client/showMerge";
import type { FileRoot, ShowEntry } from "../../api/types";
import { useFiles } from "../../features/files/FilesContext";
import { useOptionalSelectiveImport } from "../../features/selectiveImport/SelectiveImportContext";
import { StackedModal } from "./StackedModal";
import "./showMerge.css";

/**
 * Take Offline Copy: a restore point of the open show plus a copy that remembers it, so the
 * programming done in the copy elsewhere can be merged back (TL-858).
 */
export function OfflineCopyDialog({
	activeShow,
	onClose,
}: {
	activeShow: ShowEntry;
	onClose: () => void;
}) {
	const merge = useOptionalSelectiveImport();
	const files = useFiles();
	const [name, setName] = useState(`${activeShow.name} – offline`);
	const [roots, setRoots] = useState<FileRoot[]>([]);
	const [rootId, setRootId] = useState("shows");
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState("");
	const [taken, setTaken] = useState<string | null>(null);
	useEffect(() => {
		let current = true;
		void files
			.fileRoots()
			.then((found) => current && setRoots(found))
			.catch((reason) => current && setError(formatErrorDetails(reason)));
		return () => {
			current = false;
		};
	}, [files]);
	const destinations = [
		{ value: "shows", label: "Internal" },
		...roots
			.filter((root) => root.removable && !root.network && root.writable)
			.map((root) => ({ value: root.id, label: `USB: ${root.label}` })),
	];
	const close = () => {
		if (!busy) onClose();
	};
	async function take() {
		if (!merge?.takeOfflineCopy || !name.trim() || busy) return;
		setBusy(true);
		setError("");
		try {
			const copy = await merge.takeOfflineCopy(name.trim(), rootId, "");
			setTaken(copy.name);
		} catch (reason) {
			setError(formatErrorDetails(reason));
		} finally {
			setBusy(false);
		}
	}
	return (
		<StackedModal onClose={close}>
			<div
				className="nested-modal offline-copy-modal"
				role="dialog"
				aria-modal="true"
				aria-label="Take offline copy"
			>
				<ModalTitleBar
					title="Take Offline Copy"
					closeDisabled={busy}
					onClose={close}
				/>
				{taken ? (
					<p role="status">
						Saved <b>{taken}</b>. Program it on another desk, then use Merge
						Offline Copy here to bring the work back into{" "}
						<b>{activeShow.name}</b>.
					</p>
				) : (
					<>
						<p>
							Saves a named revision of <b>{activeShow.name}</b> and a copy
							that remembers it. Changes made in the copy merge back into this
							show later; changes made on both sides are shown side by side.
						</p>
						<TextInput
							clearable
							className="show-name-input"
							autoFocus
							value={name}
							disabled={busy}
							onChange={(event) => setName(event.target.value)}
							onKeyboardCommit={() => void take()}
							aria-label="Offline copy name"
						/>
						<SelectField
							ariaLabel="Save offline copy to"
							label="Save to"
							value={rootId}
							options={destinations}
							disabled={busy}
							onChange={setRootId}
						/>
						{busy && <p role="status">Taking offline copy…</p>}
						{error && (
							<ErrorAlert as="p" role="alert">
								{error}
							</ErrorAlert>
						)}
					</>
				)}
				<footer>
					<Button disabled={busy} onClick={close}>
						{taken ? "Done" : "Cancel"}
					</Button>
					{!taken && (
						<Button
							variant="primary"
							disabled={busy || !name.trim() || !merge?.takeOfflineCopy}
							onClick={() => void take()}
						>
							Take Offline Copy
						</Button>
					)}
				</footer>
			</div>
		</StackedModal>
	);
}

const KIND_LABELS: Record<string, string> = {
	cue_list: "Cuelists",
	group: "Groups",
	preset: "Presets",
	patched_fixture: "Patch",
	playback_page: "Playback pages",
	macro: "Macros",
	timecode: "Timecodes",
	dynamic: "Dynamics",
};

const kindLabel = (kind: string) =>
	KIND_LABELS[kind] ??
	kind.replace(/_/g, " ").replace(/^./, (first) => first.toUpperCase());

/** A stored value as an operator reads it. */
function valueText(value: unknown): string {
	if (value === undefined || value === null) return "Not present";
	if (typeof value === "string") return value || "(empty)";
	if (typeof value === "number" || typeof value === "boolean")
		return String(value);
	const text = JSON.stringify(value);
	return text.length > 120 ? `${text.slice(0, 117)}…` : text;
}

/** Structural steps the operator does not need to read: a Cue's changes are simply "in the Cue". */
const HIDDEN_PATH = new Set(["cues", "changes", "group_changes"]);

function conflictTitle(conflict: ShowMergeConflict) {
	const kind = kindLabel(conflict.kind).replace(/s$/, "");
	const label = `${kind} ${conflict.object_number ?? conflict.object_id}`;
	const object =
		conflict.object_name && conflict.object_name !== label
			? `${label} “${conflict.object_name}”`
			: label;
	const path = conflict.path.filter(
		(segment, index) =>
			!HIDDEN_PATH.has(segment) && segment !== conflict.path[index - 1],
	);
	return path.length ? `${object} › ${path.join(" › ")}` : `${object} (whole object)`;
}

/** Merge Offline Copy: preview, decide each conflict, then merge as one undoable step. */
export function ShowMergeModal({
	activeShow,
	shows,
	onClose,
}: {
	activeShow: ShowEntry;
	shows: ShowEntry[];
	onClose: () => void;
}) {
	const merge = useOptionalSelectiveImport();
	const copies = shows.filter((show) => show.id !== activeShow.id);
	const [copyId, setCopyId] = useState("");
	const [preview, setPreview] = useState<ShowMergePreview | null>(null);
	const [choices, setChoices] = useState<Record<string, "mine" | "theirs">>(
		{},
	);
	const [status, setStatus] = useState("");
	const [error, setError] = useState("");
	const [busy, setBusy] = useState(false);
	const [merged, setMerged] = useState<string | null>(null);
	const resolutions = useMemo<ShowMergeResolution[]>(
		() =>
			Object.entries(choices).map(([conflict_id, choice]) => ({
				conflict_id,
				choice,
			})),
		[choices],
	);
	useEffect(() => {
		if (!copyId || !merge?.previewMerge) return;
		const abort = new AbortController();
		setPreview(null);
		setChoices({});
		setError("");
		setStatus("Comparing the copy with this show…");
		merge
			.previewMerge(activeShow.id, copyId, [], abort.signal)
			.then((result) => {
				setPreview(result);
				setStatus("");
			})
			.catch((reason) => {
				if (abort.signal.aborted) return;
				setError(formatErrorDetails(reason));
				setStatus("");
			});
		return () => abort.abort();
	}, [activeShow.id, copyId, merge]);
	const conflicts = preview?.conflicts ?? [];
	const undecided = conflicts.filter((conflict) => !choices[conflict.id]);
	const close = () => {
		if (!busy) onClose();
	};
	const chooseAll = (choice: "mine" | "theirs") =>
		setChoices(
			Object.fromEntries(conflicts.map((conflict) => [conflict.id, choice])),
		);
	async function apply() {
		if (!preview || !merge?.applyMerge || undecided.length || busy) return;
		setBusy(true);
		setError("");
		setStatus("Saving a restore point and merging…");
		try {
			const result = await merge.applyMerge(
				activeShow.id,
				copyId,
				preview.target_revision,
				resolutions,
			);
			setMerged(
				`Merged ${result.written_objects} object${result.written_objects === 1 ? "" : "s"} from ${preview.lineage.copy_name}. Named revision ${result.safety_revision} holds the show as it was before.`,
			);
			setStatus("");
		} catch (reason) {
			setError(formatErrorDetails(reason));
			setStatus("");
		} finally {
			setBusy(false);
		}
	}
	return (
		<StackedModal onClose={close}>
			<div
				className="nested-modal show-merge-modal"
				role="dialog"
				aria-modal="true"
				aria-label="Merge offline copy"
			>
				<ModalTitleBar
					title="Merge Offline Copy"
					closeDisabled={busy}
					onClose={close}
				/>
				{merged ? (
					<p role="status">{merged}</p>
				) : (
					<>
						<SelectField
							ariaLabel="Offline copy"
							label="Offline copy"
							value={copyId}
							disabled={busy}
							options={[
								{ value: "", label: "Choose a copy of this show…" },
								...copies.map((show) => ({ value: show.id, label: show.name })),
							]}
							onChange={setCopyId}
						/>
						{status && <p role="status">{status}</p>}
						{error && (
							<ErrorAlert as="p" role="alert">
								{error}
							</ErrorAlert>
						)}
						{preview && (
							<MergePreviewBody
								preview={preview}
								choices={choices}
								busy={busy}
								onChoose={(id, choice) =>
									setChoices((current) => ({ ...current, [id]: choice }))
								}
								onChooseAll={chooseAll}
							/>
						)}
					</>
				)}
				<footer>
					<Button disabled={busy} onClick={close}>
						{merged ? "Done" : "Cancel"}
					</Button>
					{!merged && (
						<Button
							variant="primary"
							disabled={
								busy || !preview || undecided.length > 0 || !merge?.applyMerge
							}
							onClick={() => void apply()}
						>
							{undecided.length
								? `Decide ${undecided.length} Conflict${undecided.length === 1 ? "" : "s"}`
								: `Merge ${preview?.write_count ?? 0} Change${preview?.write_count === 1 ? "" : "s"}`}
						</Button>
					)}
				</footer>
			</div>
		</StackedModal>
	);
}

function MergePreviewBody({
	preview,
	choices,
	busy,
	onChoose,
	onChooseAll,
}: {
	preview: ShowMergePreview;
	choices: Record<string, "mine" | "theirs">;
	busy: boolean;
	onChoose: (id: string, choice: "mine" | "theirs") => void;
	onChooseAll: (choice: "mine" | "theirs") => void;
}) {
	return (
		<div className="show-merge-preview">
			<p>
				Taken from <b>{preview.lineage.source_show_name}</b> on{" "}
				{new Date(preview.lineage.taken_at).toLocaleString()}.{" "}
				{preview.auto_merged > 0 &&
					`${preview.auto_merged} object${preview.auto_merged === 1 ? "" : "s"} changed on both sides merged automatically.`}
			</p>
			{preview.summary.length === 0 ? (
				<p role="note">The copy has no changes this show does not already have.</p>
			) : (
				<table className="show-merge-summary" aria-label="Merge summary">
					<thead>
						<tr>
							<th scope="col">Kind</th>
							<th scope="col">Added</th>
							<th scope="col">Changed</th>
							<th scope="col">Deleted</th>
							<th scope="col">Conflicts</th>
						</tr>
					</thead>
					<tbody>
						{preview.summary.map((kind) => (
							<tr key={kind.kind}>
								<th scope="row">{kindLabel(kind.kind)}</th>
								<td>{kind.added}</td>
								<td>{kind.changed}</td>
								<td>{kind.deleted}</td>
								<td>{kind.conflicts}</td>
							</tr>
						))}
					</tbody>
				</table>
			)}
			{preview.renumbers.length > 0 && (
				<section aria-label="Renumbered objects">
					<h3>Both sides used the same number</h3>
					<ul>
						{preview.renumbers.map((renumber) => (
							<li key={`${renumber.kind}-${renumber.from}`}>
								{kindLabel(renumber.kind).replace(/s$/, "")} {renumber.from}
								{renumber.name ? ` “${renumber.name}”` : ""} from the copy
								becomes {renumber.to}; this show keeps its own {renumber.from}.
							</li>
						))}
					</ul>
				</section>
			)}
			{preview.macro_warnings.length > 0 && (
				<section aria-label="Macros to check" className="show-merge-warnings">
					<h3>Check these Macros</h3>
					<ul>
						{preview.macro_warnings.map((warning) => (
							<li key={warning.macro_id}>
								{warning.name || "Macro"}: {warning.mentions.join(", ")}
							</li>
						))}
					</ul>
				</section>
			)}
			{preview.conflicts.length > 0 && (
				<section aria-label="Conflicts">
					<header className="show-merge-conflict-header">
						<h3>
							{preview.conflicts.length} conflict
							{preview.conflicts.length === 1 ? "" : "s"}
						</h3>
						<Button disabled={busy} onClick={() => onChooseAll("mine")}>
							Keep All Mine
						</Button>
						<Button disabled={busy} onClick={() => onChooseAll("theirs")}>
							Take All Theirs
						</Button>
					</header>
					<ul className="show-merge-conflicts">
						{preview.conflicts.map((conflict) => (
							<li key={conflict.id} aria-label={conflictTitle(conflict)}>
								<strong>{conflictTitle(conflict)}</strong>
								<div className="show-merge-sides">
									<Button
										aria-pressed={choices[conflict.id] === "mine"}
										disabled={busy}
										onClick={() => onChoose(conflict.id, "mine")}
									>
										<span>Keep Mine</span>
										<small>{valueText(conflict.mine)}</small>
									</Button>
									<Button
										aria-pressed={choices[conflict.id] === "theirs"}
										disabled={busy}
										onClick={() => onChoose(conflict.id, "theirs")}
									>
										<span>Take Theirs</span>
										<small>{valueText(conflict.theirs)}</small>
									</Button>
								</div>
							</li>
						))}
					</ul>
				</section>
			)}
		</div>
	);
}
