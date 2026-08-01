import { Button } from "@tosklight/ui";
import { WindowHeader } from "@tosklight/ui/window-kit";
import type { SetupWindowController } from "./controller";

export const SETUP_SECTIONS = [
	"Shows & recovery",
	"Users & sessions",
	"Programmer",
	"Outputs",
	"Timecode",
	"Network & Inputs",
	"Screens & playback",
	"Preferences",
];

export function SetupHeader({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const actions =
		controller.section === 7
			? []
			: controller.section === 6
				? [
						[
							{
								id: "undo",
								label: "Undo",
								disabled: !controller.screenCanUndo,
								onClick: () => controller.screenUndo.current?.(),
							},
							{
								id: "desk-lock",
								label: "Desk Lock",
								onClick: () => controller.setDeskLockSettingsOpen(true),
							},
						],
					]
				: [
						[
							{
								id: "save",
								label: "Save changes",
								disabled:
									!controller.draft ||
									(controller.section === 2 &&
										!controller.programmerSettingsLoaded),
								onClick: () => void controller.save(),
							},
						],
					];
	return (
		<WindowHeader
			title="Desk Setup"
			info={{
				primary: SETUP_SECTIONS[controller.section],
				secondary: controller.restartRequired ? "Restart required" : undefined,
			}}
			actions={actions}
		/>
	);
}

export function SetupNavigation({
	section,
	onSelect,
}: {
	section: number;
	onSelect: (section: number) => void;
}) {
	return (
		<nav>
			{SETUP_SECTIONS.map((name, index) => (
				<Button
					onClick={() => onSelect(index)}
					className={index === section ? "active" : ""}
					key={name}
				>
					{name}
				</Button>
			))}
		</nav>
	);
}
