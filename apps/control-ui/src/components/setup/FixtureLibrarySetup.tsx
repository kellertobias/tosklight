import { useState } from "react";
import { useServer } from "../../api/ServerContext";
import { FixtureLibraryBrowser } from "./fixtureLibrary/browser";
import {
	FixtureLibraryEditor,
	useFixtureLibraryEditor,
} from "./fixtureLibrary/editor";
import { useFixtureLibraryModel } from "./fixtureLibrary/model";
import {
	FixtureRevisionHistory,
	useFixtureRevisionHistory,
} from "./fixtureLibrary/revisions";
import { FixtureLibraryToolbar } from "./fixtureLibrary/toolbar";
import {
	FixtureImportDialogs,
	useFixtureLibraryTransfers,
} from "./fixtureLibrary/transfers";
import { FixtureLibraryWarnings } from "./fixtureLibrary/warnings";

export {
	blankDefinition,
	FIXTURE_TYPES,
	parseHeadDrafts,
} from "./fixtureLibrary/definitions";
export { importGdtf, importGdtfData } from "./fixtureLibrary/gdtf";

export function FixtureLibrarySetup() {
	const server = useServer();
	const [selectedFamilyKey, setSelectedFamilyKey] = useState("");
	const [selectedModeKey, setSelectedModeKey] = useState("");
	const [query, setQuery] = useState("");
	const [typeFilter, setTypeFilter] = useState("");
	const [manufacturer, setManufacturer] = useState("");
	const model = useFixtureLibraryModel({
		fixtureProfiles: server.fixtureProfiles,
		legacyDefinitions: server.fixtureLibrary,
		manufacturer,
		query,
		selectedFamilyKey,
		selectedModeKey,
		typeFilter,
	});
	const editor = useFixtureLibraryEditor(server.fixtureProfiles);
	const transfers = useFixtureLibraryTransfers({
		selectedMode: model.selectedMode,
		setSelectedFamilyKey,
		setSelectedModeKey,
	});
	const revisions = useFixtureRevisionHistory({
		selectedMode: model.selectedMode,
		onEditRevision: editor.openRevision,
	});

	return (
		<div className="fixture-library-setup">
			<FixtureLibraryToolbar
				fixtureTypes={model.fixtureTypes}
				query={query}
				typeFilter={typeFilter}
				onCreate={editor.openCreate}
				setImportModal={transfers.setModal}
				setQuery={setQuery}
				setTypeFilter={setTypeFilter}
			/>
			<FixtureLibraryWarnings />
			<FixtureLibraryBrowser
				libraryFamilies={model.libraryFamilies}
				manufacturer={manufacturer}
				manufacturers={model.manufacturers}
				selectedFamily={model.selectedFamily}
				selectedMode={model.selectedMode}
				onEdit={editor.openSelected}
				onExport={() => void transfers.exportSelectedPackage()}
				onRevisionHistory={() => void revisions.open()}
				setManufacturer={setManufacturer}
				setSelectedFamilyKey={setSelectedFamilyKey}
				setSelectedModeKey={setSelectedModeKey}
			/>
			<FixtureImportDialogs
				busy={transfers.busy}
				modal={transfers.modal}
				close={() => transfers.setModal(null)}
				importGdtfFile={transfers.importGdtfFile}
				importPackage={transfers.importPackage}
			/>
			{revisions.history && (
				<FixtureRevisionHistory
					history={revisions.history}
					error={revisions.error}
					onClose={revisions.close}
					onDelete={revisions.deleteRevision}
					onEdit={revisions.editRevision}
				/>
			)}
			{editor.editor && (
				<FixtureLibraryEditor
					editor={editor.editor}
					manufacturers={model.manufacturers}
					onClose={editor.close}
				/>
			)}
		</div>
	);
}
