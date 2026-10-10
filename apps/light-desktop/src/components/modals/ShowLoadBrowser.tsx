import {
	Button,
	ErrorAlert,
	ModalFrame,
	OperationBusyOverlay,
} from "@tosklight/ui";
import { WindowScrollArea } from "@tosklight/ui/window-kit";
import { useEffect, useState } from "react";
import type {
	FileEntry,
	FileRoot,
	NetworkShow,
	NetworkShowPeer,
	ShowEntry,
	ShowRevision,
} from "../../api/types";
import { useFiles } from "../../features/files/FilesContext";
import type { QuickSetupModel } from "./QuickSetupModal";
import { useShowLoadOperation } from "./showLoadOperation";

type Source = "internal" | "usb" | "network";
interface Row {
	key: string;
	name: string;
	updated: string | null;
	local?: ShowEntry;
	file?: FileEntry;
	peer?: NetworkShowPeer;
	remote?: NetworkShow;
}
const dateLabel = (value: string | null) => {
	const date = value ? new Date(value) : null;
	return date && !Number.isNaN(date.getTime())
		? date.toLocaleString()
		: "Date unavailable";
};

function useShowLoadBrowserState(model: QuickSetupModel) {
	const files = useFiles();
	const lifecycle = model.authorities.lifecycle;
	const [source, setSource] = useState<Source>("internal");
	const [roots, setRoots] = useState<FileRoot[]>([]);
	const [rootId, setRootId] = useState("shows");
	const [path, setPath] = useState("");
	const [entries, setEntries] = useState<FileEntry[]>([]);
	const [peers, setPeers] = useState<NetworkShowPeer[]>([]);
	const operation = useShowLoadOperation();
	const busy = operation.phase !== null;
	const error = operation.error;
	const [selected, setSelected] = useState<Row | null>(null);
	const [revisions, setRevisions] = useState<ShowRevision[]>([]);
	useEffect(() => {
		void operation.run("catalogue", async (current) => {
			if (source === "network" && !lifecycle)
				throw new Error("Desk is disconnected. Reconnect, then Retry.");
			const found = await files.fileRoots();
			if (!current()) return;
			setRoots(found);
			if (source === "usb" && !rootId) {
				setEntries([]);
				setPeers([]);
				return;
			}
			if (source === "network") {
				if (!lifecycle)
					throw new Error("Desk is disconnected. Reconnect, then Retry.");
				const catalog = await lifecycle.networkShows();
				if (current()) {
					setPeers(catalog.peers);
					setEntries([]);
				}
			} else {
				const directory = await files.fileEntries(rootId, path);
				if (current()) {
					setEntries(directory.entries);
					setPeers([]);
				}
			}
		});
		return () => {
			operation.cancelRead();
		};
	}, [source, rootId, path, files, lifecycle]);
	const root = roots.find((root) => root.id === rootId);
	const sourceLabel =
		source === "internal"
			? "Internal"
			: source === "usb"
				? `USB: ${root?.label ?? "No drive connected"}`
				: "Network";
	const folderTitle =
		source === "network"
			? "Network shows"
			: `${root?.label ?? (source === "usb" ? "USB" : "Shows")} / ${path}`;
	const folders = entries
		.filter((entry) => entry.kind === "folder")
		.sort((a, b) => a.name.localeCompare(b.name));
	const rows = showLoadRows(source, peers, entries, lifecycle);
	function requestClose() {
		if (!operation.cancelRead()) return;
		if (selected) setSelected(null);
		else model.dialogs.setLoadOpen(false);
	}
	function switchSource(next: Source, driveId?: string) {
		if (operation.isBusy()) return;
		setSource(next);
		setPath("");
		setSelected(null);
		setRootId(
			next === "usb"
				? (driveId ?? roots.find((root) => root.removable)?.id ?? "")
				: "shows",
		);
	}
	async function revisionsFor(row: Row) {
		if (operation.isBusy()) return;
		setSelected(row);
		setRevisions([]);
		await operation.run("revisions", async (current) => {
			if (!lifecycle)
				throw new Error("Desk is disconnected. Reconnect, then Retry.");
			const found = row.local
				? await lifecycle.listShowRevisions(row.local.id)
				: (row.remote?.revisions ?? []);
			if (current()) setRevisions(found);
		});
	}
	async function perform(row: Row, named: number | null, partial: boolean) {
		await operation.run(partial ? "prepare" : "load", async (current) => {
			if (!lifecycle)
				throw new Error("Desk is disconnected. Reconnect, then Retry.");
			if (partial) {
				const prepared = await prepareShowSource(lifecycle, row, rootId, named);
				if (!current()) return;
				model.dialogs.setPartialSource(prepared);
				model.dialogs.setSelectiveImportOpen(true);
			} else {
				await openShowSource(lifecycle, row, rootId, named);
			}
			if (current()) model.dialogs.setLoadOpen(false);
		});
	}

	return {
		roots,
		source,
		path,
		root,
		sourceLabel,
		folderTitle,
		folders,
		rows,
		peers,
		busy,
		error,
		operation,
		selected,
		revisions,
		requestClose,
		switchSource,
		setPath,
		revisionsFor,
		perform,
	};
}

