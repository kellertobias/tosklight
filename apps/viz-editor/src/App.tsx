import {
	type FixtureNote,
	FixturePatchSetup,
	type FixtureProfile,
	type FixtureVisibility,
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
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import appIcon from "../src-tauri/icons/icon.svg";
import { CadApp } from "./cad/CadApp";
import { CadRigOverview } from "./cad/CadViewport";
import { cadSession } from "./cad/session";
import type { CadEntity, CadSceneSnapshot } from "./cad/types";
import type { DocumentSummary } from "./document/session";
import { documentSession, sessionPatchLayers } from "./document/session";
import { TauriPatchTransport } from "./document/transport";
import { FileBar } from "./FileBar";
import { MediaWorkspace } from "./MediaWorkspace";
import { PreviewControls } from "./PreviewControls";
import { RendererSettingsWorkspace } from "./RendererSettingsWorkspace";
import { beginWindowDrag, WindowControls } from "./WindowChrome";

const DEFAULT_LAYER: PatchLayer = {
	id: "default",
	name: "Default",
	order: 0,
	locked: false,
	visible2d: true,
	visible3d: true,
};
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
	const [fixtureVisibility, setFixtureVisibility] = useState<
		ReadonlyMap<string, FixtureVisibility>
	>(new Map());
	const [fixtureNotes, setFixtureNotes] = useState<
		ReadonlyMap<string, FixtureNote>
	>(new Map());
	const [error, setError] = useState<string | null>(null);
	const [workspace, setWorkspace] = useState<
		"show" | "cad" | "patch" | "venue" | "effects" | "media" | "settings"
	>("show");
	const [showPage, setShowPage] = useState<ShowPage>("show");
	const [openingWindow, setOpeningWindow] = useState(false);
	const [openingViz, setOpeningViz] = useState(false);
	const [visualizerRunning, setVisualizerRunning] = useState(false);
	// Bumped when something outside the sheet changed the document — an MVR import — so the sheet
	// reads the new snapshot instead of showing the rig as it was before.
	const [reload, setReload] = useState(0);
	// What the patch sheet has selected, and what the preview controls therefore drive.
	const [selected, setSelected] = useState<readonly string[]>([]);
	const [selectionRevision, setSelectionRevision] = useState(0);
	const selectionRevisionRef = useRef(0);
	const selectionQueue = useRef<Promise<void>>(Promise.resolve());
	const cadEntitiesRef = useRef(new Map<string, CadEntity>());
	const [cadScene, setCadScene] = useState<CadSceneSnapshot | null>(null);
	// The rig itself, for the preview controls: the sheet owns the table, this owns the values.
	const [fixtures, setFixtures] = useState<readonly PatchFixtureProjection[]>(
		[],
	);
	// The profile revisions the show embedded, which are what Full DMX reads its slots from.
	const [profileRevisions, setProfileRevisions] = useState<
		readonly PatchProfileRevision[]
	>([]);
	const transport = useMemo(() => new TauriPatchTransport(), []);

	function applyCadSnapshot(snapshot: CadSceneSnapshot) {
		setCadScene(snapshot);
		cadEntitiesRef.current = new Map(
			snapshot.entities.map((entity) => [entity.id, entity]),
		);
		const ids = Array.isArray(snapshot.selectedIds) ? snapshot.selectedIds : [];
		const revision = Number.isFinite(snapshot.selectionRevision)
			? snapshot.selectionRevision
			: 0;
		setSelected(ids);
		setSelectionRevision(revision);
		selectionRevisionRef.current = revision;
	}

	function loadCadScene() {
		return cadSession.snapshot().then(applyCadSnapshot);
	}

	useEffect(() => {
		documentSession
			.visualizerIsRunning()
			.then(setVisualizerRunning)
			.catch(() => undefined);
		const refresh = () => {
			documentSession
				.visualizerIsRunning()
				.then(setVisualizerRunning)
				.catch(() => undefined);
		};
		window.addEventListener("focus", refresh);
		return () => window.removeEventListener("focus", refresh);
	}, []);

	useEffect(() => {
		if (!visualizerRunning) return;
		const interval = window.setInterval(() => {
			documentSession
				.visualizerIsRunning()
				.then(setVisualizerRunning)
				.catch(() => undefined);
		}, 500);
		return () => window.clearInterval(interval);
	}, [visualizerRunning]);

	useEffect(() => {
		documentSession
			.current()
			.then((summary) => {
				setDocument(summary);
				if (summary) {
					loadLayers();
					loadFixtureVisibility();
					loadFixtureNotes();
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
		loadCadScene().catch(() => undefined);
		cadSession
			.onSelectionDelta((delta) => {
				setCadScene((current) =>
					current
						? {
								...current,
								selectionRevision: delta.revision,
								selectedIds: delta.selectedIds,
							}
						: current,
				);
				setSelected(delta.selectedIds);
				setSelectionRevision(delta.revision);
				selectionRevisionRef.current = delta.revision;
				if (delta.selectedIds.length) {
					const selectedEntity = cadEntitiesRef.current.get(
						delta.selectedIds[0],
					);
					// Selecting in the drawing shows what was selected. On the CAD screen the
					// drawing already is that view, so following the selection to the sheet would
					// take the operator away from the thing they just clicked.
					setWorkspace((current) =>
						current === "cad"
							? current
							: selectedEntity?.kind === "venue"
								? "venue"
								: "patch",
					);
				}
			})
			.then((unlisten) => {
				selectionUnlisten = unlisten;
			})
			.catch(() => undefined);
		cadSession
			.onSceneDelta((delta) => {
				const next = new Map(cadEntitiesRef.current);
				for (const id of delta.removedIds) next.delete(id);
				for (const entity of delta.upserted) next.set(entity.id, entity);
				cadEntitiesRef.current = next;
				setCadScene((current) =>
					current
						? {
								...current,
								sceneRevision: delta.sceneRevision,
								entities: [...next.values()],
								drawings: delta.drawings.length
									? delta.drawings
									: current.drawings,
								attachments: delta.attachments,
							}
						: current,
				);
				loadFixtures();
				setReload((current) => current + 1);
			})
			.then((unlisten) => {
				sceneUnlisten = unlisten;
			})
			.catch(() => undefined);
		return () => {
			selectionUnlisten?.();
			sceneUnlisten?.();
		};
	}, []);

	// Another window opened, renamed or imported into the same document. The session is the
	// authority for both windows, so this one reads it again instead of being told what changed.
	useEffect(() => {
		let unlisten: (() => void) | undefined;
		documentSession
			.onDocumentChanged(() => reloadDocument())
			.then((stop) => {
				unlisten = stop;
			})
			.catch(() => undefined);
		return () => unlisten?.();
	}, []);

	function replaceSelection(ids: readonly string[]) {
		setSelected(ids);
		selectionQueue.current = selectionQueue.current.then(async () => {
			try {
				const delta = await cadSession.replaceSelection(
					selectionRevisionRef.current,
					ids,
				);
				selectionRevisionRef.current = delta.revision;
				setSelectionRevision(delta.revision);
				setSelected(delta.selectedIds);
			} catch (reason) {
				report(reason);
				try {
					const snapshot = await cadSession.snapshot();
					selectionRevisionRef.current = snapshot.selectionRevision;
					setSelectionRevision(snapshot.selectionRevision);
					setSelected(snapshot.selectedIds);
				} catch (refreshReason) {
					report(refreshReason);
				}
			}
		});
	}

	useEffect(() => {
		if ((workspace !== "patch" && workspace !== "venue") || !selected.length)
			return;
		const frame = window.requestAnimationFrame(() => {
			const row = window.document.querySelector<HTMLElement>(
				`[data-fixture-id="${selected[0]}"]`,
			);
			row?.scrollIntoView({ block: "nearest" });
		});
		return () => window.cancelAnimationFrame(frame);
	}, [workspace, selected]);

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

	function loadFixtureVisibility() {
		documentSession
			.fixtureVisibility()
			.then((stored) =>
				setFixtureVisibility(
					new Map(
						stored.map((visibility) => [visibility.fixtureId, visibility]),
					),
				),
			)
			.catch(report);
	}

	function loadFixtureNotes() {
		documentSession
			.fixtureNotes()
			.then((stored) =>
				setFixtureNotes(new Map(stored.map((note) => [note.fixtureId, note]))),
			)
			.catch(report);
	}

	/// Read the session again, from the top.
	///
	/// Whatever changed the document — this window's file bar, an MVR import, or another window
	/// on the same session — the answer is the same: nothing here is authoritative, so everything
	/// here is read back rather than patched in place.
	function reloadDocument() {
		setReload((current) => current + 1);
		documentSession.current().then(setDocument).catch(report);
		documentSession.fixtureProfiles().then(setProfiles).catch(report);
		loadLayers();
		loadFixtureVisibility();
		loadFixtureNotes();
		loadFixtures();
		loadCadScene().catch(report);
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
				fixtureVisibility,
				fixtureNotes,
				unresolvedMvrFixtures: [],
				savePatchLayer: async (layer) => {
					const previous = layers;
					setLayers((current) =>
						current.some((existing) => existing.id === layer.id)
							? current.map((existing) =>
									existing.id === layer.id ? layer : existing,
								)
							: [...current, layer],
					);
					try {
						await documentSession.savePatchLayer(layer);
					} catch (reason) {
						setLayers(previous);
						report(reason);
						return false;
					}
					return true;
				},
				saveFixtureVisibility: async (visibility) => {
					const previous = fixtureVisibility;
					setFixtureVisibility((current) => {
						const next = new Map(current);
						next.set(visibility.fixtureId, visibility);
						return next;
					});
					try {
						await documentSession.saveFixtureVisibility(visibility);
						return true;
					} catch (reason) {
						setFixtureVisibility(previous);
						report(reason);
						return false;
					}
				},
				saveFixtureNote: async (note) => {
					const previous = fixtureNotes;
					setFixtureNotes((current) => {
						const next = new Map(current);
						next.set(note.fixtureId, note);
						return next;
					});
					try {
						await documentSession.saveFixtureNote(note);
						return true;
					} catch (reason) {
						setFixtureNotes(previous);
						report(reason);
						return false;
					}
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
		[
			profiles,
			layers,
			fixtureVisibility,
			fixtureNotes,
			selected,
			selectionRevision,
		],
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
							alt="ToskLight Architect"
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
								id: "cad",
								label: "CAD",
								icon: <span>⊞</span>,
								disabled: !document,
							},
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
						className="viz-editor-open-window"
						title="Open another window on this show"
						disabled={openingWindow}
						onClick={() => {
							setOpeningWindow(true);
							documentSession
								.openWindow()
								.catch(report)
								.finally(() => setOpeningWindow(false));
						}}
					>
						{openingWindow ? "Opening…" : "Open Window"}
					</Button>
					<Button
						className="viz-editor-open-viz"
						disabled={!document || openingViz}
						onClick={() => {
							setOpeningViz(true);
							documentSession
								.openVisualizer()
								.then(() => setVisualizerRunning(true))
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
									onReloadDocument={reloadDocument}
								>
									{document && cadScene?.showId === document.showId ? (
										<figure className="viz-show-rig-overview">
											<figcaption>
												<span>Rig overview</span>
												<strong>{document.name}</strong>
											</figcaption>
											<CadRigOverview
												entities={cadScene.entities}
												drawings={cadScene.drawings}
												showName={document.name}
											/>
										</figure>
									) : null}
								</FileBar>
							) : showPage === "dmx" ? (
								<FileBar
									page="dmx"
									document={document}
									onDocument={setDocument}
									onError={report}
									onReloadProfiles={() =>
										documentSession
											.fixtureProfiles()
											.then(setProfiles)
											.catch(report)
									}
									onReloadDocument={() => undefined}
								/>
							) : (
								<RendererSettingsWorkspace page={showPage} onError={report} />
							)}
						</section>
					) : null}
					{document && workspace === "cad" ? <CadApp /> : null}
					{document &&
					workspace !== "show" &&
					workspace !== "cad" &&
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
									showAllLayersRequest={selected.length ? selectionRevision : 0}
								/>
							</PatchViewProvider>
							{visualizerRunning ? (
								<PreviewControls
									fixtures={fixtures}
									profileRevisions={profileRevisions}
									selected={selected}
									onError={report}
								/>
							) : null}
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
