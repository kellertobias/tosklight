/**
 * The Show screen: the open show as a document, apart from the screens that plan its rig and from
 * the machine's Settings.
 *
 * The file actions — new, open, save, import and export — run across the top. Below them the show
 * is seen from both sides at once: its rig drawn on one half, and what describes it on paper — the
 * project, lighting designer, venue and contacts every printed page carries — on the other.
 */
import { WindowHeader } from "@tosklight/ui/window-kit";
import { useEffect, useState } from "react";
import { CadProjectPanel, type CadPaperwork } from "./cad/CadProjectPanel";
import { CadRigOverview } from "./cad/CadViewport";
import type { CadSceneSnapshot } from "./cad/types";
import { type DocumentSummary, documentSession } from "./document/session";
import { FileBar } from "./FileBar";
import { McpSettingsWorkspace } from "./McpSettingsWorkspace";
import { ShowNameCaption } from "./ShowNameCaption";
import { beginWindowDrag } from "./WindowChrome";
import "./showScreen.css";

function paperworkOf(document: DocumentSummary | null): CadPaperwork {
	return {
		project: document?.project ?? "",
		lightingDesigner: document?.lightingDesigner ?? "",
		venue: document?.venue ?? "",
		contactEmail: document?.contactEmail ?? "",
		contactPhone: document?.contactPhone ?? "",
		showDate: document?.showDate ?? "",
		showVersion: document?.showVersion ?? "",
	};
}

/** The show's paperwork, edited as a draft and saved into the show on request. */
function ShowPaperwork({
	document,
	onDocument,
	onError,
}: {
	document: DocumentSummary | null;
	onDocument: (document: DocumentSummary) => void;
	onError: (reason: unknown) => void;
}) {
	const [draft, setDraft] = useState(() => paperworkOf(document));
	const [saving, setSaving] = useState(false);
	// Another show, or the same show saved from elsewhere, replaces the draft with what it says.
	useEffect(() => {
		setDraft(paperworkOf(document));
	}, [document?.showId, document?.lastSavedAt]);

	if (!document)
		return (
			<section className="viz-show-paperwork" aria-label="Show information">
				<p>Open or create a show to describe it.</p>
			</section>
		);
	return (
		<section className="viz-show-paperwork" aria-label="Show information">
			<CadProjectPanel
				paperwork={draft}
				documentInfo={document}
				saving={saving}
				onChange={(field, value) =>
					setDraft((current) => ({ ...current, [field]: value }))
				}
				onSave={async () => {
					setSaving(true);
					try {
						onDocument(await documentSession.savePaperwork(draft));
					} catch (reason) {
						onError(reason);
					} finally {
						setSaving(false);
					}
				}}
			/>
		</section>
	);
}

export function ShowScreen({
	document,
	cadScene,
	onDocument,
	onReloadProfiles,
	onReloadDocument,
	onError,
}: {
	document: DocumentSummary | null;
	cadScene: CadSceneSnapshot | null;
	onDocument: (document: DocumentSummary) => void;
	onReloadProfiles: () => void;
	onReloadDocument: () => void;
	onError: (reason: unknown) => void;
}) {
	// MCP sets up how an assistant reaches this show, so it opens from the Show screen's own title.
	const [mcpOpen, setMcpOpen] = useState(false);
	return (
		<section className="viz-show-screen">
			<WindowHeader
				title="Show"
				dragHandleProps={{
					"data-tauri-drag-region": true,
					onPointerDown: beginWindowDrag,
				}}
				groups={[
					{
						id: "show-actions",
						actions: [
							{
								id: "mcp",
								label: "MCP",
								active: mcpOpen,
								onPress: () => setMcpOpen((open) => !open),
							},
						],
					},
				]}
			/>
			{mcpOpen ? (
				<McpSettingsWorkspace />
			) : (
				<FileBar
					document={document}
					onDocument={onDocument}
					onError={onError}
					onReloadProfiles={onReloadProfiles}
					onReloadDocument={onReloadDocument}
				>
					<div className="viz-show-halves">
						{document && cadScene?.showId === document.showId ? (
							<figure className="viz-show-rig-overview">
								<ShowNameCaption
									name={document.name}
									onRename={(name) =>
										documentSession
											.rename(name)
											.then(onReloadDocument)
											.catch(onError)
									}
								/>
								<CadRigOverview
									entities={cadScene.entities}
									drawings={cadScene.drawings}
									showName={document.name}
								/>
							</figure>
						) : (
							<div className="viz-show-rig-overview is-empty" aria-hidden="true" />
						)}
						<ShowPaperwork
							document={document}
							onDocument={onDocument}
							onError={onError}
						/>
					</div>
				</FileBar>
			)}
		</section>
	);
}
