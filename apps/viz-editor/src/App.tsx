import {
	FixturePatchSetup,
	type FixtureProfile,
	mergeFixtureDefinitions,
	type PatchFixtureProjection,
	type PatchHost,
	PatchHostProvider,
	type PatchLayer,
	type PatchProfileRevision,
	PatchViewProvider,
} from "@tosklight/patch";
import { Button } from "@tosklight/ui";
import { OperatorDestinationList } from "@tosklight/ui/application";
import { WindowHeader } from "@tosklight/ui/window-kit";
import { useCallback, useEffect, useMemo, useState } from "react";
import appIcon from "../src-tauri/icons/icon.svg";
import type { DocumentSummary } from "./document/session";
import { documentSession, sessionPatchLayers } from "./document/session";
import { TauriPatchTransport } from "./document/transport";
import { FileBar } from "./FileBar";
import { MediaWorkspace } from "./MediaWorkspace";
import { PreviewControls } from "./PreviewControls";
import { RendererSettingsWorkspace } from "./RendererSettingsWorkspace";
import { beginWindowDrag, WindowControls } from "./WindowChrome";
import { cadSession } from "./cad/session";

const DEFAULT_LAYER: PatchLayer = { id: "default", name: "Default", order: 0 };
type ShowPage =
	| "show"
	| "dmx"
	| "rendering"
	| "atmosphere"
	| "picture"
	| "features";

