import { Button } from "@tosklight/ui";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { documentSession } from "./document/session";
import type { DeskPeer, DocumentSummary, MvrPreview } from "./document/session";
import { MvrImport } from "./MvrImport";

const SHOW_FILTER = [{ name: "ToskLight show", extensions: ["show"] }];
const MVR_FILTER = [{ name: "MVR", extensions: ["mvr"] }];

export function FileBar({
	document,
	onDocument,
	onError,
	onReloadProfiles,
	onReloadDocument,
}: {
	document: DocumentSummary | null;
	onDocument: (summary: DocumentSummary) => void;
	onError: (reason: unknown) => void;
	onReloadProfiles: () => void;
	/** Something changed the document from outside the sheet, so the sheet has to read it again. */
	onReloadDocument: () => void;
}) {
	const [status, setStatus] = useState("");
	const [busy, setBusy] = useState(false);
	const desks = useDiscoveredDesks();
	/** The archive the operator is deciding about, and what is in it. */
	const [pendingMvr, setPendingMvr] = useState<{
		path: string;
		preview: MvrPreview;
	} | null>(null);

	async function createShow() {
		const path = await save({ filters: SHOW_FILTER });
		if (!path) return null;
		const name = fileStem(path);
		onDocument(await documentSession.create(path, name));
		onReloadProfiles();
		return `Created ${name}`;
	}

	async function openShow() {
		const path = await open({ filters: SHOW_FILTER, multiple: false });
		if (typeof path !== "string") return null;
		const summary = await documentSession.open(path);
		onDocument(summary);
		onReloadProfiles();
		return `Opened ${summary.name}`;
	}

	async function saveShowAs() {
		const path = await save({ filters: SHOW_FILTER });
		if (!path) return null;
		await documentSession.saveAs(path);
		return `Saved to ${path}`;
	}

	function finishMvr(summary: string, imported: boolean) {
		setPendingMvr(null);
		setStatus(summary);
		if (imported) onReloadDocument();
	}

	/** Nothing is written until the operator has seen what the archive holds and decided about
	 * the fixtures that need a decision. */
	async function readMvr() {
		const path = await open({ filters: MVR_FILTER, multiple: false });
		if (typeof path !== "string") return null;
		const preview = await documentSession.previewMvr(path);
		setPendingMvr({ path, preview });
		return `Read ${preview.fixtures.length} fixtures from the archive`;
	}

	async function exportMvr() {
		const path = await save({ filters: MVR_FILTER });
		if (!path) return null;
		const fixtures = await documentSession.exportMvr(path);
		return `Exported ${fixtures} fixtures`;
	}

	async function loadFrom(desk: DeskPeer) {
		const summary = await documentSession.loadFromDesk(desk.instance);
		onDocument(summary);
		onReloadProfiles();
		return `Loaded ${summary.name} from ${desk.name}`;
	}

	/** Every file action reports what it did; none of them happen silently. */
	async function run(label: string, action: () => Promise<string | null>) {
		setBusy(true);
		setStatus(`${label}…`);
		try {
			const result = await action();
			setStatus(result ?? "");
		} catch (reason) {
			setStatus("");
			onError(reason);
		} finally {
			setBusy(false);
		}
	}

	return (
		<header className="viz-editor-file-bar">
			<h1>{document ? document.name : "ToskLight Viz"}</h1>
			<p className="viz-editor-path">{document?.path ?? "No show open"}</p>
			<div className="viz-editor-file-actions">
				<Button disabled={busy} onClick={() => void run("Creating", createShow)}>
					New
				</Button>
				<Button disabled={busy} onClick={() => void run("Opening", openShow)}>
					Open
				</Button>
				{desks.map((desk) => (
					<Button
						key={desk.instance}
						disabled={busy}
						title={`${desk.name} at ${desk.address}`}
						onClick={() => void run("Loading", () => loadFrom(desk))}
					>
						Load from Desk · {desk.name}: {desk.show}
					</Button>
				))}
				<Button
					disabled={busy || !document}
					onClick={() => void run("Saving", saveShowAs)}
				>
					Save As
				</Button>
				<Button disabled={busy || !document} onClick={() => void run("Reading", readMvr)}>
					Import MVR
				</Button>
				<Button
					disabled={busy || !document}
					onClick={() => void run("Exporting", exportMvr)}
				>
					Export MVR
				</Button>
			</div>
			<output className="viz-editor-status">{status}</output>
			{pendingMvr && (
				<MvrImport
					// Keyed by the archive, so choosing another one starts its own decisions
					// rather than inheriting the last archive's.
					key={pendingMvr.path}
					path={pendingMvr.path}
					preview={pendingMvr.preview}
					onImported={(summary) => finishMvr(summary, true)}
					onCancel={() => finishMvr("Import cancelled; nothing was changed", false)}
					onError={onError}
				/>
			)}
		</header>
	);
}

function fileStem(path: string) {
	const name = path.split(/[\\/]/u).pop() ?? "Show";
	return name.replace(/\.show$/iu, "") || "Show";
}

/**
 * The desks on the network worth loading from.
 *
 * Nothing is offered when nothing is found: a button that can only fail is worse than no button.
 * A desk that starts after this window did should still appear, and one that goes should stop
 * being offered — the browse already keeps that list, and this is only how often the bar reads it.
 */
function useDiscoveredDesks() {
	const [desks, setDesks] = useState<DeskPeer[]>([]);
	useEffect(() => {
		let current = true;
		const look = () =>
			void documentSession
				.discoveredDesks()
				.then((found) => current && setDesks(found))
				.catch(() => current && setDesks([]));
		look();
		const timer = window.setInterval(look, 5000);
		return () => {
			current = false;
			window.clearInterval(timer);
		};
	}, []);
	return desks;
}
