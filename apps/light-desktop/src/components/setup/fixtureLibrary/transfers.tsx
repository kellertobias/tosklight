import {
	Button,
	ErrorAlert,
	FormLayout,
	formatErrorDetails,
	ModalRegistration,
	ModalTitleBar,
	OperationBusyOverlay,
	SelectField,
	TextField,
} from "@tosklight/ui";
import { useRef, useState } from "react";
import type { AttributeValueType } from "../../../api/attributeConfigurationModels";
import type {
	AttributeConfigurationSnapshot,
	AttributeEncoderGroup,
	ConfiguredAttributeDescriptor,
} from "../../../api/client/attributeConfiguration";
import type {
	FixtureAttributeMapping,
	FixtureGdtfImportPreview,
	FixtureImportRequirement,
} from "../../../api/client/fixtures";
import type { FixtureDefinition } from "../../../api/types";
import { useAttributeConfigurationActions } from "../../../features/attributeConfiguration/AttributeConfigurationActions";
import { useAttributeRegistry } from "../../../features/deskSnapshot/DeskSnapshotState";
import { useFixtureLibrary } from "../../../features/fixtureLibrary/FixtureLibraryContext";
import { RootConfinedFilePickerButton } from "../../files/RootConfinedFilePickerButton";
import { GdtfImportDialog } from "./GdtfImportDialog";

export type FixtureImportModal = "gdtf" | "package" | null;

interface FixtureLibraryTransfersOptions {
	selectedMode: FixtureDefinition | null;
	setSelectedFamilyKey: (key: string) => void;
	setSelectedModeKey: (key: string) => void;
}

export interface PendingGdtfImport extends FixtureGdtfImportPreview {
	source: Uint8Array;
	expectedRevision: number;
}

interface ImportedCustomAttributeDraft {
	sourceAttribute: string;
	label: string;
	valueType: AttributeValueType;
	encoderGroup: AttributeEncoderGroup;
	encoderPage: number;
	encoderSlot: number;
	activationGroupId: string;
	displayUnit: string;
	physicalUnit: string;
}

interface AttributeMappingCandidate {
	id: string;
	label: string;
	value_type: AttributeValueType;
	retired?: boolean;
}

const ENCODER_GROUP_OPTIONS: Array<{
	value: AttributeEncoderGroup;
	label: string;
}> = [
	{ value: "intensity", label: "Intensity" },
	{ value: "color", label: "Color" },
	{ value: "position", label: "Position" },
	{ value: "beam", label: "Beam" },
	{ value: "shapers", label: "Shapers" },
	{ value: "focus", label: "Focus" },
	{ value: "control", label: "Control" },
	{ value: "media", label: "Media" },
];

async function downloadFixturePackage(
	server: ReturnType<typeof useFixtureLibrary>,
	selectedMode: FixtureDefinition | null,
) {
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
}

function useFixtureTransferState() {
	const [busy, setBusyState] = useState(false);
	const busyRef = useRef(false);
	const setBusy = (value: boolean) => {
		busyRef.current = value;
		setBusyState(value);
	};
	const beginBusy = () => {
		if (busyRef.current) return false;
		setBusy(true);
		return true;
	};
	const [modal, setModal] = useState<FixtureImportModal>(null);
	const [error, setError] = useState<string | null>(null);
	const [pendingPackage, setPendingPackage] = useState<Uint8Array | null>(null);
	const [pendingGdtf, setPendingGdtf] = useState<PendingGdtfImport | null>(
		null,
	);
	const [requirements, setRequirements] = useState<FixtureImportRequirement[]>(
		[],
	);
	const [mappings, setMappings] = useState<Record<string, string>>({});
	const [createdAttributes, setCreatedAttributes] = useState<
		ConfiguredAttributeDescriptor[]
	>([]);
	const [customAttributeSnapshot, setCustomAttributeSnapshot] =
		useState<AttributeConfigurationSnapshot | null>(null);
	const [customAttributeDraft, setCustomAttributeDraft] =
		useState<ImportedCustomAttributeDraft | null>(null);
	const selectModal = (next: FixtureImportModal, completed = false) => {
		if (busyRef.current && !completed) return;
		setError(null);
		setPendingPackage(null);
		setPendingGdtf(null);
		setRequirements([]);
		setMappings({});
		setCustomAttributeSnapshot(null);
		setCustomAttributeDraft(null);
		setModal(next);
	};
	return {
		busy,
		isBusy: () => busyRef.current,
		beginBusy,
		setBusy,
		modal,
		selectModal,
		error,
		setError,
		pendingPackage,
		setPendingPackage,
		pendingGdtf,
		setPendingGdtf,
		requirements,
		setRequirements,
		mappings,
		setMappings,
		createdAttributes,
		setCreatedAttributes,
		customAttributeSnapshot,
		setCustomAttributeSnapshot,
		customAttributeDraft,
		setCustomAttributeDraft,
	};
}

