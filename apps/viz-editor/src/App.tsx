import {
	FixturePatchSetup,
	noPatchSelection,
	type PatchHost,
	PatchHostProvider,
	type PatchLayer,
	PatchViewProvider,
	type FixtureProfile,
	mergeFixtureDefinitions,
} from "@tosklight/patch";
import { useEffect, useMemo, useState } from "react";
import { FileBar } from "./FileBar";
import { documentSession, sessionPatchLayers } from "./document/session";
import type { DocumentSummary } from "./document/session";
import { TauriPatchTransport } from "./document/transport";

const DEFAULT_LAYER: PatchLayer = { id: "default", name: "Default", order: 0 };

export function App() {
	const [document, setDocument] = useState<DocumentSummary | null>(null);
	const [profiles, setProfiles] = useState<readonly FixtureProfile[]>([]);
	const [layers, setLayers] = useState<readonly PatchLayer[]>([DEFAULT_LAYER]);
	const [error, setError] = useState<string | null>(null);
	// Bumped when something outside the sheet changed the document — an MVR import — so the sheet
	// reads the new snapshot instead of showing the rig as it was before.
	const [reload, setReload] = useState(0);
	const transport = useMemo(() => new TauriPatchTransport(), []);

	useEffect(() => {
		documentSession.current().then(setDocument).catch(report);
		documentSession.fixtureProfiles().then(setProfiles).catch(report);
		loadLayers();
	}, []);

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
			// No programmer here: selecting a row moves the sheet's own cursor and nothing else.
			selection: noPatchSelection,
			// No `Set` key either, so editing a cell is always allowed.
			editArmed: true,
			setEditArmed: () => undefined,
		}),
		[profiles, layers],
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
				}}
			/>
			{error ? (
				<output className="viz-editor-error">{error}</output>
			) : null}
			{document ? (
				<PatchHostProvider value={host}>
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
				</PatchHostProvider>
			) : (
				<section className="viz-editor-empty">
					<h1>No show open</h1>
					<p>
						Create a rig or open an existing show file. What you patch here is what the
						visualizer draws.
					</p>
				</section>
			)}
		</div>
	);
}
