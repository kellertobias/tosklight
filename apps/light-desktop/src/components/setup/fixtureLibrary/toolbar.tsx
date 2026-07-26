import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { Button } from "@tosklight/ui";
import { SearchBar } from "@tosklight/ui";
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
		// The Fixture Library title bar exposes one actions surface that carries both the shared
		// search and the neighbouring Import/Create actions (MANUAL-019), so the search portals into
		// the actions target rather than a separate search slot.
		setSearchTarget(document.getElementById("setup-section-actions"));
		setActionsTarget(document.getElementById("setup-section-actions"));
	}, []);

	return (
		<>
			{searchTarget &&
				createPortal(
					<SearchBar
						value={query}
						onChange={setQuery}
						settings={[{
							kind: "select",
							id: "type",
							label: "Fixture type",
							value: typeFilter,
							options: [
								{ value: "", label: "All" },
								...fixtureTypes.map((type) => ({ value: type, label: type })),
							],
						}]}
						onSettingChange={(_, value) => setTypeFilter(String(value))}
						onClearSettings={() => setTypeFilter("")}
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
