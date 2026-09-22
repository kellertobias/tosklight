import {
	type FixtureDefinition,
	type FixtureNote,
	FixturePatchSetup,
	type FixtureProfile,
	type FixtureVisibility,
	mergeFixtureDefinitions,
	type PatchFixtureProjection,
	type PatchFixtureWrite,
	type PatchHost,
	PatchHostProvider,
	type PatchLayer,
	type PatchProfileRevision,
	PatchViewProvider,
	revealPatchRow,
} from "@tosklight/patch";
import { open } from "@tauri-apps/plugin-dialog";
import { Button, type TitleActionGroup } from "@tosklight/ui";
import {
	type ComponentProps,
	type ReactNode,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import {
	ArchitectSettings,
	NoShowOpen,
	SETTINGS_PAGES,
	type SettingsPage,
} from "./ArchitectSettings";
import { CadApp } from "./cad/CadApp";
import { CadAddFlows, type CadAddRequest } from "./cad/CadAddFlows";
import { CadToolProvider } from "./cad/cadTools";
import { VENUE_MODEL_EXTENSIONS } from "./cad/venueModelFormats";
import { cadSession } from "./cad/session";
import { useCadSelection } from "./cad/useCadSelection";
import type { CadEntity, CadSceneSnapshot } from "./cad/types";
import { DmxPatchScreen } from "./DmxPatchScreen";
import type { DmxPage } from "./DmxWorkspace";
import type { DocumentSummary } from "./document/session";
import { documentSession, sessionPatchLayers } from "./document/session";
import { TauriPatchTransport } from "./document/transport";
import type { PatchTransport } from "@tosklight/patch/transport";
import { type EditorWorkspace, EditorSidebar } from "./EditorSidebar";
import { MediaWorkspace } from "./MediaWorkspace";
import { PreviewControls } from "./PreviewControls";
import { ShowScreen } from "./ShowScreen";
import { beginWindowDrag, WindowControls } from "./WindowChrome";

const DEFAULT_LAYER: PatchLayer = {
	id: "default",
	name: "Default",
	order: 0,
	locked: false,
	visible2d: true,
	visible3d: true,
};

/** The Patch screen's two pages: the sheet itself, and which DMX addresses the rig occupies. */
type PatchPage = "sheet" | "dmx";

/**
 * What each CAD add action opens the fixture library on. Trusses are rigging, stage elements and
 * curtains are Venue objects searched by name, and a Venue element is any object that is not patched.
 */
/**
 * The patch sheet's two providers, which every screen built on the sheet needs in the same shape.
 *
 * The preview controls sit inside it as an ordinary child: neither provider renders an element,
 * and the controls read no patch context of their own.
 */
function PatchScope({
	host,
	showId,
	reload,
	suffix,
	definitions,
	transport,
	onError,
	children,
}: {
	host: PatchHost;
	showId: string;
	reload: number;
	suffix?: string;
	definitions: FixtureDefinition[];
	transport: PatchTransport;
	onError: (reason: unknown) => void;
	children: ReactNode;
}) {
	return (
		<PatchHostProvider value={host}>
			<PatchViewProvider
				key={`${showId}-${reload}${suffix ? `-${suffix}` : ""}`}
				showId={showId}
				initialFixtures={[]}
				definitions={definitions}
				transport={transport}
				onError={onError}
			>
				{children}
			</PatchViewProvider>
		</PatchHostProvider>
	);
}

/**
 * The shared patch sheet with what only the Architect offers: the column quick views in the column
 * settings, and the visible columns remembered per screen on this machine.
 */
function ArchitectPatchSheet(
	props: NonNullable<ComponentProps<typeof FixturePatchSetup>>,
) {
	return (
		<FixturePatchSetup
			{...props}
			quickViews
			// Patch keeps the columns it remembered while it listed lamps without their effects.
			columnStorageKey={`viz-editor.patch-columns.${
				props.scope === "patch" ? "dmx" : (props.scope ?? "all")
			}`}
			onImportVenueModel={props.scope === "patch" ? importVenueModel : undefined}
		/>
	);
}

/**
 * Choose a 3D model on this computer — GLB, glTF, 3MF or OBJ — and place it in the show as a venue
 * object. Whatever the format, the show keeps it as one GLB.
 *
 * Every window's sheet and the CAD views hear the patch change the import makes, so nothing here
 * has to reload them.
 */
async function importVenueModel(layerId: string) {
	const path = await open({
		multiple: false,
		directory: false,
		filters: [{ name: "3D model", extensions: [...VENUE_MODEL_EXTENSIONS] }],
	});
	if (typeof path !== "string") return null;
	const imported = await documentSession.importVenueModel(path, layerId);
	return imported.fixtureId;
}

/**
 * Saves or deletes one patch layer in the document. The sidebar shows the change at once and takes
 * it back, with the reason, when the document refuses it.
 */
async function changeSessionLayers(
	layers: readonly PatchLayer[],
	setLayers: (layers: readonly PatchLayer[]) => void,
	report: (reason: unknown) => void,
	change: { save: PatchLayer } | { remove: string },
) {
	if ("save" in change) {
		const { save } = change;
		setLayers(
			layers.some((existing) => existing.id === save.id)
				? layers.map((existing) => (existing.id === save.id ? save : existing))
				: [...layers, save],
		);
	} else setLayers(layers.filter((layer) => layer.id !== change.remove));
	try {
		if ("save" in change) await documentSession.savePatchLayer(change.save);
		else await documentSession.deletePatchLayer(change.remove);
	} catch (reason) {
		setLayers(layers);
		report(reason);
		return false;
	}
	return true;
}

/** The Patch title's Sheet and DMX tabs. */
function patchPageTabs(
	active: PatchPage,
	onChange: (page: PatchPage) => void,
): TitleActionGroup {
	return {
		id: "patch-pages",
		kind: "tabs",
		activeId: active,
		onActiveChange: (id) => onChange(id as PatchPage),
		actions: [
			{ id: "sheet", label: "Sheet" },
			{ id: "dmx", label: "DMX" },
		],
	};
}

/** Writes the fixtures the DMX grid moved, as one patch change. */
function writeMovedPatch(
	transport: PatchTransport,
	showId: string,
	fixtures: readonly PatchFixtureWrite[],
) {
	return transport.patchFixtures(showId, 0, {
		requestId: crypto.randomUUID(),
		fixtures,
		removeFixtureIds: [],
	});
}

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
	const [workspace, setWorkspace] = useState<EditorWorkspace>("show");
	const [settingsPage, setSettingsPage] = useState<SettingsPage>("visualizer");
	const [dmxPage, setDmxPage] = useState<DmxPage>("network");
	const [patchPage, setPatchPage] = useState<PatchPage>("sheet");
	// Each press of a CAD add action is a new request, so pressing the same one again reopens it.
	const [cadAdd, setCadAdd] = useState<CadAddRequest>({
		kind: "venue",
		request: 0,
	});
	const [visualizerRunning, setVisualizerRunning] = useState(false);
	// Bumped when something outside the sheet changed the document — an MVR import — so the sheet
	// reads the new snapshot instead of showing the rig as it was before.
	const [reload, setReload] = useState(0);
	// What the patch sheet has selected, and what the preview controls therefore drive.
	const {
		selected,
		revision: selectionRevision,
		revealRequest,
		receive: receiveSelection,
		replace: replaceSelection,
	} = useCadSelection((reason) => report(reason));
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
		receiveSelection(ids, revision);
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
				receiveSelection(delta.selectedIds, delta.revision);
				if (delta.selectedIds.length) {
					// Selecting in the drawing shows what was selected. On the CAD screen the
					// drawing already is that view, so following the selection to the sheet would
					// take the operator away from the thing they just clicked. A Venue object is on the
					// Patch sheet too, once the reveal has switched Show all on.
					setWorkspace((current) => {
						if (current === "cad") return current;
						setPatchPage("sheet");
						return "patch";
					});
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

	useEffect(() => {
		if (workspace !== "patch" || !selected.length) return;
		const frame = window.requestAnimationFrame(() => {
			const row = window.document.querySelector<HTMLElement>(
				`[data-fixture-id="${selected[0]}"]`,
			);
			if (row) revealPatchRow(row);
		});
		return () => window.cancelAnimationFrame(frame);
	}, [workspace, selected]);

	/// The preview controls drive fixtures, so they need the rig the sheet is showing.
	function loadFixtures() {
		return documentSession
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
	/** Re-read the machine's fixture library, after authoring a fixture or opening a document. */
	const reloadProfiles = useCallback(() => {
		documentSession.fixtureProfiles().then(setProfiles).catch(report);
	}, [report]);
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
				savePatchLayer: (layer) =>
					changeSessionLayers(layers, setLayers, report, { save: layer }),
				deletePatchLayer: (layerId) =>
					changeSessionLayers(layers, setLayers, report, { remove: layerId }),
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
	const filename = document ? showFileName(document.path) : "No show open";

	const patchPages = patchPageTabs(patchPage, (page) => {
		loadFixtures();
		setPatchPage(page);
	});

	const settingsPages: TitleActionGroup = {
		id: "settings-pages",
		kind: "tabs",
		activeId: settingsPage,
		onActiveChange: (id) => {
			if (id === "dmx") loadFixtures();
			setSettingsPage(id as SettingsPage);
		},
		actions: SETTINGS_PAGES.map(({ id, label }) => ({ id, label })),
	};

	return (
		<div className="viz-editor">
			<WindowControls />
			<div className="viz-editor-shell">
				<EditorSidebar
					filename={filename}
					workspace={workspace}
					hasDocument={Boolean(document)}
					onSelectWorkspace={(id) => {
						if (id === "patch") loadFixtures();
						setWorkspace(id);
					}}
					onSelectSettings={() => setWorkspace("settings")}
					openWindow={() =>
						documentSession
							.openWindow()
							.then(() => undefined)
							.catch(report)
					}
					openVisualizer={() =>
						documentSession
							.openVisualizer()
							.then(() => setVisualizerRunning(true))
							.catch(report)
					}
				/>
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
					{workspace === "show" ? (
						<ShowScreen
							document={document}
							cadScene={cadScene}
							onDocument={setDocument}
							onReloadProfiles={reloadProfiles}
							onReloadDocument={reloadDocument}
							onError={report}
						/>
					) : null}
					{workspace === "settings" ? (
						<ArchitectSettings
							page={settingsPage}
							pages={settingsPages}
							document={document}
							profiles={profiles}
							fixtures={fixtures}
							profileRevisions={profileRevisions}
							dmxPage={dmxPage}
							onDmxPage={setDmxPage}
							onReloadProfiles={reloadProfiles}
							onError={report}
						/>
					) : null}
					{document && workspace === "cad" ? (
						<CadToolProvider
							documentKey={document.showId}
							onAdd={(kind, profileId, several) =>
								setCadAdd((current) => ({
									kind,
									profileId,
									several,
									request: current.request + 1,
								}))
							}
						>
							<CadApp />
							{/* The add flows write to the show themselves, so a scene change is no reason to
							    remount them: a remount would replay the last add request. */}
							<CadAddFlows add={cadAdd} onError={report} />
						</CadToolProvider>
					) : null}
					{document && workspace === "patch" && patchPage === "dmx" ? (
						<DmxPatchScreen
							pages={patchPages}
							fixtures={fixtures}
							profileRevisions={profileRevisions}
							onApplyPatch={async (moved) => {
								await writeMovedPatch(transport, document.showId, moved);
								await loadFixtures(); // The move stays drawn until the rig reads back.
								setReload((current) => current + 1);
							}}
						/>
					) : null}
					{document && workspace === "patch" && patchPage === "sheet" ? (
						<PatchScope
							host={host}
							showId={document.showId}
							reload={reload}
							definitions={definitions}
							transport={transport}
							onError={report}
						>
							<ArchitectPatchSheet
								title="Patch"
								scope="patch"
								trailingTitleGroups={[patchPages]}
								showAllLayersRequest={revealRequest}
							/>
							{visualizerRunning ? (
								<PreviewControls
									fixtures={fixtures}
									profileRevisions={profileRevisions}
									selected={selected}
									onError={report}
								/>
							) : null}
						</PatchScope>
					) : null}
					{document && workspace === "media" ? (
						<PatchScope
							host={host}
							showId={document.showId}
							reload={reload}
							suffix="media"
							definitions={definitions}
							transport={transport}
							onError={report}
						>
							<MediaWorkspace onError={report} />
						</PatchScope>
					) : null}
					{!document && workspace !== "settings" && workspace !== "show" ? (
						<NoShowOpen />
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
