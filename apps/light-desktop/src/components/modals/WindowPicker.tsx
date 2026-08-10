import { Button, ModalRegistration } from "@tosklight/ui";
import { useMediaServers } from "../../features/mediaServers/MediaServersContext";
import { useApp } from "../../state/AppContext";
import type { BuiltInWindow } from "../../types";

export const windowChoices: Array<[BuiltInWindow, string]> = [
	["presets", "Preset pool"],
	["groups", "Group pool"],
	["fixtures", "Fixture sheet"],
	["stage", "Stage"],
	["cuelist_pool", "Cuelist Pool"],
	["cues", "Cues · Cuelist"],
	["cuelists", "Cuelists (tabs)"],
	["virtual_playbacks", "Virtual Playbacks"],
	["macros", "Macro Pool"],
	["media", "Media"],
	["running", "Running"],
	["timecode", "Timecode"],
	["file_manager", "File Manager"],
	["text_editor", "Text Editor"],
	["channels", "Channels"],
	["dynamics", "Dynamics"],
	["scheduler", "Scheduler"],
	["dmx", "DMX output"],
	["help", "Help"],
];
export const availableWindowChoices = (mediaAvailable: boolean) =>
	windowChoices.filter(([kind]) => kind !== "media" || mediaAvailable);
export function WindowPicker() {
	const { state, dispatch } = useApp();
	const mediaAvailable = (useMediaServers()?.mediaServers.length ?? 0) > 0;
	if (!state.windowPicker) return null;
	const close = () => dispatch({ type: "OPEN_WINDOW_PICKER", rect: null });
	return (
		<ModalRegistration onClose={close}>
			<div
				className="floating-dialog window-picker"
				role="dialog"
				aria-label="Open Window"
			>
				<h2>Open Window</h2>
				<div className="dialog-grid">
					{availableWindowChoices(mediaAvailable).map(([kind, label]) => (
						<Button
							key={kind}
							onClick={() => dispatch({ type: "ADD_WINDOW", kind })}
						>
							{label}
						</Button>
					))}
				</div>
				<Button className="dialog-done" onClick={close}>
					Cancel
				</Button>
			</div>
		</ModalRegistration>
	);
}
