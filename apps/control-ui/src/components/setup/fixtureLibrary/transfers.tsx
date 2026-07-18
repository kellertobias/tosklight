import { useState } from "react";
import { useServer } from "../../../api/ServerContext";
import type { FixtureDefinition } from "../../../api/types";
import { ModalTitleBar } from "../../common";
import { RootConfinedFilePickerButton } from "../../files/RootConfinedFilePickerButton";
import { fixtureProfileFromDefinitions } from "../fixtureProfileModel";
import { importGdtfData } from "./gdtf";

export type FixtureImportModal = "gdtf" | "package" | null;

interface FixtureLibraryTransfersOptions {
	selectedMode: FixtureDefinition | null;
	setSelectedFamilyKey: (key: string) => void;
	setSelectedModeKey: (key: string) => void;
}

export function useFixtureLibraryTransfers({
	selectedMode,
	setSelectedFamilyKey,
	setSelectedModeKey,
}: FixtureLibraryTransfersOptions) {
	const server = useServer();
	const [busy, setBusy] = useState(false);
	const [modal, setModal] = useState<FixtureImportModal>(null);

	const selectImportedProfile = (profile: {
		id: string;
		revision: number;
		manufacturer: string;
		name: string;
		short_name: string;
		modes: { id: string }[];
	}) => {
		setSelectedFamilyKey(
			`${profile.manufacturer}\0${profile.short_name || profile.name}`,
		);
		setSelectedModeKey(
			`${profile.id}:${profile.revision}:${profile.modes[0]?.id ?? profile.id}`,
		);
		setModal(null);
	};

	const importGdtfFile = async (file?: File) => {
		if (!file) return;
		setBusy(true);
		try {
			const source = new Uint8Array(await file.arrayBuffer());
			const imported = await importGdtfData(source, file.name);
			const profile = fixtureProfileFromDefinitions(imported);
			const saved = imported.length
				? await server.saveFixtureProfile(profile, 0)
				: null;
			if (
				saved &&
				(await server.saveFixtureProfileSourceGdtf(
					saved.id,
					saved.revision,
					source,
				))
			) {
				selectImportedProfile(saved);
			}
		} finally {
			setBusy(false);
		}
	};

	const importPackage = async (file?: File) => {
		if (!file) return;
		setBusy(true);
		try {
			const imported = await server.importFixturePackage(
				new Uint8Array(await file.arrayBuffer()),
			);
			selectImportedProfile(imported);
		} finally {
			setBusy(false);
		}
	};

	const exportSelectedPackage = async () => {
		if (!selectedMode) return;
		const id = selectedMode.profile_id ?? selectedMode.id;
		const blob = await server.exportFixturePackage(id, selectedMode.revision);
		const url = URL.createObjectURL(blob);
		const anchor = document.createElement("a");
		anchor.href = url;
		anchor.download =
			`${selectedMode.manufacturer}-${selectedMode.name || selectedMode.model}.toskfixture`
				.replace(/[^a-z0-9._-]+/gi, "-")
				.toLowerCase();
		anchor.click();
		URL.revokeObjectURL(url);
	};

	return {
		busy,
		exportSelectedPackage,
		importGdtfFile,
		importPackage,
		modal,
		setModal,
	};
}

interface FixtureImportDialogsProps {
	busy: boolean;
	modal: FixtureImportModal;
	close: () => void;
	importGdtfFile: (file?: File) => Promise<void>;
	importPackage: (file?: File) => Promise<void>;
}

export function FixtureImportDialogs({
	busy,
	modal,
	close,
	importGdtfFile,
	importPackage,
}: FixtureImportDialogsProps) {
	return (
		<>
			{modal === "gdtf" && (
				<div className="stacked-modal-layer">
					<section className="nested-modal gdtf-import-modal">
						<ModalTitleBar
							title="Import GDTF"
							closeLabel="Close Import GDTF"
							onClose={close}
						/>
						<p>
							Select a GDTF archive. Every DMX mode will be imported into the
							desk-wide fixture library.
						</p>
						<RootConfinedFilePickerButton
							variant="primary"
							disabled={busy}
							label={busy ? "Importing…" : "Choose GDTF file"}
							allowedExtensions={["gdtf"]}
							onFiles={(files) => importGdtfFile(files[0])}
						/>
					</section>
				</div>
			)}
			{modal === "package" && (
				<div className="stacked-modal-layer">
					<section className="nested-modal fixture-package-import-modal">
						<ModalTitleBar
							title="Import fixture"
							closeLabel="Close Import fixture"
							onClose={close}
						/>
						<p>
							Select a transferable .toskfixture package. Its modes, photograph,
							stage icon, and 3D model travel together.
						</p>
						<RootConfinedFilePickerButton
							variant="primary"
							disabled={busy}
							label={busy ? "Importing…" : "Choose fixture package"}
							allowedExtensions={["toskfixture"]}
							onFiles={(files) => importPackage(files[0])}
						/>
					</section>
				</div>
			)}
		</>
	);
}
