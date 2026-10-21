/** Offline-ready service projections for reproducible Setup documentation; never contacts a desk. */
import { type PropsWithChildren, useMemo } from "react";
import type { ExtensionRuntimeSnapshot } from "../../../light-desktop/src/api/client/deskManagement";
import type { SessionResponse } from "../../../light-desktop/src/api/types";
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

const extensionSnapshot: ExtensionRuntimeSnapshot = {
	extensions_directory: "Extensions",
	configuration_path: "Extensions/extensions.json",
	configuration_diagnostic: null,
	packages: [],
	instances: [],
	instance_diagnostics: [],
};

export function SetupHelpReady({
	children,
	extensionStatus = extensionSnapshot,
	session = null,
}: PropsWithChildren<{
	extensionStatus?: ExtensionRuntimeSnapshot;
	session?: SessionResponse | null;
}>) {
	const storyDesk = useMemo(() => {
		const store = new DeskSnapshotStore();
		store.install(desk.getSnapshot().bootstrap, session);
		return store;
	}, [session]);
	const storyExtensions = useMemo(
		() => ({
			load: async () => extensionStatus,
			rescan: async () => extensionStatus,
		}),
		[extensionStatus],
	);
	return (
		<DeskSnapshotStateProvider store={storyDesk}>
			<ShellStatusStateProvider store={shell}>
				<ExtensionRuntimeActionsProvider actions={storyExtensions}>
					{children}
				</ExtensionRuntimeActionsProvider>
			</ShellStatusStateProvider>
		</DeskSnapshotStateProvider>
	);
}
