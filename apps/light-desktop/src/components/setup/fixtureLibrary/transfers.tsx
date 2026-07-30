import { Button, ModalRegistration, ModalTitleBar } from "@tosklight/ui";
import { useState } from "react";
import type {
	FixtureAttributeMapping,
	FixtureImportRequirement,
} from "../../../api/generated/light-wire";
import type { FixtureDefinition } from "../../../api/types";
import { useAttributeRegistry } from "../../../features/deskSnapshot/DeskSnapshotState";
import { useFixtureLibrary } from "../../../features/fixtureLibrary/FixtureLibraryContext";
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
	const server = useFixtureLibrary();
	const attributeRegistry = useAttributeRegistry() ?? [];
	const [busy, setBusy] = useState(false);
	const [modal, setModal] = useState<FixtureImportModal>(null);
	const [error, setError] = useState<string | null>(null);
	const [pendingPackage, setPendingPackage] = useState<Uint8Array | null>(null);
	const [requirements, setRequirements] = useState<
		FixtureImportRequirement[]
	>([]);
	const [mappings, setMappings] = useState<Record<string, string>>({});

	const selectModal = (next: FixtureImportModal) => {
		setError(null);
		setPendingPackage(null);
		setRequirements([]);
		setMappings({});
		setModal(next);
	};

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
		setError(null);
		setBusy(true);
		try {
			const source = new Uint8Array(await file.arrayBuffer());
			const imported = await importGdtfData(source, file.name);
			const profile = fixtureProfileFromDefinitions(imported);
			const saved = imported.length
				? ((await server?.saveFixtureProfile(profile, 0)) ?? null)
				: null;
			if (
				saved &&
				(await server?.saveFixtureProfileSourceGdtf(
					saved.id,
					saved.revision,
					source,
				))
			) {
				selectImportedProfile(saved);
			}
		} catch (reason) {
			setError(reason instanceof Error ? reason.message : String(reason));
		} finally {
			setBusy(false);
		}
	};

	const importPackage = async (file?: File) => {
		if (!file) return;
		setError(null);
		setBusy(true);
		try {
			const source = new Uint8Array(await file.arrayBuffer());
			const imported = await server?.importFixturePackage(source);
			if (imported?.type === "profile") {
				selectImportedProfile(imported.profile);
			} else if (imported?.type === "import_required") {
				setPendingPackage(source);
				setRequirements(imported.unknown_attributes);
				setMappings({});
			}
		} catch (reason) {
			setError(reason instanceof Error ? reason.message : String(reason));
		} finally {
			setBusy(false);
		}
	};

	const confirmPackageMappings = async () => {
		if (!pendingPackage || requirements.length === 0) return;
		const attributeMappings = requirements.map(
			(requirement): FixtureAttributeMapping => ({
				source_attribute: requirement.attribute,
				target_attribute: mappings[requirement.attribute] ?? "",
			}),
		);
		if (attributeMappings.some((mapping) => !mapping.target_attribute)) {
			setError("Choose a compatible descriptor for every imported attribute.");
			return;
		}
		setError(null);
		setBusy(true);
		try {
			const imported = await server?.importFixturePackage(
				pendingPackage,
				attributeMappings,
			);
			if (imported?.type === "profile") {
				selectImportedProfile(imported.profile);
			} else if (imported?.type === "import_required") {
				setRequirements(imported.unknown_attributes);
			}
		} catch (reason) {
			setError(reason instanceof Error ? reason.message : String(reason));
		} finally {
			setBusy(false);
		}
	};

	const exportSelectedPackage = async () => {
		if (!selectedMode) return;
		const id = selectedMode.profile_id ?? selectedMode.id;
		const blob = await server?.exportFixturePackage(id, selectedMode.revision);
		if (!blob) return;
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
		error,
		exportSelectedPackage,
		confirmPackageMappings,
		importGdtfFile,
		importPackage,
		mappingCandidates: attributeRegistry.filter(
			(descriptor) => !descriptor.retired,
		),
		mappings,
		modal,
		requirements,
		setMapping: (source: string, target: string) =>
			setMappings((current) => ({ ...current, [source]: target })),
		setModal: selectModal,
	};
}

interface FixtureImportDialogsProps {
	busy: boolean;
	error: string | null;
	modal: FixtureImportModal;
	close: () => void;
	confirmPackageMappings: () => Promise<void>;
	importGdtfFile: (file?: File) => Promise<void>;
	importPackage: (file?: File) => Promise<void>;
	mappingCandidates: ReturnType<typeof useAttributeRegistry>;
	mappings: Record<string, string>;
	requirements: FixtureImportRequirement[];
	setMapping: (source: string, target: string) => void;
}

export function FixtureImportDialogs({
	busy,
	error,
	modal,
	close,
	confirmPackageMappings,
	importGdtfFile,
	importPackage,
	mappingCandidates,
	mappings,
	requirements,
	setMapping,
}: FixtureImportDialogsProps) {
	return (
		<>
			{modal === "gdtf" && (
				<ModalRegistration onClose={close}>
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
							{error && <p role="alert">{error}</p>}
							<RootConfinedFilePickerButton
								variant="primary"
								disabled={busy}
								label={busy ? "Importing…" : "Choose GDTF file"}
								allowedExtensions={["gdtf"]}
								onFiles={(files) => importGdtfFile(files[0])}
							/>
						</section>
					</div>
				</ModalRegistration>
			)}
			{modal === "package" && (
				<ModalRegistration onClose={close}>
					<div className="stacked-modal-layer">
						<section className="nested-modal fixture-package-import-modal">
							<ModalTitleBar
								title="Import fixture"
								closeLabel="Close Import fixture"
								onClose={close}
							/>
							<p>
								Select a transferable .toskfixture package. Its modes,
								photograph, stage icon, and 3D model travel together.
							</p>
							{error && <p role="alert">{error}</p>}
							{requirements.length === 0 ? (
								<RootConfinedFilePickerButton
									variant="primary"
									disabled={busy}
									label={busy ? "Importing…" : "Choose fixture package"}
									allowedExtensions={["toskfixture"]}
									onFiles={(files) => importPackage(files[0])}
								/>
							) : (
								<>
									<p>
										Map each package attribute to a compatible configured
										descriptor. To preserve it as a new identity, first create
										and place a custom descriptor under <strong>Show → Desk Setup
										→ Programmer → Attributes</strong>, then choose the package
										again.
									</p>
									<div className="fixture-package-attribute-mappings">
										{requirements.map((requirement) => (
											<label key={requirement.attribute}>
												<span>
													<code>{requirement.attribute}</code>{" "}
													({requirement.value_type})
												</span>
												<select
													aria-label={`Map ${requirement.attribute}`}
													value={mappings[requirement.attribute] ?? ""}
													onChange={(event) =>
														setMapping(
															requirement.attribute,
															event.currentTarget.value,
														)
													}
												>
													<option value="">Choose descriptor…</option>
													{(mappingCandidates ?? [])
														.filter(
															(candidate) =>
																candidate.value_type ===
																requirement.value_type,
														)
														.map((candidate) => (
															<option
																key={candidate.id}
																value={candidate.id}
															>
																{candidate.label} ({candidate.id})
															</option>
														))}
												</select>
											</label>
										))}
									</div>
									<Button
										variant="primary"
										disabled={
											busy ||
											requirements.some(
												(requirement) =>
													!mappings[requirement.attribute],
											)
										}
										onClick={() => void confirmPackageMappings()}
									>
										{busy ? "Importing…" : "Import with mappings"}
									</Button>
								</>
							)}
						</section>
					</div>
				</ModalRegistration>
			)}
		</>
	);
}
