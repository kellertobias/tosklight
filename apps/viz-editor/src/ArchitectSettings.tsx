import type {
	FixtureProfile,
	PatchFixtureProjection,
	PatchProfileRevision,
} from "@tosklight/patch";
import type { TitleActionGroup } from "@tosklight/ui";
import { WindowHeader } from "@tosklight/ui/window-kit";
import { type DmxPage, DmxWorkspace } from "./DmxWorkspace";
import type { DocumentSummary } from "./document/session";
import { FixtureLibraryWorkspace } from "./FixtureLibraryWorkspace";
import { RendererSettingsWorkspace } from "./RendererSettingsWorkspace";
import { beginWindowDrag } from "./WindowChrome";

export type SettingsPage = "visualizer" | "fixtures" | "dmx";

// The Visualizer, the machine's fixture library and the DMX wiring are pages of Settings rather than
// screens of the dock: none of them is a view of the rig being planned. The show itself is its own
// Show screen, and MCP is reached from that screen's title.
export const SETTINGS_PAGES: readonly { id: SettingsPage; label: string }[] = [
	{ id: "visualizer", label: "Visualizer" },
	{ id: "fixtures", label: "Fixtures" },
	{ id: "dmx", label: "DMX" },
];

/** One Settings title for every page; Fixtures and DMX add their own groups left of the pages. */
export function ArchitectSettings({
	page,
	pages,
	document,
	profiles,
	fixtures,
	profileRevisions,
	dmxPage,
	onDmxPage,
	onReloadProfiles,
	onError,
}: {
	page: SettingsPage;
	pages: TitleActionGroup;
	document: DocumentSummary | null;
	profiles: readonly FixtureProfile[];
	fixtures: readonly PatchFixtureProjection[];
	profileRevisions: readonly PatchProfileRevision[];
	dmxPage: DmxPage;
	onDmxPage: (page: DmxPage) => void;
	onReloadProfiles: () => void;
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
			{page === "visualizer" ? (
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
