import { open, save } from "@tauri-apps/plugin-dialog";
import { Button, ModalFrame } from "@tosklight/ui";
import { type ReactNode, useEffect, useState } from "react";
import type { DeskPeer, DocumentSummary, MvrPreview, RecentDocument } from "./document/session";
import { documentSession } from "./document/session";
import { MvrImport } from "./MvrImport";
import { useDiscoveredDesks } from "./useDiscoveredDesks";

const SHOW_FILTER = [{ name: "ToskLight show", extensions: ["show"] }];
const MVR_FILTER = [{ name: "MVR", extensions: ["mvr"] }];

function displayShowPath(path: string): string {
    const normalized = path.replaceAll("\\", "/");
    const marker = "/de.tokenet.tosklight.visualizer/shows/";
    const internal = normalized.indexOf(marker);
    return internal >= 0 ? `<internal>/${normalized.slice(internal + marker.length)}` : path;
}

function displayOperatingSystem(value: string | null | undefined): string {
    return ({ macos: "macOS", windows: "Windows", linux: "Linux" } as Record<string, string>)[value ?? ""] ?? value ?? "Not announced";
}

export function FileBar({
	document,
	onDocument,
	onError,
	onReloadProfiles,
	onReloadDocument,
	children,
}: {
	document: DocumentSummary | null;
	onDocument: (summary: DocumentSummary) => void;
	onError: (reason: unknown) => void;
	onReloadProfiles: () => void;
	/** Something changed the document from outside the sheet, so the sheet has to read it again. */
	onReloadDocument: () => void;
	children?: ReactNode;
}) {
	const [status, setStatus] = useState("");
	const [busy, setBusy] = useState(false);
	const desks = useDiscoveredDesks();
	const [browser, setBrowser] = useState<"recent" | "desks" | null>(null);
	const [recent, setRecent] = useState<RecentDocument[]>([]);
	const [sourceDesk, setSourceDesk] = useState<string | null>(null);
	const activeDesks = desks.filter((desk) => Boolean(desk.show?.trim()));
	useEffect(() => { void documentSession.sourceDesk().then(setSourceDesk).catch(() => setSourceDesk(null)); }, [document]);
	async function browseRecent() {
		setBrowser("recent");
		setRecent(await documentSession.recentDocumentDetails());
		return null;
	}
	async function browseDesks() {
		setBrowser("desks");
		return null;
	}
	/** The archive the operator is deciding about, and what is in it. */
	const [pendingMvr, setPendingMvr] = useState<{
		path: string;
		preview: MvrPreview;
	} | null>(null);
	const actions = useFileActions(
		onDocument,
		onReloadProfiles,
		onReloadDocument,
		setPendingMvr,
	);

	function finishMvr(summary: string, imported: boolean) {
		setPendingMvr(null);
		setStatus(summary);
		if (imported) onReloadDocument();
	}

	/** Every file action reports what it did; none of them happen silently. */
	async function run(label: string, action: () => Promise<string | null>) {
		setBusy(true);
		setStatus(`${label}…`);
		try {
			const result = await action();
			setStatus(result ?? "");
		} catch (reason) {
			setStatus(`${label} failed: ${String(reason)}`);
			onError(reason);
		} finally {
			setBusy(false);
		}
	}

	return (
		<section className="viz-editor-file-bar is-show">
			<FileActionButtons busy={busy} document={document} sourceDesk={sourceDesk} actions={actions} browseRecent={browseRecent} browseDesks={browseDesks} run={run} />
			{children}
			{browser && <ModalFrame
				title={browser === "recent" ? "Recent shows" : "Control desk shows"}
				ariaLabel={browser === "recent" ? "Recent shows" : "Control desk shows"}
				closeLabel={browser === "recent" ? "Close Recent shows" : "Close Control desk shows"}
				dialogClassName="viz-show-browser"
				closeDisabled={busy}
				policy={{ escape: !busy, backdrop: !busy }}
				onClose={() => { if (!busy) setBrowser(null); }}
			>
				<div className="viz-show-browser-scroll" aria-busy={busy}>
                    <output aria-live="polite" className="viz-editor-status">{status}</output>
				{browser === "recent" ? <>
					{recent.length === 0 && <p>No recent shows are available.</p>}
					<table className="viz-recent-shows-table"><thead><tr><th>Show</th><th>Location</th><th>Last saved</th><th>Actions</th></tr></thead><tbody>
					{recent.map(({ path, lastSavedAt }) => <tr key={path}><td>{fileStem(path)}</td><td title={path}>{displayShowPath(path).startsWith("<internal>/") ? <><span className="viz-location-badge">Internal</span><span>{displayShowPath(path).slice(11)}</span></> : displayShowPath(path)}</td><td>{lastSavedAt ? new Date(lastSavedAt * 1000).toLocaleString() : "—"}</td><td><Button aria-label={`Open ${displayShowPath(path)}`} disabled={busy} onClick={() => void run("Opening", async () => {
						const summary = await documentSession.open(path); onDocument(summary); onReloadProfiles(); onReloadDocument(); setBrowser(null); return `Opened ${summary.name}`;
					})}>Open</Button></td></tr>)}
					</tbody></table>
				</> : <>
					{activeDesks.length === 0 ? <p className="viz-show-browser-empty">No announced control desks have an active show.</p> : <table className="viz-desk-shows-table"><thead><tr><th>Desk</th><th>Active Show</th><th>Actions</th></tr></thead><tbody>
					{activeDesks.map((desk) => <tr key={desk.instance}><td><strong className="viz-show-browser-primary">{desk.name}</strong><span className="viz-show-browser-secondary">{desk.address} · {displayOperatingSystem(desk.operatingSystem)}</span></td><td><span className="viz-show-browser-primary">{desk.show}</span><span className="viz-show-browser-secondary">Last loaded: {desk.showLastLoadedAt ? new Date(desk.showLastLoadedAt).toLocaleString() : "—"}</span></td><td><Button aria-label={`Load ${desk.show} from ${desk.name}`} disabled={busy} onClick={() => void run("Loading", async () => {
						const summary = await documentSession.loadFromDesk(desk.instance); onDocument(summary); onReloadProfiles(); onReloadDocument(); setSourceDesk(desk.name); setBrowser(null); return `Loaded ${summary.name} from ${desk.name}`;
					})}>Load</Button></td></tr>)}
					</tbody></table>}
				</>}
				</div>
			</ModalFrame>}

			{!browser && <output aria-live="polite" className="viz-editor-status">{status}</output>}
			{pendingMvr && (
				<MvrImport
					// Each prepared preview starts its own decisions.
					key={pendingMvr.preview.token}
					preview={pendingMvr.preview}
					onImported={(summary) => finishMvr(summary, true)}
					onCancel={() =>
						finishMvr("Import cancelled; nothing was changed", false)
					}
					onError={onError}
				/>
			)}
		</section>
	);
}