async function prepareShowSource(
	lifecycle: NonNullable<QuickSetupModel["authorities"]["lifecycle"]>,
	row: Row,
	rootId: string,
	named: number | null,
): Promise<ShowEntry> {
	if (row.local)
		return named === null
			? row.local
			: (await lifecycle.prepareShowRevision(row.local.id, named)) ||
					missingSource();
	if (row.peer)
		return (
			(await lifecycle.importRemoteShow(
				row.peer.instance,
				row.remote?.id ?? null,
				named,
				false,
			)) || missingSource()
		);
	if (!row.file) return missingSource();
	return (
		(await lifecycle.prepareShowFile(rootId, row.file.path, row.file.name)) ||
		missingSource()
	);
}
async function openShowSource(
	lifecycle: NonNullable<QuickSetupModel["authorities"]["lifecycle"]>,
	row: Row,
	rootId: string,
	named: number | null,
) {
	const loaded = row.local
		? named === null
			? await lifecycle.openShow(row.local.id)
			: await lifecycle.openShowRevision(row.local.id, named)
		: row.peer
			? await lifecycle.importRemoteShow(
					row.peer.instance,
					row.remote?.id ?? null,
					named,
					true,
				)
			: row.file
				? await lifecycle.openShowFile(rootId, row.file.path, row.file.name)
				: false;
	if (!loaded) missingSource();
}

function missingSource(): never {
	throw new Error(
		"The selected show could not be loaded. Check the desk error message, then Retry.",
	);
}
function showLoadRows(
	source: Source,
	peers: NetworkShowPeer[],
	entries: FileEntry[],
	lifecycle: QuickSetupModel["authorities"]["lifecycle"],
): Row[] {
	return source === "network"
		? peers.flatMap((peer) =>
				peer.shows.map((show) => ({
					key: `${peer.instance}:${show.id ?? "current"}`,
					name: show.name,
					updated: show.updated_at,
					peer,
					remote: show,
				})),
			)
		: entries
				.filter((entry) => entry.kind === "file" && /\.show$/i.test(entry.name))
				.map((file) => {
					const local =
						source === "internal"
							? lifecycle?.shows.find(
									(show) =>
										show.path.replaceAll("\\", "/").endsWith(`/${file.path}`) ||
										show.path === file.path,
								)
							: undefined;
					return {
						key: file.path,
						name: local?.name ?? file.name.replace(/\.show$/i, ""),
						updated:
							local?.updated_at ??
							(file.modified_millis === null
								? null
								: new Date(file.modified_millis).toISOString()),
						local,
						file,
					};
				})
				.sort((a, b) => a.name.localeCompare(b.name));
}

export function ShowLoadBrowser({ model }: { model: QuickSetupModel }) {
	const browser = useShowLoadBrowserState(model);
	const {
		source,
		path,
		root,
		folderTitle,
		peers,
		rows,
		folders,
		busy,
		error,
		operation,
		selected,
		requestClose,
		setPath,
	} = browser;

	return (
		<ModalFrame
			title={folderTitle}
			ariaLabel="Load show"
			dialogClassName="nested-modal load-show-modal"
			closeLabel="Close Load Show"
			closeDisabled={operation.committing}
			onClose={requestClose}
			groups={[{ id: "show-source", actions: showSourceActions(browser) }]}
		>
			<div className="show-browser-toolbar">
				{source !== "network" && path && (
					<Button
						disabled={busy}
						onClick={() => setPath(path.split("/").slice(0, -1).join("/"))}
					>
						Up one folder
					</Button>
				)}
			</div>
			{source === "usb" && !root && <p>No USB drive connected.</p>}
			{!selected && (
				<ShowLoadFeedback operation={operation} onCancel={requestClose} />
			)}

			{source === "network" &&
				peers.map((peer) => (
					<ErrorAlert
						as="p"
						role={peer.error ? "alert" : "status"}
						key={peer.instance}
						className={peer.error ? "modal-warning" : "show-peer"}
					>
						{peer.name} · {peer.address}
						{peer.error
							? ` · ${peer.error}`
							: peer.shows.length === 0
								? " · No shows available"
								: ""}
					</ErrorAlert>
				))}
			{!error && !busy && rows.length === 0 && folders.length === 0 && (
				<p>No shows available in this source.</p>
			)}
			{!error && !busy && <ShowLoadTable browser={browser} />}
			<ShowRevisionBrowser browser={browser} />
		</ModalFrame>
	);
}
type BrowserState = ReturnType<typeof useShowLoadBrowserState>;
function showSourceActions({
	sourceLabel,
	busy,
	roots,
	switchSource,
}: BrowserState) {
	return [
		{
			id: "source",
			kind: "dropdown" as const,
			ariaLabel: `Source: ${sourceLabel}`,
			label: (
				<>
					Source: {sourceLabel} <span aria-hidden="true">⌄</span>
				</>
			),
			disabled: busy,
			dropdown: {
				kind: "items" as const,
				ariaLabel: "Show source",
				items: [
					{
						kind: "action" as const,
						id: "internal",
						label: "Internal",
						onPress: () => switchSource("internal"),
					},
					...roots
						.filter((root) => root.removable && !root.network)
						.map((root) => ({
							kind: "action" as const,
							id: `usb-${root.id}`,
							label: `USB: ${root.label}`,
							onPress: () => switchSource("usb", root.id),
						})),
					...(!roots.some((root) => root.removable && !root.network)
						? [
								{
									kind: "action" as const,
									id: "usb-empty",
									label: "USB (No drives connected)",
									disabled: true,
									onPress: () => {},
								},
							]
						: []),
					{
						kind: "action" as const,
						id: "network",
						label: "Network",
						onPress: () => switchSource("network"),
					},
				],
			},
		},
	];
}

