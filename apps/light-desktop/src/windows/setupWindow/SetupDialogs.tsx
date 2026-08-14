import { ModalRegistration, ModalTitleBar } from "@tosklight/ui";
import { useMemo, useState } from "react";
import { DeskLockSettingsModal } from "../../components/setup/DeskLockSettingsModal";
import { FixtureLibrarySetup } from "../../components/setup/FixtureLibrarySetup";
import { ProgrammerControlSurfaceSettings } from "../../components/setup/ScreensSetup";
import {
	fixtureDefinitionsFromProfiles,
	mergeFixtureDefinitions,
} from "../../components/setup/fixtureProfileModel";
import { useFixtureLibrary } from "../../features/fixtureLibrary/FixtureLibraryContext";
import type { SetupWindowController } from "./controller";

function FixtureLibraryDialog({ onClose }: { onClose: () => void }) {
	const library = useFixtureLibrary();
	const [query, setQuery] = useState("");
	const [typeFilter, setTypeFilter] = useState("");
	const [actionsTarget, setActionsTarget] = useState<HTMLDivElement | null>(
		null,
	);
	const fixtureTypes = useMemo(
		() =>
			[
				...new Set(
					mergeFixtureDefinitions(
						library?.fixtureProfiles ?? [],
						library?.fixtureLibrary ??
							fixtureDefinitionsFromProfiles(library?.fixtureProfiles ?? []),
					).map((item) => item.device_type || "other"),
				),
			].sort(),
		[library?.fixtureLibrary, library?.fixtureProfiles],
	);
	return (
		<ModalRegistration onClose={onClose}>
			<div
				className="stacked-modal-layer fixture-library-modal-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onClose()
				}
			>
				<section
					className="fixture-library-modal"
					role="dialog"
					aria-modal="true"
					aria-label="Fixture Library"
				>
					<ModalTitleBar
						title="Fixture Library"
						search={{
							value: query,
							onSearch: setQuery,
							ariaLabel: "Search fixture library",
							placeholder: "Search manufacturer, fixture, mode, or type",
							settingsConfiguration: [
								{
									kind: "select",
									id: "type",
									label: "Fixture type",
									value: typeFilter,
									options: [
										{ value: "", label: "All" },
										...fixtureTypes.map((type) => ({
											value: type,
											label: type,
										})),
									],
								},
							],
							onSettingChange: (_, value) => setTypeFilter(String(value)),
							onClearSettings: () => setTypeFilter(""),
						}}
						toolbar={
							<div ref={setActionsTarget} className="setup-section-actions" />
						}
						closeLabel="Close Fixture Library"
						onClose={onClose}
					/>
					<div className="fixture-library-modal-body">
						<FixtureLibrarySetup
							query={query}
							typeFilter={typeFilter}
							onQueryChange={setQuery}
							onTypeFilterChange={setTypeFilter}
							showToolbarSearch={false}
							toolbarActionsTarget={actionsTarget}
						/>
					</div>
				</section>
			</div>
		</ModalRegistration>
	);
}

function EncoderPlacementDialog({ onClose }: { onClose: () => void }) {
	return (
		<ModalRegistration onClose={onClose}>
			<div
				className="stacked-modal-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onClose()
				}
			>
				<section
					className="nested-modal"
					role="dialog"
					aria-modal="true"
					aria-label="Encoder placement"
				>
					<ModalTitleBar
						title="Encoder placement"
						closeLabel="Close encoder placement"
						onClose={onClose}
					/>
					<ProgrammerControlSurfaceSettings />
				</section>
			</div>
		</ModalRegistration>
	);
}

export function SetupDialogs({
	controller,
}: {
	controller: SetupWindowController;
}) {
	return (
		<>
			{controller.fixtureLibraryOpen && (
				<FixtureLibraryDialog
					onClose={() => controller.setFixtureLibraryOpen(false)}
				/>
			)}
			{controller.deskLockSettingsOpen && (
				<DeskLockSettingsModal
					onClose={() => controller.setDeskLockSettingsOpen(false)}
				/>
			)}
			{controller.encoderPlacementOpen && (
				<EncoderPlacementDialog
					onClose={() => controller.setEncoderPlacementOpen(false)}
				/>
			)}
		</>
	);
}