function FileActionButtons({
	busy,
	document,
	sourceDesk,
	actions,
	browseRecent,
	browseDesks,
	run,
}: {
	busy: boolean;
	document: DocumentSummary | null;
	sourceDesk: string | null;
	actions: ReturnType<typeof useFileActions>;
	browseRecent: () => Promise<null>;
	browseDesks: () => Promise<null>;
	run: (label: string, action: () => Promise<string | null>) => Promise<void>;
}) {
	return (
			<div className="viz-show-actions">
				<section>
					<h2>New Show</h2>
					<Button
						disabled={busy}
						onClick={() => void run("Creating", actions.createShow)}
					>
						New Show
					</Button>
					<Button
						disabled={busy}
						title="Open a fresh copy of the demo rig that ships with ToskLight"
						onClick={() => void run("Opening", actions.openDemoShow)}
					>
						Open Demo Show
					</Button>
				</section>
				<section>
					<h2>Open</h2>
					<Button
						disabled={busy}
						onClick={() => void run("Opening", actions.openShow)}
					>
						Load Show from Disk
					</Button>
					<Button disabled={busy} onClick={() => void run("Reading recent shows", browseRecent)}>Load Recent Shows</Button>
					<Button disabled={busy} onClick={() => void run("Reading desk shows", browseDesks)}>Load from ToskLight Control</Button>
				</section>
				<section>
					<h2>Save As</h2>
					{sourceDesk && <Button disabled={busy} onClick={() => void run(`Saving to ${sourceDesk}`, documentSession.saveToSourceDesk)}>Save to {sourceDesk}</Button>}
					<Button
						disabled={busy || !document}
						onClick={() => void run("Saving", actions.saveShowAs)}
					>
						Save As
					</Button>
				</section>
				<section>
					<h2>Import / Export</h2>
					<Button
						disabled={busy || !document}
						onClick={() => void run("Reading", actions.readMvr)}
					>
						Import MVR
					</Button>
					<Button
						disabled={busy || !document}
						onClick={() => void run("Exporting", actions.exportMvr)}
					>
						Export MVR
					</Button>
				</section>
			</div>
	);
}

function useFileActions(
	onDocument: (summary: DocumentSummary) => void,
	onReloadProfiles: () => void,
	onReloadDocument: () => void,
	setPendingMvr: (value: { path: string; preview: MvrPreview } | null) => void,
) {
	const accept = (summary: DocumentSummary) => {
		onDocument(summary);
		onReloadProfiles();
		onReloadDocument();
		return summary;
	};
	return {
		createShow: async () => {
			const path = await save({ filters: SHOW_FILTER });
			if (!path) return null;
			const name = fileStem(path);
			accept(await documentSession.create(path, name));
			return `Created ${name}`;
		},
		openShow: async () => {
			const path = await open({ filters: SHOW_FILTER, multiple: false });
			if (typeof path !== "string") return null;
			const summary = accept(await documentSession.open(path));
			return `Opened ${summary.name}`;
		},
		openDemoShow: async () => {
			const summary = accept(await documentSession.openDemoShow());
			return `Opened ${summary.name}, a copy of the packaged Demo Show, at ${summary.path}`;
		},
		saveShowAs: async () => {
			const path = await save({ filters: SHOW_FILTER });
			if (!path) return null;
			await documentSession.saveAs(path);
			const desk = await documentSession.sourceDesk();
			if (desk) return `Saved to ${path}; ${await documentSession.saveToSourceDesk()}`;
			return `Saved to ${path}`;
		},
		readMvr: async () => {
			// Nothing is written until the operator has reviewed the archive and made its decisions.
			const path = await open({ filters: MVR_FILTER, multiple: false });
			if (typeof path !== "string") return null;
			const preview = await documentSession.previewMvr(path);
			setPendingMvr({ path, preview });
			return `Read ${preview.fixtures.length} fixtures from the archive`;
		},
		exportMvr: async () => {
			const path = await save({ filters: MVR_FILTER });
			if (!path) return null;
			return `Exported ${await documentSession.exportMvr(path)} fixtures`;
		},
		loadFrom: async (desk: DeskPeer) => {
			const summary = accept(await documentSession.loadFromDesk(desk.instance));
			return `Loaded ${summary.name} from ${desk.name}`;
		},
	};
}

function fileStem(path: string) {
	const name = path.split(/[\\/]/u).pop() ?? "Show";
	return name.replace(/\.show$/iu, "") || "Show";
}
