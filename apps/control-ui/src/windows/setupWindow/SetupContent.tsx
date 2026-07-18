import { ScreensSetup } from "../../components/setup/ScreensSetup";
import { WindowScrollArea } from "../../components/window-kit";
import type { SetupWindowController } from "./controller";
import {
	ShowsRecoverySection,
	TimecodeSection,
	UsersSessionsSection,
} from "./GeneralSections";
import { NetworkSection } from "./NetworkSection";
import { OutputsSection } from "./OutputsSection";
import { ProgrammerSection } from "./ProgrammerSection";

function ActiveSetupSection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	switch (controller.section) {
		case 0:
			return <ShowsRecoverySection controller={controller} />;
		case 1:
			return <UsersSessionsSection controller={controller} />;
		case 2:
			return <ProgrammerSection controller={controller} />;
		case 3:
			return <OutputsSection controller={controller} />;
		case 4:
			return <TimecodeSection controller={controller} />;
		case 5:
			return <NetworkSection controller={controller} />;
		default:
			return null;
	}
}

export function SetupContent({
	controller,
}: {
	controller: SetupWindowController;
}) {
	return (
		<main>
			<WindowScrollArea className="setup-content-scroll">
				<div className="setup-content">
					<ActiveSetupSection controller={controller} />
					<div hidden={controller.section !== 6}>
						<ScreensSetup />
					</div>
					{controller.server.error && (
						<p className="modal-error">{controller.server.error}</p>
					)}
				</div>
			</WindowScrollArea>
		</main>
	);
}
