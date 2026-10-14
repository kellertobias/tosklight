import {
	Button,
	ErrorAlert,
	FormLayout,
	ModalFrame,
	OperationBusyOverlay,
	SelectField,
	TextField,
	WindowScrollArea,
	formatErrorDetails,
} from "@tosklight/ui";
import { useEffect, useRef, useState } from "react";
import type {
	FileEntry,
	FileRoot,
	FixtureDefinition,
} from "../../../api/types";
import { useFiles } from "../../../features/files/FilesContext";
import { useFixtureLibrary } from "../../../features/fixtureLibrary/FixtureLibraryContext";

function useExportFolder() {
	const files = useFiles();
	const [roots, setRoots] = useState<FileRoot[]>([]);
	const [rootId, setRootId] = useState("");
	const [path, setPath] = useState("");
	const [entries, setEntries] = useState<FileEntry[]>([]);
	const [reading, setReading] = useState(true);
	const [error, setError] = useState("");
	useEffect(() => {
		let active = true;
		void files
			.fileRoots()
			.then((next) => {
				if (active) {
					setRoots(next);
					setRootId(next.find((root) => root.writable)?.id ?? "");
				}
			})
			.catch((reason) => {
				if (active) setError(formatErrorDetails(reason));
			})
			.finally(() => {
				if (active) setReading(false);
			});
		return () => {
			active = false;
		};
	}, [files.fileRoots]);
	useEffect(() => {
		if (!rootId) return;
		let active = true;
		setReading(true);
		setError("");
		void files
			.fileEntries(rootId, path)
			.then((directory) => {
				if (active)
					setEntries(
						directory.entries.filter((entry) => entry.kind === "folder"),
					);
			})
			.catch((reason) => {
				if (active) setError(formatErrorDetails(reason));
			})
			.finally(() => {
				if (active) setReading(false);
			});
		return () => {
			active = false;
		};
	}, [files.fileEntries, rootId, path]);
	return {
		roots,
		root: roots.find((root) => root.id === rootId),
		rootId,
		path,
		entries,
		reading,
		error,
		selectRoot: (id: string) => {
			setPath("");
			setRootId(id);
		},
		setPath,
	};
}

export function FixturePackageExportDialog({
	mode,
	onClose,
}: {
	mode: FixtureDefinition;
	onClose: (status: string) => void;
}) {
	const library = useFixtureLibrary();
	const folder = useExportFolder();
	const [name, setName] = useState(
		`${mode.manufacturer}-${mode.name || mode.model}-${mode.revision}.toskfixture`
			.replace(/[^a-z0-9._-]+/gi, "-")
			.toLowerCase(),
	);
	const [saving, setSaving] = useState(false);
	const [error, setError] = useState("");
	const [status, setStatus] = useState("");
	const locked = useRef(false);
	const close = () => {
		if (!locked.current) onClose(status || "Export cancelled.");
	};
	async function save() {
		if (
			locked.current ||
			folder.reading ||
			!folder.root?.writable ||
			!name.trim()
		)
			return;
		locked.current = true;
		setSaving(true);
		setError("");
		setStatus("");
		try {
			if (!library?.exportFixturePackageFile)
				throw new Error(
					"Fixture package export requires a connected writable desk.",
				);
			const outcome = await library.exportFixturePackageFile(
				mode.profile_id ?? mode.id,
				mode.revision,
				{ root_id: folder.rootId, path: folder.path, name: name.trim() },
			);
			setStatus(`Saved ${folder.root.label} / ${outcome.path}`);
		} catch (reason) {
			setError(formatErrorDetails(reason));
		} finally {
			locked.current = false;
			setSaving(false);
		}
	}
	return (
		<ModalFrame
			title="Export fixture"
			ariaLabel="Export fixture"
			dialogClassName="nested-modal fixture-package-import-modal"
			closeLabel={status ? "Close Export fixture" : "Cancel Export fixture"}
			closeDisabled={saving}
			policy={{ escape: !saving, backdrop: !saving }}
			onClose={close}
			accept={{
				id: "save-fixture",
				label: "Save fixture package",
				variant: "primary",
				disabled:
					saving ||
					folder.reading ||
					!folder.root?.writable ||
					!name.trim() ||
					Boolean(status) ||
					Boolean(folder.error),
				onPress: () => void save(),
			}}
		>
			<p>
				{mode.manufacturer} {mode.name || mode.model} · Revision {mode.revision}
			</p>
			<FormLayout>
				<SelectField
					label="Destination"
					value={folder.rootId}
					disabled={saving || folder.reading}
					options={folder.roots
						.filter((root) => root.writable)
						.map((root) => ({ value: root.id, label: root.label }))}
					onChange={folder.selectRoot}
				/>
				<TextField
					label="File name"
					value={name}
					disabled={saving}
					onChange={(event) => setName(event.target.value)}
				/>
			</FormLayout>
			<p>
				Folder: {folder.root?.label ?? "No writable destination"} /{" "}
				{folder.path || "/"}
			</p>
			{folder.path && (
				<Button
					disabled={saving || folder.reading}
					onClick={() =>
						folder.setPath(folder.path.split("/").slice(0, -1).join("/"))
					}
				>
					Up one folder
				</Button>
			)}
			<WindowScrollArea className="show-save-folder-scroll">
				{folder.entries.map((entry) => (
					<Button
						key={entry.path}
						disabled={saving || folder.reading}
						onClick={() => folder.setPath(entry.path)}
					>
						{entry.name}
					</Button>
				))}
			</WindowScrollArea>
			{folder.reading && <p role="status">Reading destinations…</p>}
			{(error || folder.error) && (
				<ErrorAlert as="p" role="alert">
					{error || folder.error}
				</ErrorAlert>
			)}
			{status && <p role="status">{status}</p>}
			{saving && (
				<OperationBusyOverlay
					title="Saving fixture package…"
					message="Writing the exact selected revision. Wait for the export result; it cannot be cancelled while saving."
				/>
			)}
		</ModalFrame>
	);
}
