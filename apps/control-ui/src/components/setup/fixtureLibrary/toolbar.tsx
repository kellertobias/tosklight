import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { Button } from "../../common";
import { SearchBar } from "../../common/SearchBar";
import type { FixtureImportModal } from "./transfers";

interface FixtureLibraryToolbarProps {
	fixtureTypes: string[];
	query: string;
	typeFilter: string;
	onCreate: () => void;
	setImportModal: (modal: FixtureImportModal) => void;
	setQuery: (query: string) => void;
	setTypeFilter: (type: string) => void;
}

export function FixtureLibraryToolbar({
	fixtureTypes,
	query,
	typeFilter,
	onCreate,
	setImportModal,
	setQuery,
	setTypeFilter,
}: FixtureLibraryToolbarProps) {
	const [searchTarget, setSearchTarget] = useState<HTMLElement | null>(null);
	const [actionsTarget, setActionsTarget] = useState<HTMLElement | null>(null);
	useEffect(() => {
		setSearchTarget(
			document.getElementById("setup-section-search") ??
				document.getElementById("setup-section-actions"),
		);
		setActionsTarget(document.getElementById("setup-section-actions"));
	}, []);

	return (
		<>
			{searchTarget &&
				createPortal(
					<SearchBar
						value={query}
						onChange={setQuery}
						filters={[
							{ id: "type", label: "Fixture type", options: fixtureTypes },
						]}
						values={{ type: typeFilter }}
						onFilterChange={(_, value) => setTypeFilter(value)}
						placeholder="Search manufacturer, fixture, mode, or type"
					/>,
					searchTarget,
				)}
			{actionsTarget &&
				createPortal(
					<div className="setup-section-action-group">
						<Button onClick={() => setImportModal("package")}>
							Import fixture
						</Button>
						<Button onClick={() => setImportModal("gdtf")}>Import GDTF</Button>
						<Button onClick={onCreate}>Create fixture</Button>
					</div>,
					actionsTarget,
				)}
		</>
	);
}