type FixtureTransferState = ReturnType<typeof useFixtureTransferState>;

function customAttributeOperations(
	state: FixtureTransferState,
	attributeActions: ReturnType<typeof useAttributeConfigurationActions>,
) {
	const beginCustomAttribute = async (
		requirement: FixtureImportRequirement,
	) => {
		if (!attributeActions?.canWrite) {
			state.setError(
				"The primary desk is not ready to create show attributes.",
			);
			return;
		}
		state.setError(null);
		if (!state.beginBusy()) return;
		try {
			const snapshot = await attributeActions.load();
			const encoderGroup = defaultEncoderGroup(requirement.value_type);
			state.setCustomAttributeSnapshot(snapshot);
			state.setCustomAttributeDraft({
				sourceAttribute: requirement.attribute,
				label: importedAttributeLabel(requirement.attribute),
				valueType: requirement.value_type,
				encoderGroup,
				...nextAvailablePlacement(snapshot, encoderGroup),
				activationGroupId:
					requirement.value_type === "control" ? "" : "__new__",
				displayUnit: "",
				physicalUnit: "",
			});
		} catch (reason) {
			state.setError(formatErrorDetails(reason));
		} finally {
			state.setBusy(false);
		}
	};
	const editCustomAttribute = (
		patch: Partial<ImportedCustomAttributeDraft>,
	) => {
		state.setCustomAttributeDraft((current) => {
			if (!current) return current;
			if (
				patch.encoderGroup &&
				patch.encoderGroup !== current.encoderGroup &&
				state.customAttributeSnapshot
			)
				return {
					...current,
					...patch,
					...nextAvailablePlacement(
						state.customAttributeSnapshot,
						patch.encoderGroup,
					),
					activationGroupId: current.valueType === "control" ? "" : "__new__",
				};
			return { ...current, ...patch };
		});
	};
	const createCustomAttribute = async () => {
		const draft = state.customAttributeDraft;
		const snapshot = state.customAttributeSnapshot;
		if (!draft || !snapshot || !attributeActions?.canWrite) return;
		if (!draft.label.trim()) {
			state.setError("Enter a display label for the custom attribute.");
			return;
		}
		state.setError(null);
		if (!state.beginBusy()) return;
		try {
			const id = customAttributeId(draft.label);
			const custom = {
				id,
				label: draft.label.trim(),
				value_type: draft.valueType,
				display_unit: draft.displayUnit.trim() || null,
				physical_unit: draft.physicalUnit.trim() || null,
				normalized_bounds:
					draft.valueType === "continuous" ? { min: 0, max: 1 } : null,
				domain_bounds: null,
				cyclic: false,
				recordable: draft.valueType !== "control",
				lifecycle: "active" as const,
			};
			const activationGroups =
				draft.valueType === "control"
					? snapshot.configuration.activation_groups
					: draft.activationGroupId === "__new__"
						? [
								...snapshot.configuration.activation_groups,
								{ id, label: custom.label, members: [id] },
							]
						: snapshot.configuration.activation_groups.map((group) =>
								group.id === draft.activationGroupId
									? { ...group, members: [...group.members, id] }
									: group,
							);
			const updated = await attributeActions.update(snapshot, {
				custom_attributes: [
					...snapshot.configuration.custom_attributes,
					custom,
				],
				placements: [
					...snapshot.configuration.placements,
					{
						attribute: id,
						encoder_group: draft.encoderGroup,
						encoder_page: draft.encoderPage,
						encoder_slot: draft.encoderSlot,
						push_turn_of: null,
					},
				],
				activation_groups: activationGroups,
			});
			const created = updated.descriptors.find(
				(descriptor) => descriptor.id === id,
			);
			if (created)
				state.setCreatedAttributes((current) => [...current, created]);
			state.setMappings((current) => ({
				...current,
				[draft.sourceAttribute]: id,
			}));
			state.setCustomAttributeSnapshot(null);
			state.setCustomAttributeDraft(null);
		} catch (reason) {
			state.setError(formatErrorDetails(reason));
		} finally {
			state.setBusy(false);
		}
	};
	return { beginCustomAttribute, editCustomAttribute, createCustomAttribute };
}

