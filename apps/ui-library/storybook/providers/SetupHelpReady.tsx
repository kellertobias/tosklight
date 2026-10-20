/** Offline-ready service projections for reproducible Setup documentation; never contacts a desk. */
import type { PropsWithChildren } from "react";
import { DeskSnapshotStateProvider } from "../../../light-desktop/src/features/deskSnapshot/DeskSnapshotState";
import { DeskSnapshotStore } from "../../../light-desktop/src/features/deskSnapshot/store";
import { ExtensionRuntimeActionsProvider } from "../../../light-desktop/src/features/extensions/ExtensionRuntimeActions";
import { ShellStatusStateProvider } from "../../../light-desktop/src/features/shellStatus/ShellStatusState";
import { ShellStatusStore } from "../../../light-desktop/src/features/shellStatus/store";
import { dmxOutputHealth } from "../fixtures/dmx";

const desk = new DeskSnapshotStore();
desk.install(
	{
		api_version: "v2",
		attribute_registry: [],
		desk: null,
		clients: [],
		active_show: {
			id: "storybook-show",
			name: "Opening Night",
			path: "Shows/Opening Night.show",
			revision: 8,
			updated_at: "2026-07-26T10:00:00Z",
		},
		active_programmers: [],
		frame_rate_hz: 44,
		output_health: dmxOutputHealth,
		active_timecode_source: null,
		active_timecode: null,
		active_show_error: null,
		hardware_connected: false,
	},
	null,
);

const shell = new ShellStatusStore();
shell.install("connected", null);

const extensionSnapshot = {
	extensions_directory: "Extensions",
	configuration_path: "Extensions/extensions.json",
	configuration_diagnostic: null,
	packages: [],
	instances: [],
	instance_diagnostics: [],
};
const extensions = {
	load: async () => extensionSnapshot,
	rescan: async () => extensionSnapshot,
};

export function SetupHelpReady({ children }: PropsWithChildren) {
	return (
		<DeskSnapshotStateProvider store={desk}>
			<ShellStatusStateProvider store={shell}>
				<ExtensionRuntimeActionsProvider actions={extensions}>
					{children}
				</ExtensionRuntimeActionsProvider>
			</ShellStatusStateProvider>
		</DeskSnapshotStateProvider>
	);
}
