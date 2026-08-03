import { WindowScrollArea } from "@tosklight/ui/window-kit";
import { ScreensSetup } from "../../components/setup/ScreensSetup";
import { ServerErrorNotice } from "../../components/shell/ServerErrorNotice";
import type { SetupWindowController } from "./controller";
import { ShowsRecoverySection, TimecodeSection } from "./GeneralSections";
import { NetworkSection } from "./NetworkSection";
import { OutputsSection } from "./OutputsSection";
import {
	AttributesEncodersSection,
	DefaultsSection,
	HighlightSection,
	OthersSection,
} from "./ProgrammerSection";

function ActiveSetupSection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	switch (controller.section) {
		case "shows":
			return <ShowsRecoverySection controller={controller} />;
		case "outputs":
			return <OutputsSection controller={controller} />;
		case "timecode":
			return <TimecodeSection controller={controller} />;
		case "network":
			return <NetworkSection controller={controller} />;
		case "preferences-defaults":
			return <DefaultsSection controller={controller} />;
		case "preferences-attributes":
			return <AttributesEncodersSection controller={controller} />;
		case "preferences-highlight":
			return <HighlightSection controller={controller} />;
		case "preferences-others":
			return <OthersSection controller={controller} />;
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
					<div hidden={controller.section !== "screens"}>
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