type ImportedProfileSelection = {
	id: string;
	revision: number;
	manufacturer: string;
	name: string;
	short_name: string;
	modes: { id: string }[];
};

function gdtfOperations(
	state: FixtureTransferState,
	server: ReturnType<typeof useFixtureLibrary>,
	attributeRegistry: NonNullable<ReturnType<typeof useAttributeRegistry>>,
	selectImportedProfile: (profile: ImportedProfileSelection) => void,
) {
	const importGdtfFile = async (file?: File) => {
		if (!file) return;
		state.setError(null);
		if (!state.beginBusy()) return;
		state.setPendingGdtf(null);
		try {
			if (!server?.previewFixtureGdtf)
				throw new Error("GDTF import requires a connected desk server.");
			const source = new Uint8Array(await file.arrayBuffer());
			const preview = await server.previewFixtureGdtf(source);
			const remembered = (await server.fixtureSourceMappings?.()) ?? [];
			const types = new Map(
				attributeRegistry.map((descriptor) => [
					descriptor.id,
					descriptor.value_type,
				]),
			);
			const mappings = Object.fromEntries(
				preview.unknown_attributes.flatMap((requirement) => {
					const preference = remembered.find(
						(mapping) =>
							mapping.source_format === "gdtf" &&
							`gdtf.${mapping.source_attribute}` === requirement.attribute &&
							types.get(mapping.target_attribute) === requirement.value_type,
					);
					return preference
						? [[requirement.attribute, preference.target_attribute]]
						: [];
				}),
			);
			state.setPendingGdtf({
				...preview,
				source,
				expectedRevision:
					server.fixtureProfiles.find(
						(profile) => profile.id === preview.profile.id,
					)?.revision ?? 0,
			});
			state.setRequirements(preview.unknown_attributes);
			state.setMappings(mappings);
		} catch (reason) {
			state.setError(formatErrorDetails(reason));
		} finally {
			state.setBusy(false);
		}
	};
	const confirmGdtfMappings = async () => {
		const pending = state.pendingGdtf;
		if (!pending) return;
		const attributeMappings = state.requirements.map((requirement) => ({
			source_attribute: requirement.attribute,
			target_attribute: state.mappings[requirement.attribute] ?? "",
		}));
		if (attributeMappings.some((mapping) => !mapping.target_attribute)) {
			state.setError(
				"Choose a compatible descriptor for every GDTF attribute.",
			);
			return;
		}
		state.setError(null);
		if (!state.beginBusy()) return;
		try {
			if (!server?.importFixtureGdtf)
				throw new Error("GDTF import requires a connected desk server.");
			for (const requirement of state.requirements) {
				await server.rememberFixtureSourceMapping?.({
					sourceFormat: "gdtf",
					sourceAttribute: requirement.attribute.startsWith("gdtf.")
						? requirement.attribute.slice(5)
						: requirement.attribute,
					targetAttribute: state.mappings[requirement.attribute] ?? null,
				});
			}
			const saved = await server.importFixtureGdtf({
				profileId: pending.profile.id,
				expectedRevision: pending.expectedRevision,
				source: pending.source,
				attributeMappings,
			});
			selectImportedProfile(saved);
		} catch (reason) {
			state.setError(formatErrorDetails(reason));
		} finally {
			state.setBusy(false);
		}
	};
	return { importGdtfFile, confirmGdtfMappings };
}

