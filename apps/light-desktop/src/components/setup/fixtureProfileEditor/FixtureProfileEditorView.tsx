import { Fragment } from "react";
import type { AttributeDescriptor, FixtureProfile } from "../../../api/types";
import { Button, ModalTitleBar } from "../../common";
import { ConfirmDialog, ManufacturerLookup } from "./dialogs";
import { GenericProfileTab } from "./genericProfileTab";
import { ModeEditor } from "./modeEditor";
import { ModesTab } from "./modesTab";
import {
	type ProfileEditorTab,
	useFixtureProfileEditorController,
} from "./useFixtureProfileEditorController";

export type FixtureProfileEditorProps = {
	initialProfile: FixtureProfile;
	expectedRevision?: number;
	manufacturers: string[];
	attributeRegistry?: AttributeDescriptor[];
	onSave: (
		profile: FixtureProfile,
		expectedRevision: number,
	) => Promise<FixtureProfile>;
	onClose: () => void;
};

type EditorController = ReturnType<typeof useFixtureProfileEditorController>;

function ProfileEditorBody({
	editor,
	attributeRegistry,
}: {
	editor: EditorController;
	attributeRegistry: AttributeDescriptor[];
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
			{editor.tab === "generic" && (
				<GenericProfileTab
					draft={editor.draft}
					onChange={editor.setDraft}
					onLookup={() => {
						editor.setLookupQuery("");
						editor.setLookup(true);
					}}
				/>
			)}
			{editor.tab === "modes" && (
				<ModesTab
					draft={editor.draft}
					onChange={editor.updateMode}
					onMove={editor.moveMode}
					onDelete={editor.deleteMode}
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
	onSave,
	onClose,
}: FixtureProfileEditorProps) {
	const editor = useFixtureProfileEditorController({
		initialProfile,
		expectedRevision,
		onSave,
		onClose,
	});
	return (
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
					tabs={[
						{ id: "generic", label: "Generic" },
						{ id: "modes", label: "Modes" },
					]}
					activeTab={editor.tab}
					onTabChange={(id) => editor.setTab(id as ProfileEditorTab)}
					actions={
						<Fragment>
							{editor.tab === "modes" && (
								<Button onClick={editor.addMode}>Add mode</Button>
							)}
							<Button
								variant="primary"
								loading={editor.busy}
								onClick={editor.requestSave}
							>
								Save fixture
							</Button>
						</Fragment>
					}
					closeLabel="Close fixture editor"
					onClose={editor.requestClose}
				/>
				<ProfileEditorBody
					editor={editor}
					attributeRegistry={attributeRegistry}
				/>
			</section>
			{editor.editedMode && (
				<ModeEditor
					mode={editor.editedMode}
					tab={editor.modeTab}
					attributeRegistry={attributeRegistry}
					openSplit={editor.openSplit}
					onTabChange={editor.setModeTab}
					onOpenSplit={editor.setOpenSplit}
					onChange={editor.updateMode}
					onClose={editor.closeMode}
				/>
			)}
			<EditorDialogs
				editor={editor}
				initialProfile={initialProfile}
				manufacturers={manufacturers}
				onClose={onClose}
			/>
		</div>
	);
}
