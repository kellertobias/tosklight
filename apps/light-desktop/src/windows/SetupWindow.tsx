import type { SetupWindowController } from "./setupWindow/controller";
import { useSetupWindowController } from "./setupWindow/controller";
import { SetupHeader, SetupNavigation } from "./setupWindow/SetupChrome";
import { SetupContent } from "./setupWindow/SetupContent";
import { SetupDialogs } from "./setupWindow/SetupDialogs";
import type { WindowProps } from "./windowTypes";

export function SetupWindow(_: WindowProps) {
	const controller = useSetupWindowController();
	return <SetupWindowView controller={controller} />;
}

export function SetupWindowView({
	controller,
}: {
	controller: SetupWindowController;
}) {
	return (
		<div className="setup-window">
			<SetupHeader controller={controller} />
			<div className="setup-window-body">
				<SetupNavigation
					section={controller.section}
					onSelect={controller.setSection}
				/>
				<SetupContent controller={controller} />
			</div>
			<SetupDialogs controller={controller} />
		</div>
	);
}
