import type {
	FixtureProfile,
	PatchFixtureProjection,
	PatchProfileRevision,
} from "@tosklight/patch";
import type { TitleActionGroup } from "@tosklight/ui";
import { WindowHeader } from "@tosklight/ui/window-kit";
import { CadRigOverview } from "./cad/CadViewport";
import type { CadSceneSnapshot } from "./cad/types";
import { type DmxPage, DmxWorkspace } from "./DmxWorkspace";
import { type DocumentSummary, documentSession } from "./document/session";
import { FileBar } from "./FileBar";
import { FixtureLibraryWorkspace } from "./FixtureLibraryWorkspace";
import { McpSettingsWorkspace } from "./McpSettingsWorkspace";
import { RendererSettingsWorkspace } from "./RendererSettingsWorkspace";
import { ShowNameCaption } from "./ShowNameCaption";
import { beginWindowDrag } from "./WindowChrome";

export type SettingsPage = "show" | "visualizer" | "fixtures" | "dmx" | "mcp";

// The show, the machine's fixture library and the DMX wiring are pages of Settings rather than
// screens of the dock: none of them is a view of the rig being planned.
export const SETTINGS_PAGES: readonly { id: SettingsPage; label: string }[] = [
	{ id: "show", label: "Show" },
	{ id: "visualizer", label: "Visualizer" },
	{ id: "fixtures", label: "Fixtures" },
	{ id: "dmx", label: "DMX" },
	{ id: "mcp", label: "MCP" },
];

/** One Settings title for every page; Fixtures and DMX add their own groups left of the pages. */
export function ArchitectSettings({
	page,
	pages,
	document,
	cadScene,
	profiles,
	fixtures,
	profileRevisions,
	dmxPage,
	onDmxPage,
	onDocument,
	onReloadProfiles,
	onReloadDocument,
	onError,
}: {
	page: SettingsPage;
	pages: TitleActionGroup;
	document: DocumentSummary | null;
	cadScene: CadSceneSnapshot | null;
	profiles: readonly FixtureProfile[];
	fixtures: readonly PatchFixtureProjection[];
	profileRevisions: readonly PatchProfileRevision[];
	dmxPage: DmxPage;
	onDmxPage: (page: DmxPage) => void;
	onDocument: (document: DocumentSummary | null) => void;
	onReloadProfiles: () => void;
	onReloadDocument: () => void;
	onError: (reason: unknown) => void;
}) {
	if (page === "fixtures")
		return (
			<FixtureLibraryWorkspace
				profiles={profiles}
				onReloadProfiles={onReloadProfiles}
				settingsPages={pages}
				onError={onError}
			/>
		);
	if (page === "dmx" && document)
		return (
			<DmxWorkspace
				page={dmxPage}
				onPage={onDmxPage}
				document={document}
				fixtures={fixtures}
				profileRevisions={profileRevisions}
				settingsPages={pages}
				onError={onError}
			/>
		);
	return (
		<section className="viz-show-settings-workspace">
			<WindowHeader
				title="Settings"
				dragHandleProps={{
					"data-tauri-drag-region": true,
					onPointerDown: beginWindowDrag,
				}}
				groups={[pages]}
			/>
			{page === "show" ? (
				<FileBar
					document={document}
					onDocument={onDocument}
					onError={onError}
					onReloadProfiles={onReloadProfiles}
					onReloadDocument={onReloadDocument}
				>
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
					) : null}
				</FileBar>
			) : page === "mcp" ? (
				<McpSettingsWorkspace />
			) : page === "visualizer" ? (
				<RendererSettingsWorkspace onError={onError} />
			) : (
				// DMX is the wiring of an open show.
				<NoShowOpen />
			)}
		</section>
	);
}

export function NoShowOpen() {
	return (
		<section className="viz-editor-empty">
			<h1>No show open</h1>
			<p>
				Create a rig or open an existing show file. What you patch here is what
				the visualizer draws.
			</p>
		</section>
	);
}