export function useFixtureLibraryTransfers({
	selectedMode,
	setSelectedFamilyKey,
	setSelectedModeKey,
}: FixtureLibraryTransfersOptions) {
	const server = useFixtureLibrary();
	const attributeActions = useAttributeConfigurationActions();
	const attributeRegistry = useAttributeRegistry() ?? [];
	const state = useFixtureTransferState();
	const { beginCustomAttribute, editCustomAttribute, createCustomAttribute } =
		customAttributeOperations(state, attributeActions);
	const {
		busy,
		modal,
		error,
		pendingPackage,
		requirements,
		mappings,
		createdAttributes,
		customAttributeSnapshot,
		customAttributeDraft,
	} = state;
	const {
		setBusy,
		setError,
		setPendingPackage,
		setRequirements,
		setMappings,
		setCustomAttributeSnapshot,
		setCustomAttributeDraft,
		selectModal,
	} = state;

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
		selectModal(null, true);
	};

	const { importGdtfFile, confirmGdtfMappings } = gdtfOperations(
		state,
		server,
		attributeRegistry,
		selectImportedProfile,
	);

	const importPackage = async (file?: File) => {
		if (!file) return;
		setError(null);
		if (!state.beginBusy()) return;
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
			setError(formatErrorDetails(reason));
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
		if (!state.beginBusy()) return;
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
			setError(formatErrorDetails(reason));
		} finally {
			setBusy(false);
		}
	};

	return {
		activationGroupOptions: compatibleActivationGroups(
			customAttributeSnapshot,
			customAttributeDraft,
		),
		beginCustomAttribute,
		busy,
		cancelCustomAttribute: () => {
			if (state.isBusy()) return;
			setCustomAttributeSnapshot(null);
			setCustomAttributeDraft(null);
		},
		error,
		createCustomAttribute,
		customAttributeDraft,
		editCustomAttribute,
		exportSelectedPackage: () => downloadFixturePackage(server, selectedMode),
		confirmPackageMappings,
		confirmGdtfMappings,
		pendingGdtf: state.pendingGdtf,
		importGdtfFile,
		importPackage,
		mappingCandidates: [...attributeRegistry, ...createdAttributes].filter(
			(descriptor, index, all) =>
				!descriptor.retired &&
				all.findIndex((candidate) => candidate.id === descriptor.id) === index,
		),
		mappings,
		modal,
		requirements,
		setMapping: (source: string, target: string) =>
			setMappings((current) => ({ ...current, [source]: target })),
		setModal: selectModal,
		placementOptions: customAttributePlacementOptions(
			customAttributeSnapshot,
			customAttributeDraft,
		),
	};
}

export interface FixtureImportDialogsProps {
	pendingGdtf?: PendingGdtfImport | null;
	busy: boolean;
	error: string | null;
	modal: FixtureImportModal;
	close: () => void;
	confirmGdtfMappings: () => Promise<void>;
	confirmPackageMappings: () => Promise<void>;
	importGdtfFile: (file?: File) => Promise<void>;
	importPackage: (file?: File) => Promise<void>;
	mappingCandidates: AttributeMappingCandidate[];
	mappings: Record<string, string>;
	requirements: FixtureImportRequirement[];
	setMapping: (source: string, target: string) => void;
	activationGroupOptions: Array<{ value: string; label: string }>;
	beginCustomAttribute: (
		requirement: FixtureImportRequirement,
	) => Promise<void>;
	cancelCustomAttribute: () => void;
	createCustomAttribute: () => Promise<void>;
	customAttributeDraft: ImportedCustomAttributeDraft | null;
	editCustomAttribute: (patch: Partial<ImportedCustomAttributeDraft>) => void;
	placementOptions: Array<{ value: string; label: string }>;
}