export function App() {
	const [document, setDocument] = useState<DocumentSummary | null>(null);
	const [profiles, setProfiles] = useState<readonly FixtureProfile[]>([]);
	const [layers, setLayers] = useState<readonly PatchLayer[]>([DEFAULT_LAYER]);
	const [error, setError] = useState<string | null>(null);
	const [workspace, setWorkspace] = useState<
		"show" | "patch" | "venue" | "effects" | "media" | "settings"
	>("show");
	const [showPage, setShowPage] = useState<ShowPage>("show");
	const [openingCad, setOpeningCad] = useState(false);
	const [openingViz, setOpeningViz] = useState(false);
	// Bumped when something outside the sheet changed the document — an MVR import — so the sheet
	// reads the new snapshot instead of showing the rig as it was before.
	const [reload, setReload] = useState(0);
	// What the patch sheet has selected, and what the preview controls therefore drive.
	const [selected, setSelected] = useState<readonly string[]>([]);
	const [selectionRevision, setSelectionRevision] = useState(0);
	// The rig itself, for the preview controls: the sheet owns the table, this owns the values.
	const [fixtures, setFixtures] = useState<readonly PatchFixtureProjection[]>(
		[],
	);
	// The profile revisions the show embedded, which are what Full DMX reads its slots from.
	const [profileRevisions, setProfileRevisions] = useState<
		readonly PatchProfileRevision[]
	>([]);
	const transport = useMemo(() => new TauriPatchTransport(), []);

	useEffect(() => {
		documentSession
			.current()
			.then((summary) => {
				setDocument(summary);
				if (summary) {
					loadLayers();
					loadFixtures();
				}
			})
			.catch(report);
		documentSession.fixtureProfiles().then(setProfiles).catch(report);
		// Say the interface is on screen. `--verify` waits for this and exits with the verdict:
		// a window that opens white reports nothing, which is exactly the failure to catch.
		documentSession.surfaceReady().catch(() => undefined);
	}, []);

	useEffect(() => {
		let selectionUnlisten: (() => void) | undefined;
		let sceneUnlisten: (() => void) | undefined;
		cadSession
			.snapshot()
			.then((snapshot) => {
				setSelected(snapshot.selectedIds);
				setSelectionRevision(snapshot.selectionRevision);
			})
			.catch(() => undefined);
		cadSession
			.onSelectionDelta((delta) => {
				setSelected(delta.selectedIds);
				setSelectionRevision(delta.revision);
				if (delta.selectedIds.length) setWorkspace("patch");
			})
			.then((unlisten) => {
				selectionUnlisten = unlisten;
			})
			.catch(() => undefined);
		cadSession
			.onSceneDelta(() => loadFixtures())
			.then((unlisten) => {
				sceneUnlisten = unlisten;
			})
			.catch(() => undefined);
		return () => {
			selectionUnlisten?.();
			sceneUnlisten?.();
		};
	}, []);

	async function replaceSelection(ids: readonly string[]) {
		setSelected(ids);
		try {
			const delta = await cadSession.replaceSelection(selectionRevision, ids);
			setSelectionRevision(delta.revision);
			setSelected(delta.selectedIds);
		} catch (reason) {
			report(reason);
			cadSession
				.snapshot()
				.then((snapshot) => {
					setSelectionRevision(snapshot.selectionRevision);
					setSelected(snapshot.selectedIds);
				})
				.catch(report);
		}
	}

	/// The preview controls drive fixtures, so they need the rig the sheet is showing.
	function loadFixtures() {
		documentSession
			.patchSnapshot()
			.then((snapshot) => {
				setFixtures(snapshot.fixtures);
				setProfileRevisions(snapshot.profileRevisions);
			})
			.catch(report);
	}

	/// A document written on a desk arrives with its own layers, and its fixtures belong to them.
	function loadLayers() {
		documentSession
			.patchLayers()
			.then((stored) => setLayers(stored.length ? stored : [DEFAULT_LAYER]))
			.catch(report);
	}

	const report = useCallback((reason: unknown) => {
		setError(String(reason));
	}, []);
	useEffect(() => {
		if (!error) return;
		const timeout = window.setTimeout(() => setError(null), 8000);
		return () => window.clearTimeout(timeout);
	}, [error]);

	const host = useMemo<PatchHost>(
		() => ({
			library: {
				fixtureProfiles: profiles,
				// A planning document patches from transferable profiles only; the desk's legacy
				// definitions exist for shows recorded before profiles did.
				fixtureLibrary: [],
				patchLayers: sessionPatchLayers(layers),
				unresolvedMvrFixtures: [],
				savePatchLayer: async (layer) => {
					// The document is the authority, as it is on the desk: the sheet shows the
					// layer once the show actually has it.
					try {
						await documentSession.savePatchLayer(layer);
					} catch (reason) {
						report(reason);
						return false;
					}
					setLayers((current) =>
						current.some((existing) => existing.id === layer.id)
							? current.map((existing) =>
									existing.id === layer.id ? layer : existing,
								)
							: [...current, layer],
					);
					return true;
				},
			},
			// There is still no programmer here. What the sheet's selection drives is the preview
			// controls and nothing else: no cues, no tracking, no arbitration.
			selection: {
				fixtureIds: new Set(selected),
				orderedFixtureIds: selected,
				replace: (intent) => void replaceSelection(intent.resolvedFixtures),
			},
			// No `Set` key either, so editing a cell is always allowed.
			editArmed: true,
			desktopEditing: true,
			setEditArmed: () => undefined,
		}),
		[profiles, layers, selected, selectionRevision],
	);

	const definitions = useMemo(
		() => mergeFixtureDefinitions(profiles, []),
		[profiles],
	);
	const workspaceTitle =
		workspace === "patch"
			? "Patch"
			: workspace === "venue"
				? "Venue"
				: workspace === "effects"
					? "Effects"
					: workspace === "media"
						? "Media"
						: "Show";
	const filename = document ? showFileName(document.path) : "No show open";

	return (
		<div className="viz-editor">
			<WindowControls />
			<div className="viz-editor-shell">
				<aside className="viz-editor-sidebar">
					<div
						className="viz-editor-identity"
						data-tauri-drag-region
						title={filename}
						onPointerDown={beginWindowDrag}
					>
						<img
							src={appIcon}
							alt="ToskLight PreViz"
							className="viz-editor-app-icon"
						/>
						<span>{filename}</span>
					</div>
					<OperatorDestinationList
						ariaLabel="Visualizer screens"
						activeId={workspace}
						onSelect={(id) => {
							if (id === "venue") loadFixtures();
							if (id === "show") setShowPage("show");
							setWorkspace(id as typeof workspace);
						}}
						entries={[
							{ id: "show", label: "Show", icon: <span>◫</span> },
							{
								id: "patch",
								label: "Patch",
								icon: <span>⌘</span>,
								disabled: !document,
							},
							{
								id: "venue",
								label: "Venue",
								icon: <span>◇</span>,
								disabled: !document,
							},
							{
								id: "effects",
								label: "Effects",
								icon: <span>✦</span>,
								disabled: !document,
							},
							{
								id: "media",
								label: "Media",
								icon: <span>▣</span>,
								disabled: !document,
							},
						]}
					/>
					<Button
						className="viz-editor-settings-nav"
						active={workspace === "settings"}
						onClick={() => {
							setShowPage("rendering");
							setWorkspace("settings");
						}}
					>
						Settings
					</Button>
					<Button
						className="viz-editor-open-cad"
						disabled={!document || openingCad}
						onClick={() => {
							setOpeningCad(true);
							documentSession
								.openCad()
								.catch(report)
								.finally(() => setOpeningCad(false));
						}}
					>
						{openingCad ? "Opening CAD…" : "Open CAD"}
					</Button>
					<Button
						className="viz-editor-open-viz"
						disabled={!document || openingViz}
						onClick={() => {
							setOpeningViz(true);
							documentSession
								.openVisualizer()
								.catch(report)
								.finally(() => setOpeningViz(false));
						}}
					>
						{openingViz ? "Opening Viz…" : "Open Viz"}
					</Button>
				</aside>
				<main
					className="viz-editor-workspace"
					onPointerDown={(event) => {
						const target = event.target as HTMLElement;
						if (!target.closest(".show-patch-layout > .ui-window-header"))
							return;
						if (
							target.closest("button, input, select, textarea, [role='button']")
						)
							return;
						beginWindowDrag(event);
					}}
				>
					{workspace === "show" || workspace === "settings" ? (
						<section className="viz-show-settings-workspace">
							<WindowHeader
								title="Show"
								dragHandleProps={{
									"data-tauri-drag-region": true,
									onPointerDown: beginWindowDrag,
								}}
								groups={[
									{
										id: "show-settings-pages",
										kind: "tabs",
										activeId: showPage,
										onActiveChange: (id) => {
											const page = id as ShowPage;
											setShowPage(page);
											setWorkspace(page === "show" ? "show" : "settings");
										},
										actions: [
											{ id: "show", label: "Show" },
											{ id: "dmx", label: "DMX", disabled: !document },
											{ id: "rendering", label: "Rendering" },
											{ id: "atmosphere", label: "Atmosphere" },
											{ id: "picture", label: "Picture" },
											{ id: "features", label: "Features" },
										],
									},
								]}
							/>
							{showPage === "show" ? (
								<FileBar
									page="show"
									document={document}
									onDocument={setDocument}
									onError={report}
									onReloadProfiles={() =>
										documentSession
											.fixtureProfiles()
											.then(setProfiles)
											.catch(report)
									}
									onReloadDocument={() => {
										setReload((current) => current + 1);
										documentSession.current().then(setDocument).catch(report);
										loadLayers();
										loadFixtures();
									}}
								/>
							) : showPage === "dmx" ? (
								<FileBar
									page="dmx"
									document={document}
									onDocument={setDocument}
									onError={report}
									onReloadProfiles={() =>
										documentSession.fixtureProfiles().then(setProfiles).catch(report)
									}
									onReloadDocument={() => undefined}
								/>
							) : (
								<RendererSettingsWorkspace page={showPage} onError={report} />
							)}
						</section>
					) : null}
					{document &&
					workspace !== "show" &&
					workspace !== "media" &&
					workspace !== "settings" ? (
						<PatchHostProvider value={host}>
							<PatchViewProvider
								key={`${document.showId}-${reload}`}
								showId={document.showId}
								initialFixtures={[]}
								definitions={definitions}
								transport={transport}
								onError={report}
							>
								<FixturePatchSetup
									title={workspaceTitle}
									scope={workspace === "patch" ? "dmx" : workspace}
								/>
							</PatchViewProvider>
							<PreviewControls
								fixtures={fixtures}
								profileRevisions={profileRevisions}
								selected={selected}
								onError={report}
							/>
						</PatchHostProvider>
					) : null}
					{document && workspace === "media" ? (
						<PatchHostProvider value={host}>
							<PatchViewProvider
								key={`${document.showId}-${reload}-media`}
								showId={document.showId}
								initialFixtures={[]}
								definitions={definitions}
								transport={transport}
								onError={report}
							>
								<MediaWorkspace onError={report} />
							</PatchViewProvider>
						</PatchHostProvider>
					) : null}
					{!document && workspace !== "show" && workspace !== "settings" ? (
						<section className="viz-editor-empty">
							<h1>No show open</h1>
							<p>
								Create a rig or open an existing show file. What you patch here
								is what the visualizer draws.
							</p>
						</section>
					) : null}
				</main>
			</div>
			{error ? (
				<output className="viz-editor-toast" role="alert">
					<span>{error}</span>
					<Button aria-label="Dismiss error" onClick={() => setError(null)}>
						×
					</Button>
				</output>
			) : null}
		</div>
	);
}

function showFileName(path: string) {
	return path.split(/[\\/]/u).pop() || "Untitled.show";
}
