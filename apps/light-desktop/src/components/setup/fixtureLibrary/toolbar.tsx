import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { Button } from "@tosklight/ui";
import { SearchBar } from "@tosklight/ui";
import type { FixtureImportModal } from "./transfers";

interface FixtureLibraryToolbarProps {
	actionsTarget?: HTMLElement | null;
	fixtureTypes: string[];
	query: string;
	showSearch?: boolean;
	typeFilter: string;
	onCreate: () => void;
	setImportModal: (modal: FixtureImportModal) => void;
	setQuery: (query: string) => void;
	setTypeFilter: (type: string) => void;
}

export function FixtureLibraryToolbar({
	actionsTarget: providedActionsTarget,
	fixtureTypes,
	query,
	showSearch = true,
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
			showSearch ? document.getElementById("setup-section-actions") : null,
		);
		setActionsTarget(
			providedActionsTarget === undefined
				? document.getElementById("setup-section-actions")
				: providedActionsTarget,
		);
	}, [providedActionsTarget, showSearch]);

	return (
		<>
			{searchTarget &&
				createPortal(
					<SearchBar
						ariaLabel="Search fixture library"
						value={query}
						onChange={setQuery}
						settings={[
							{
								kind: "select",
								id: "type",
								label: "Fixture type",
								value: typeFilter,
								options: [
									{ value: "", label: "All" },
									...fixtureTypes.map((type) => ({ value: type, label: type })),
								],
							},
						]}
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