function AttributeMappingFields({
	requirements,
	mappingCandidates,
	mappings,
	setMapping,
	activationGroupOptions,
	beginCustomAttribute,
	cancelCustomAttribute,
	createCustomAttribute,
	customAttributeDraft,
	editCustomAttribute,
	placementOptions,
	busy,
	workflow = false,
}: Pick<
	FixtureImportDialogsProps,
	| "requirements"
	| "mappingCandidates"
	| "mappings"
	| "setMapping"
	| "activationGroupOptions"
	| "beginCustomAttribute"
	| "cancelCustomAttribute"
	| "createCustomAttribute"
	| "customAttributeDraft"
	| "editCustomAttribute"
	| "placementOptions"
	| "busy"
> & { workflow?: boolean }) {
	return (
		<div className="fixture-package-attribute-mappings">
			{requirements.map((requirement) => (
				<div
					key={requirement.attribute}
					className={
						workflow
							? "fixture-attribute-mapping import-mapping-row"
							: "fixture-attribute-mapping"
					}
					data-state={
						workflow
							? mappings[requirement.attribute]
								? "mapped"
								: "unresolved"
							: undefined
					}
					data-import-unresolved={
						workflow && !mappings[requirement.attribute] ? "true" : undefined
					}
				>
					{workflow && (
						<div className="import-mapping-row__source">
							<strong>GDTF source</strong>
							<code>{requirement.attribute}</code>
							<small>{requirement.value_type}</small>
						</div>
					)}
					<SelectField
						className={workflow ? "import-mapping-row__target" : undefined}
						disabled={workflow && busy}
						label={
							workflow ? (
								"Destination attribute"
							) : (
								<span>
									<code>{requirement.attribute}</code> ({requirement.value_type}
									)
								</span>
							)
						}
						ariaLabel={`Map ${requirement.attribute}`}
						value={mappings[requirement.attribute] ?? ""}
						onChange={(value) => setMapping(requirement.attribute, value)}
						options={[
							{ value: "", label: "Choose descriptor…" },
							...(mappingCandidates ?? [])
								.filter(
									(candidate) =>
										candidate.value_type === requirement.value_type,
								)
								.map((candidate) => ({
									value: candidate.id,
									label: `${candidate.label} (${candidate.id})`,
								})),
						]}
					/>
					{workflow && (
						<span className="import-mapping-row__state">
							{mappings[requirement.attribute] ? "Mapped" : "Decision required"}
						</span>
					)}
					<Button
						disabled={busy || Boolean(customAttributeDraft)}
						onClick={() => void beginCustomAttribute(requirement)}
					>
						Create custom attribute
					</Button>
					{customAttributeDraft?.sourceAttribute === requirement.attribute && (
						<CustomAttributeImportFields
							draft={customAttributeDraft}
							activationGroupOptions={activationGroupOptions}
							placementOptions={placementOptions}
							onChange={editCustomAttribute}
							onCancel={cancelCustomAttribute}
							onCreate={createCustomAttribute}
							busy={busy}
						/>
					)}
				</div>
			))}
		</div>
	);
}

