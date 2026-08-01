import { ServerErrorNotice } from "../../components/shell/ServerErrorNotice";
import { PoolPaletteSettings } from "../../components/shared/PoolColorSettings";
import { ScreensSetup } from "../../components/setup/ScreensSetup";
import { WindowScrollArea } from "@tosklight/ui/window-kit";
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
		case 7:
			return (
				<section className="setup-section" aria-labelledby="preferences-title">
					<h2 id="preferences-title">Preferences</h2>
					<PoolPaletteSettings />
				</section>
			);
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
						<ScreensSetup
							undoRef={controller.screenUndo}
							onUndoAvailabilityChange={controller.updateScreenUndoAvailability}
						/>
					</div>
					<ServerErrorNotice />
				</div>
			</WindowScrollArea>
		</main>
	);
}
