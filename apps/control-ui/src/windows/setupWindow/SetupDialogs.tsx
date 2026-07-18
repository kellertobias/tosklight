import { ModalTitleBar } from "../../components/common";
import { DeskLockSettingsModal } from "../../components/setup/DeskLockSettingsModal";
import { FixtureLibrarySetup } from "../../components/setup/FixtureLibrarySetup";
import type { SetupWindowController } from "./controller";

function FixtureLibraryDialog({ onClose }: { onClose: () => void }) {
	return (
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
					search={
						<div id="setup-section-search" className="setup-section-search" />
					}
					actions={
						<div id="setup-section-actions" className="setup-section-actions" />
					}
					closeLabel="Close Fixture Library"
					onClose={onClose}
				/>
				<div className="fixture-library-modal-body">
					<FixtureLibrarySetup />
				</div>
			</section>
		</div>
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
		</>
	);
}