function ShowLoadTable({ browser }: { browser: BrowserState }) {
	const { folders, rows, busy, setPath, perform, revisionsFor } = browser;
	return (
		<WindowScrollArea className="show-browser-table-scroll">
			<table className="show-browser-table">
				<thead>
					<tr>
						<th>Show / folder</th>
						<th>Last saved</th>
						<th>Actions</th>
					</tr>
				</thead>
				<tbody>
					{folders.map((folder) => (
						<tr key={folder.path}>
							<td colSpan={3}>
								<Button disabled={busy} onClick={() => setPath(folder.path)}>
									📁 {folder.name}
								</Button>
							</td>
						</tr>
					))}
					{rows.map((row) => (
						<tr key={row.key}>
							<td>
								<strong>{row.name}</strong>
								{row.peer && <small>{row.peer.name}</small>}
							</td>
							<td>{dateLabel(row.updated)}</td>
							<td>
								<div className="show-row-actions">
									<Button
										disabled={busy}
										onClick={() => void perform(row, null, false)}
									>
										Load Latest
									</Button>
									<Button
										aria-label={`Revisions for ${row.name}`}
										disabled={busy}
										onClick={() => void revisionsFor(row)}
									>
										…
									</Button>
								</div>
							</td>
						</tr>
					))}
				</tbody>
			</table>
		</WindowScrollArea>
	);
}

function ShowRevisionBrowser({ browser }: { browser: BrowserState }) {
	const { selected, revisions, busy, operation, requestClose, perform } =
		browser;
	if (!selected) return null;
	return (
		<ModalFrame
			title={selected.name}
			ariaLabel={`Revisions for ${selected.name}`}
			dialogClassName="nested-modal show-revisions-modal"
			closeDisabled={operation.committing}
			closeLabel="Close revisions"
			onClose={requestClose}
		>
			<p>
				Named revisions load as independent copies. Partial Load previews
				dependencies and conflicts before changing the current show.
			</p>
			<WindowScrollArea className="show-browser-table-scroll">
				<table className="show-browser-table">
					<thead>
						<tr>
							<th>Revision</th>
							<th>Last saved</th>
							<th>Actions</th>
						</tr>
					</thead>
					<tbody>
						{[
							{
								revision: null,
								name: "Latest Autosave",
								created_at: selected.updated,
							},
							...revisions.map((item) => ({
								revision: item.revision,
								name: `Revision ${item.revision} · ${item.name}`,
								created_at: item.created_at,
							})),
						].map((item) => (
							<tr key={item.revision ?? "latest"}>
								<td>
									<strong>{item.name}</strong>
								</td>
								<td>{dateLabel(item.created_at)}</td>
								<td>
									<div className="show-row-actions">
										<Button
											disabled={busy}
											onClick={() =>
												void perform(selected, item.revision, false)
											}
										>
											Load
										</Button>
										<Button
											disabled={busy}
											onClick={() =>
												void perform(selected, item.revision, true)
											}
										>
											Partial Load
										</Button>
									</div>
								</td>
							</tr>
						))}
					</tbody>
				</table>
			</WindowScrollArea>
			<ShowLoadFeedback operation={operation} onCancel={requestClose} />
		</ModalFrame>
	);
}

function ShowLoadFeedback({
	operation,
	onCancel,
}: {
	operation: ReturnType<typeof useShowLoadOperation>;
	onCancel: () => void;
}) {
	const titles = {
		catalogue: "Reading show catalogue",
		revisions: "Reading named revisions",
		prepare: "Preparing selected show",
		load: "Loading selected show",
	};
	return (
		<>
			{operation.phase && (
				<OperationBusyOverlay
					key={operation.phase}
					title={titles[operation.phase]}
					label={titles[operation.phase]}
					message={
						operation.phase === "prepare"
							? "Preparing the show for Partial Load. Please wait."
							: operation.phase === "load"
								? "Opening the show. Please wait."
								: "Please wait, or Cancel to close this view."
					}
					onCancel={operation.committing ? undefined : onCancel}
				/>
			)}
			{operation.error && (
				<>
					<ErrorAlert
						as="p"
						className="show-browser-error"
						role="alert"
						copyText={operation.error}
					>
						{operation.error.split("\n")[0]}
					</ErrorAlert>
					<Button onClick={operation.retry}>Retry</Button>
				</>
			)}
		</>
	);
}