function CustomAttributeImportFields({
	draft,
	activationGroupOptions,
	placementOptions,
	onChange,
	onCancel,
	onCreate,
	busy,
}: {
	draft: ImportedCustomAttributeDraft;
	activationGroupOptions: Array<{ value: string; label: string }>;
	placementOptions: Array<{ value: string; label: string }>;
	onChange(patch: Partial<ImportedCustomAttributeDraft>): void;
	onCancel(): void;
	onCreate(): Promise<void>;
	busy: boolean;
}) {
	return (
		<FormLayout labelPlacement="side">
			<TextField
				label="Display label"
				value={draft.label}
				onChange={(event) => onChange({ label: event.target.value })}
			/>
			<SelectField
				label="Attribute type"
				ariaLabel="Attribute type"
				value={draft.valueType}
				options={[{ value: draft.valueType, label: draft.valueType }]}
				onChange={() => undefined}
			/>
			<SelectField
				label="Encoder group"
				ariaLabel="Encoder group"
				value={draft.encoderGroup}
				options={ENCODER_GROUP_OPTIONS}
				onChange={(value) =>
					onChange({ encoderGroup: value as AttributeEncoderGroup })
				}
			/>
			<SelectField
				label="Semantic placement"
				ariaLabel="Semantic placement"
				value={`${draft.encoderPage}:${draft.encoderSlot}`}
				options={placementOptions}
				onChange={(value) => {
					const [encoderPage, encoderSlot] = value.split(":").map(Number);
					onChange({ encoderPage, encoderSlot });
				}}
			/>
			{draft.valueType !== "control" && (
				<SelectField
					label="Activation group"
					ariaLabel="Activation group"
					value={draft.activationGroupId}
					options={[
						{ value: "__new__", label: "Own activation group" },
						...activationGroupOptions,
					]}
					onChange={(activationGroupId) => onChange({ activationGroupId })}
				/>
			)}
			<TextField
				label="Display unit"
				value={draft.displayUnit}
				onChange={(event) => onChange({ displayUnit: event.target.value })}
			/>
			<TextField
				label="Physical unit"
				value={draft.physicalUnit}
				onChange={(event) => onChange({ physicalUnit: event.target.value })}
			/>
			<div>
				<Button disabled={busy} onClick={onCancel}>
					Cancel
				</Button>
				<Button
					variant="primary"
					disabled={busy || !draft.label.trim()}
					onClick={() => void onCreate()}
				>
					{busy ? "Creating…" : "Create and use attribute"}
				</Button>
			</div>
		</FormLayout>
	);
}

export function FixtureImportDialogs({
	modal,
	...props
}: FixtureImportDialogsProps) {
	return (
		<>
			{modal === "gdtf" && (
				<GdtfImportDialog {...props}>
					<AttributeMappingFields {...props} workflow />
				</GdtfImportDialog>
			)}
			{modal === "package" && <PackageImportDialog {...props} />}
		</>
	);
}

type FixtureImportDialogProps = Omit<FixtureImportDialogsProps, "modal">;

function PackageImportDialog(props: FixtureImportDialogProps) {
	const {
		busy,
		error,
		close,
		confirmPackageMappings,
		importPackage,
		mappings,
		requirements,
	} = props;
	const [readingFile, setReadingFile] = useState(false);
	const readingRef = useRef(false);
	const readBusyChanged = (value: boolean) => {
		readingRef.current = value;
		setReadingFile(value);
	};
	const operationBusy = busy || readingFile;
	const requestClose = () => {
		if (!busy && !readingRef.current) close();
	};
	return (
		<ModalRegistration onClose={requestClose}>
			<div className="stacked-modal-layer">
				<section className="nested-modal fixture-package-import-modal">
					<ModalTitleBar
						title="Import fixture"
						closeLabel="Close Import fixture"
						closeDisabled={operationBusy}
						onClose={requestClose}
					/>
					<p>
						Select a transferable .toskfixture package. Its modes, photograph,
						stage icon, and 3D model travel together.
					</p>
					{error && (
						<ErrorAlert as="p" role="alert">
							{error}
						</ErrorAlert>
					)}
					{requirements.length === 0 ? (
						<RootConfinedFilePickerButton
							variant="primary"
							disabled={operationBusy}
							label={busy ? "Importing…" : "Choose fixture package"}
							allowedExtensions={["toskfixture"]}
							onReadBusyChange={readBusyChanged}
							onFiles={(files) => importPackage(files[0])}
						/>
					) : (
						<>
							<p>
								Map each package attribute to a compatible configured
								descriptor, or create and place a custom attribute here.
							</p>
							<AttributeMappingFields {...props} />
							<Button
								variant="primary"
								disabled={
									busy ||
									requirements.some(
										(requirement) => !mappings[requirement.attribute],
									)
								}
								onClick={() => void confirmPackageMappings()}
							>
								{busy ? "Importing…" : "Import with mappings"}
							</Button>
						</>
					)}
					{operationBusy && (
						<OperationBusyOverlay
							title={
								readingFile
									? "Loading selected fixture package…"
									: "Importing fixture package…"
							}
							message={
								readingFile
									? "Reading the selected package. Wait for the read to finish; it cannot be cancelled here."
									: "Loading and importing the package. It may already be writing fixture data; wait for the authoritative result."
							}
						/>
					)}
				</section>
			</div>
		</ModalRegistration>
	);
}

