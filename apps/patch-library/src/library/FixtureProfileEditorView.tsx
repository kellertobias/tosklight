import { Fragment } from "react";
import type {
	AttributeDescriptor,
	FixtureBodyModel,
	FixtureProfile,
} from "../wire";
import { Button, ModalRegistration, ModalTitleBar } from "@tosklight/ui";
import { liftMotionAttributes } from "../sheet/fixtureProfileModel";
import {
	EditorBreadcrumbs,
	EditorTrailProvider,
	useEditorTrail,
} from "./breadcrumbs";
import { ConfirmDialog, ManufacturerLookup } from "./dialogs";
import {
	FixtureProfileEditorPortsProvider,
	type FixtureProfileEditorPorts,
} from "./ports";
import {
	IdentityProfileTab,
	SimulationProfileTab,
} from "./genericProfileTab";
import { ModeEditor } from "./modeEditor";
import { ModesTab } from "./modesTab";
import { GeometryEditor } from "./geometryEditor";
import {
	type ProfileEditorTab,
	useFixtureProfileEditorController,
} from "./useFixtureProfileEditorController";

export type FixtureProfileEditorProps = {
	initialProfile: FixtureProfile;
	expectedRevision?: number;
	manufacturers: string[];
	attributeRegistry?: AttributeDescriptor[];
	/** The generic bodies this build ships, which the Simulation tab offers. */
	bodyCatalogue?: FixtureBodyModel[];
	onSave: (
		profile: FixtureProfile,
		expectedRevision: number,
	) => Promise<FixtureProfile>;
	onClose: () => void;
	/** Host capabilities the editor cannot own: Stage geometry preview and asset picking. */
	ports: FixtureProfileEditorPorts;
};

type EditorController = ReturnType<typeof useFixtureProfileEditorController>;

const EDITOR_TABS: { id: ProfileEditorTab; label: string }[] = [
	{ id: "identity", label: "Identity" },
	{ id: "simulation", label: "Simulation" },
	{ id: "geometry", label: "Geometry" },
	{ id: "modes", label: "Modes" },
];

function ProfileEditorBody({
	editor,
	attributeRegistry,
	bodyCatalogue,
}: {
	editor: EditorController;
	attributeRegistry: AttributeDescriptor[];
	bodyCatalogue: FixtureBodyModel[];
}) {
	return (
		<div className="fixture-profile-editor-body">
			<datalist id="fixture-attribute-registry">
				{attributeRegistry.map((descriptor) => (
					<option
						key={descriptor.id}
						value={descriptor.id}
						data-family={descriptor.family}
						data-value-type={descriptor.value_type}
						data-default-unit={descriptor.default_unit ?? ""}
					>
						{descriptor.family} · {descriptor.label}
					</option>
				))}
			</datalist>
			{editor.localErrors.length > 0 && (
				<section className="fixture-profile-errors" role="alert">
					<strong>Fixture profile needs attention</strong>
					<ul>
						{editor.localErrors.map((error) => (
							<li key={error}>{error}</li>
						))}
					</ul>
				</section>
			)}
			{editor.tab === "identity" && (
				<IdentityProfileTab
					draft={editor.draft}
					onChange={editor.setDraft}
					onLookup={() => {
						editor.setLookupQuery("");
						editor.setLookup(true);
					}}
				/>
			)}
			{editor.tab === "simulation" && (
				<SimulationProfileTab
					draft={editor.draft}
					onChange={editor.setDraft}
					bodyCatalogue={bodyCatalogue}
				/>
			)}
			{editor.tab === "geometry" && (
				<GeometryEditor
					mode={{
						...editor.draft.modes[0],
						geometry: editor.draft.geometry ?? { nodes: [], emitters: [] },
					}}
					onChange={(carrier) =>
						// A template names the attribute on its moving parts; every mode takes it over.
						editor.setDraft((current) =>
							liftMotionAttributes({ ...current, geometry: carrier.geometry }),
						)
					}
				/>
			)}
			{editor.tab === "modes" && (
				<ModesTab
					draft={editor.draft}
					onChange={editor.updateMode}
					onMove={editor.moveMode}
					onDelete={editor.requestDeleteMode}
					onEdit={editor.openMode}
				/>
			)}
		</div>
	);
}

