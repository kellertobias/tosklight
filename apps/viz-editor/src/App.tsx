import {
	FixturePatchSetup,
	type PatchHost,
	PatchHostProvider,
	type PatchLayer,
	PatchViewProvider,
	type FixtureProfile,
	type PatchFixtureProjection,
	type PatchProfileRevision,
	mergeFixtureDefinitions,
} from "@tosklight/patch";
import { Button } from "@tosklight/ui";
import { useEffect, useMemo, useState } from "react";
import { FileBar } from "./FileBar";
import { MediaWorkspace } from "./MediaWorkspace";
import { PreviewControls } from "./PreviewControls";
import { documentSession, sessionPatchLayers } from "./document/session";
import type { DocumentSummary } from "./document/session";
import { TauriPatchTransport } from "./document/transport";
import { cadSession } from "./cad/session";

const DEFAULT_LAYER: PatchLayer = { id: "default", name: "Default", order: 0 };

export function App() {
	const [document, setDocument] = useState<DocumentSummary | null>(null);
	const [profiles, setProfiles] = useState<readonly FixtureProfile[]>([]);
	const [layers, setLayers] = useState<readonly PatchLayer[]>([DEFAULT_LAYER]);
	const [error, setError] = useState<string | null>(null);
	const [workspace, setWorkspace] = useState<"patch" | "media">("patch");
	// Bumped when something outside the sheet changed the document — an MVR import — so the sheet
	// reads the new snapshot instead of showing the rig as it was before.
	const [reload, setReload] = useState(0);
	// What the patch sheet has selected, and what the preview controls therefore drive.
	const [selected, setSelected] = useState<readonly string[]>([]);
	const [selectionRevision, setSelectionRevision] = useState(0);
	const [openingCad, setOpeningCad] = useState(false);
	const [openingViz, setOpeningViz] = useState(false);
	// The rig itself, for the preview controls: the sheet owns the table, this owns the values.
	const [fixtures, setFixtures] = useState<readonly PatchFixtureProjection[]>([]);
	// The profile revisions the show embedded, which are what Full DMX reads its slots from.
	const [profileRevisions, setProfileRevisions] = useState<
		readonly PatchProfileRevision[]
	>([]);
	const transport = useMemo(() => new TauriPatchTransport(), []);

	useEffect(() => {
		documentSession.current().then(setDocument).catch(report);
		documentSession.fixtureProfiles().then(setProfiles).catch(report);
		loadLayers();
		loadFixtures();
		// Say the interface is on screen. `--verify` waits for this and exits with the verdict:
		// a window that opens white reports nothing, which is exactly the failure to catch.
		documentSession.surfaceReady().catch(() => undefined);
	}, []);

	useEffect(() => {
		let unlisten: (() => void) | undefined;
		cadSession.snapshot().then((snapshot) => {
			setSelected(snapshot.selectedIds);
			setSelectionRevision(snapshot.selectionRevision);
		}).catch(() => undefined);
		cadSession.onSelectionDelta((delta) => {
			setSelected(delta.selectedIds);
			setSelectionRevision(delta.revision);
			if (delta.selectedIds.length) setWorkspace("patch");
		}).then((stop) => { unlisten = stop; }).catch(() => undefined);
		let sceneUnlisten: (() => void) | undefined;
		cadSession.onSceneDelta(() => loadFixtures()).then((stop) => {
			sceneUnlisten = stop;
		}).catch(() => undefined);
		return () => {
			unlisten?.();
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
			cadSession.snapshot().then((snapshot) => {
				setSelectionRevision(snapshot.selectionRevision);
				setSelected(snapshot.selectedIds);
			}).catch(report);
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

	function report(reason: unknown) {
		setError(String(reason));
	}

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
			setEditArmed: () => undefined,
		}),
		[profiles, layers, selected, selectionRevision],
	);

	const definitions = useMemo(
		() => mergeFixtureDefinitions(profiles, []),
		[profiles],
	);

	return (
		<div className="viz-editor">
			<FileBar
				document={document}
				onDocument={setDocument}
				onError={report}
				onReloadProfiles={() =>
					documentSession.fixtureProfiles().then(setProfiles).catch(report)
				}
				onReloadDocument={() => {
					setReload((current) => current + 1);
					documentSession.current().then(setDocument).catch(report);
					loadLayers();
					loadFixtures();
				}}
			/>
			{error ? (
				<output className="viz-editor-error">{error}</output>
			) : null}
			{document ? (
				<nav className="viz-editor-workspaces" aria-label="Visualizer authoring workspace">
					<Button active={workspace === "patch"} onClick={() => setWorkspace("patch")}>Patch & Venue</Button>
					<Button active={workspace === "media"} onClick={() => setWorkspace("media")}>Media</Button>
				</nav>
			) : null}
			{document ? (
				workspace === "patch" ? <PatchHostProvider value={host}>
					<PatchViewProvider
						key={`${document.showId}-${reload}`}
						showId={document.showId}
						initialFixtures={[]}
						definitions={definitions}
						transport={transport}
						onError={report}
					>
						<FixturePatchSetup />
					</PatchViewProvider>
					<PreviewControls
						fixtures={fixtures}
						profileRevisions={profileRevisions}
						selected={selected}
						onError={report}
					/>
				</PatchHostProvider> : <MediaWorkspace onError={report} />
			) : (
				<section className="viz-editor-empty">
					<h1>No show open</h1>
					<p>
						Create a rig or open an existing show file. What you patch here is what the
						visualizer draws.
					</p>
				</section>
			)}
			{document ? <nav className="viz-editor-launch-actions" aria-label="Planning applications">
				<Button onClick={async () => {
					setOpeningCad(true);
					try { await documentSession.openCad(); } catch (reason) { report(reason); }
					finally { setOpeningCad(false); }
				}}>{openingCad ? "Opening CAD…" : "Open CAD"}</Button>
				<Button onClick={async () => {
					setOpeningViz(true);
					try { await documentSession.openVisualizer(); } catch (reason) { report(reason); }
					finally { setOpeningViz(false); }
				}}>{openingViz ? "Opening Viz…" : "Open Viz"}</Button>
			</nav> : null}
		</div>
	);
}
