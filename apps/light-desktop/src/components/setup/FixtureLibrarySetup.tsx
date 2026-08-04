import { useEffect, useState } from "react";
import { useFixtureLibrary } from "../../features/fixtureLibrary/FixtureLibraryContext";
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

export function FixtureLibrarySetup({
	query: controlledQuery,
	typeFilter: controlledTypeFilter,
	onQueryChange,
	onTypeFilterChange,
	showToolbarSearch = true,
	toolbarActionsTarget,
}: {
	query?: string;
	typeFilter?: string;
	onQueryChange?: (query: string) => void;
	onTypeFilterChange?: (type: string) => void;
	showToolbarSearch?: boolean;
	toolbarActionsTarget?: HTMLElement | null;
} = {}) {
	const server = useFixtureLibrary();
	const [selectedFamilyKey, setSelectedFamilyKey] = useState("");
	const [selectedModeKey, setSelectedModeKey] = useState("");
	const [localQuery, setLocalQuery] = useState("");
	const [localTypeFilter, setLocalTypeFilter] = useState("");
	const [manufacturer, setManufacturer] = useState("");
	const query = controlledQuery ?? localQuery;
	const typeFilter = controlledTypeFilter ?? localTypeFilter;
	const setQuery = onQueryChange ?? setLocalQuery;
	const setTypeFilter = onTypeFilterChange ?? setLocalTypeFilter;
	const model = useFixtureLibraryModel({
		fixtureProfiles: server?.fixtureProfiles ?? [],
		legacyDefinitions: server?.fixtureLibrary ?? [],
		manufacturer,
		query,
		selectedFamilyKey,
		selectedModeKey,
		typeFilter,
	});
	useEffect(() => {
		if (manufacturer && !model.manufacturers.includes(manufacturer))
			setManufacturer("");
	}, [manufacturer, model.manufacturers]);
	const editor = useFixtureLibraryEditor(server?.fixtureProfiles ?? []);
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
				actionsTarget={toolbarActionsTarget}
				fixtureTypes={model.fixtureTypes}
				query={query}
				showSearch={showToolbarSearch}
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
				error={transfers.error}
				modal={transfers.modal}
				close={() => transfers.setModal(null)}
				confirmGdtfMappings={transfers.confirmGdtfMappings}
				confirmPackageMappings={transfers.confirmPackageMappings}
				importGdtfFile={transfers.importGdtfFile}
				importPackage={transfers.importPackage}
				mappingCandidates={transfers.mappingCandidates}
				mappings={transfers.mappings}
				requirements={transfers.requirements}
				setMapping={transfers.setMapping}
				activationGroupOptions={transfers.activationGroupOptions}
				beginCustomAttribute={transfers.beginCustomAttribute}
				cancelCustomAttribute={transfers.cancelCustomAttribute}
				createCustomAttribute={transfers.createCustomAttribute}
				customAttributeDraft={transfers.customAttributeDraft}
				editCustomAttribute={transfers.editCustomAttribute}
				placementOptions={transfers.placementOptions}
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