function defaultEncoderGroup(
	valueType: AttributeValueType,
): AttributeEncoderGroup {
	if (valueType === "color") return "color";
	if (valueType === "position") return "position";
	if (valueType === "control") return "control";
	return "beam";
}

function importedAttributeLabel(source: string) {
	return source
		.replace(/^GDTF:/u, "")
		.replace(/[._:-]+/gu, " ")
		.trim();
}

function customAttributeId(label: string) {
	const slug =
		label
			.toLowerCase()
			.replace(/[^a-z0-9]+/gu, ".")
			.replace(/^\.+|\.+$/gu, "") || "attribute";
	return `custom.${slug}.${crypto.randomUUID()}`;
}

function nextAvailablePlacement(
	snapshot: AttributeConfigurationSnapshot,
	encoderGroup: AttributeEncoderGroup,
) {
	const occupied = new Set(
		snapshot.configuration.placements
			.filter((placement) => placement.encoder_group === encoderGroup)
			.map(
				(placement) => `${placement.encoder_page}:${placement.encoder_slot}`,
			),
	);
	for (let encoderPage = 1; ; encoderPage += 1)
		for (let encoderSlot = 1; encoderSlot <= 6; encoderSlot += 1)
			if (!occupied.has(`${encoderPage}:${encoderSlot}`))
				return { encoderPage, encoderSlot };
}

function customAttributePlacementOptions(
	snapshot: AttributeConfigurationSnapshot | null,
	draft: ImportedCustomAttributeDraft | null,
) {
	if (!snapshot || !draft) return [];
	const placements = snapshot.configuration.placements.filter(
		(placement) => placement.encoder_group === draft.encoderGroup,
	);
	const occupied = new Set(
		placements.map(
			(placement) => `${placement.encoder_page}:${placement.encoder_slot}`,
		),
	);
	const maximumPage = Math.max(
		1,
		...placements.map((placement) => placement.encoder_page),
	);
	const options: Array<{ value: string; label: string }> = [];
	for (let page = 1; page <= maximumPage + 1; page += 1)
		for (let slot = 1; slot <= 6; slot += 1) {
			const value = `${page}:${slot}`;
			if (!occupied.has(value))
				options.push({ value, label: `Page ${page}, encoder ${slot}` });
		}
	return options;
}

function compatibleActivationGroups(
	snapshot: AttributeConfigurationSnapshot | null,
	draft: ImportedCustomAttributeDraft | null,
) {
	if (!snapshot || !draft || draft.valueType === "control") return [];
	const placements = new Map(
		snapshot.configuration.placements.map((placement) => [
			placement.attribute,
			placement,
		]),
	);
	return snapshot.configuration.activation_groups
		.filter((group) =>
			group.members.every(
				(member) =>
					placements.get(member)?.encoder_group === draft.encoderGroup,
			),
		)
		.map((group) => ({ value: group.id, label: group.label }));
}
