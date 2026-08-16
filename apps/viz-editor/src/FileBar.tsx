import { open, save } from "@tauri-apps/plugin-dialog";
import { Button } from "@tosklight/ui";
import { type ReactNode, useEffect, useState } from "react";
import type { DeskPeer, DocumentSummary, MvrPreview } from "./document/session";
import { documentSession } from "./document/session";
import { LiveDmxInputsPanel } from "./LiveDmxInputsPanel";
import { MvrImport } from "./MvrImport";

const SHOW_FILTER = [{ name: "ToskLight show", extensions: ["show"] }];
const MVR_FILTER = [{ name: "MVR", extensions: ["mvr"] }];

export function FileBar({
	page = "show",
	document,
	onDocument,
	onError,
	onReloadProfiles,
	onReloadDocument,
	children,
}: {
	page?: "show" | "dmx";
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
			setStatus("");
			onError(reason);
		} finally {
			setBusy(false);
		}
	}

	return (
		<section
			className={`viz-editor-file-bar ${page === "show" ? "is-show" : ""}`}
		>
			{page === "show" ? (
				<div className="viz-show-actions">
					<section>
						<h2>New Show</h2>
						<Button
							disabled={busy}
							onClick={() => void run("Creating", actions.createShow)}
						>
							New Show
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
						<Button
							disabled={busy}
							title="Open a fresh copy of the demo rig that ships with ToskLight"
							onClick={() => void run("Opening", actions.openDemoShow)}
						>
							Open Demo Show
						</Button>
						{desks.map((desk) => (
							<Button
								key={desk.instance}
								disabled={busy}
								title={`${desk.name} at ${desk.address}`}
								onClick={() =>
									void run("Loading", () => actions.loadFrom(desk))
								}
							>
								Load from Desk · {desk.name}: {desk.show}
							</Button>
						))}
					</section>
					<section>
						<h2>Save As</h2>
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
			) : null}
			{page === "show" ? children : null}
			{page === "dmx" && document ? (
				<LiveDmxInputsPanel
					document={document}
					desks={desks}
					onError={onError}
				/>
			) : null}
			{page === "show" ? (
				<output className="viz-editor-status">{status}</output>
			) : null}
			{page === "show" && pendingMvr && (
				<MvrImport
					// Keyed by the archive, so choosing another one starts its own decisions
					// rather than inheriting the last archive's.
					key={pendingMvr.path}
					path={pendingMvr.path}
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