function EditorDialogs({
	editor,
	initialProfile,
	manufacturers,
	onClose,
}: {
	editor: EditorController;
	initialProfile: FixtureProfile;
	manufacturers: string[];
	onClose: () => void;
}) {
	return (
		<>
			{editor.lookup && (
				<ManufacturerLookup
					manufacturers={manufacturers}
					query={editor.lookupQuery}
					onQuery={editor.setLookupQuery}
					onSelect={(manufacturer) => {
						editor.setDraft({ ...editor.draft, manufacturer });
						editor.setLookup(false);
					}}
					onClose={() => editor.setLookup(false)}
				/>
			)}
			{editor.modePendingDelete && (
				<ConfirmDialog
					title={`Remove ${editor.modePendingDelete.name || "this mode"}?`}
					description="Its heads, channels, and functions are removed from this fixture. Nothing is saved until you save the fixture."
					primary="Remove mode"
					danger
					onPrimary={() =>
						editor.modePendingDelete &&
						editor.deleteMode(editor.modePendingDelete.id)
					}
					secondary="Keep mode"
					onSecondary={editor.cancelDeleteMode}
				/>
			)}
			{editor.closeConfirm && (
				<ConfirmDialog
					title="Discard fixture changes?"
					description="This fixture profile has unsaved changes."
					primary="Discard changes"
					danger
					onPrimary={onClose}
					secondary="Stay"
					onSecondary={() => editor.setCloseConfirm(false)}
				/>
			)}
			{editor.revisionConfirm && (
				<ConfirmDialog
					title="Create a new fixture revision?"
					description={`Revision ${initialProfile.revision} remains unchanged. The complete fixture profile, including every mode, will be saved as a new atomic revision.`}
					primary="Save and create revision"
					onPrimary={() => void editor.saveNow()}
					secondary="Keep editing"
					onSecondary={() => editor.setRevisionConfirm(false)}
				/>
			)}
		</>
	);
}

export function FixtureProfileEditor({
	initialProfile,
	expectedRevision = initialProfile.revision,
	manufacturers,
	attributeRegistry = [],
	bodyCatalogue = [],
	onSave,
	onClose,
	ports,
}: FixtureProfileEditorProps) {
	const editor = useFixtureProfileEditorController({
		initialProfile,
		expectedRevision,
		onSave,
		onClose,
	});
	const fixtureLabel =
		[editor.draft.manufacturer, editor.draft.name].filter(Boolean).join(" ") ||
		"New fixture";
	const tabLabel =
		EDITOR_TABS.find(({ id }) => id === editor.tab)?.label ?? editor.tab;
	const trail = useEditorTrail([fixtureLabel, tabLabel]);
	return (
		<FixtureProfileEditorPortsProvider ports={ports}>
		<ModalRegistration onClose={editor.requestClose}>
			<div
				className="stacked-modal-layer fixture-profile-editor-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && editor.requestClose()
				}
			>
				<section
					className="nested-modal fixture-profile-editor-modal"
					role="dialog"
					aria-modal="true"
					aria-label={
						initialProfile.revision
							? "Edit fixture profile"
							: "Create fixture profile"
					}
				>
					<ModalTitleBar
						title={
							initialProfile.revision
								? `Edit ${initialProfile.manufacturer} ${initialProfile.name}`
								: "Create fixture"
						}
						details={<EditorBreadcrumbs trail={trail} />}
						groups={[
							// Tab actions sit left of the tabs: the bar is right-aligned, so a button that
							// appears for one tab would otherwise push every tab sideways when it does.
							...(editor.tab === "modes"
								? [
										{
											id: "mode",
											actions: [
												{
													id: "add-mode",
													label: "Add mode",
													onPress: editor.addMode,
												},
											],
										},
									]
								: []),
							{
								id: "editor-tabs",
								kind: "tabs",
								activeId: editor.tab,
								onActiveChange: (id) => editor.setTab(id as ProfileEditorTab),
								actions: EDITOR_TABS.map(({ id, label }) => ({ id, label })),
							},
						]}
						accept={{
							id: "save",
							label: "Save fixture",
							variant: "primary",
							loading: editor.busy,
							onPress: editor.requestSave,
						}}
						closeLabel="Close fixture editor"
						onClose={editor.requestClose}
					/>
					<ProfileEditorBody
						editor={editor}
						attributeRegistry={attributeRegistry}
						bodyCatalogue={bodyCatalogue}
					/>
				</section>
				{editor.editedMode && (
					<EditorTrailProvider trail={trail}>
					<ModeEditor
						mode={editor.editedMode}
						geometry={editor.draft.geometry ?? { nodes: [], emitters: [] }}
						tab={editor.modeTab}
						attributeRegistry={attributeRegistry}
						openSplit={editor.openSplit}
						onTabChange={editor.setModeTab}
						onOpenSplit={editor.setOpenSplit}
						onChange={editor.updateMode}
						onClose={editor.closeMode}
					/>
					</EditorTrailProvider>
				)}
				<EditorDialogs
					editor={editor}
					initialProfile={initialProfile}
					manufacturers={manufacturers}
					onClose={onClose}
				/>
			</div>
		</ModalRegistration>
		</FixtureProfileEditorPortsProvider>
	);
}
