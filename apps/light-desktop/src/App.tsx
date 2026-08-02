import { useEffect } from "react";
import { ServerRuntime } from "./api/ServerRuntime";
import { DeskLockOverlay } from "./components/modals/DeskLockOverlay";
import { QuitConfirmOverlay } from "./components/modals/QuitConfirmOverlay";
import { AppShell } from "./components/shell/AppShell";
import { GroupSettingsIntentHost } from "./features/controlSurfaceInteraction/GroupSettingsIntentHost";
import {
	SetInteractionProvider,
	useSetInteraction,
} from "./features/controlSurfaceInteraction/SetInteractionProvider";
import { useControlSurfaceTarget } from "./features/controlSurfaceInteraction/useControlSurfaceTarget";
import {
	useActiveShowId,
	useBootstrapSnapshot,
	useSessionSnapshot,
} from "./features/deskSnapshot/DeskSnapshotState";
import { PatchFeatureBoundary } from "./features/patch/PatchFeatureBoundary";
import { useConnectionStatus } from "./features/shellStatus/ShellStatusState";
import { useDesktopBridge } from "./platform/desktop";
import { AppProvider } from "./state/AppContext";
import { FileManagerPickerHost } from "./windows/FileManagerPickerHost";

function DeskSetInteractionBoundary({ children }: React.PropsWithChildren) {
	const showId = useActiveShowId();
	const deskId = useSessionSnapshot()?.desk.id ?? null;
	return (
		<SetInteractionProvider showId={showId} deskId={deskId}>
			<SetInteractionRouteTarget />
			<GroupSettingsIntentHost />
			{children}
		</SetInteractionProvider>
	);
}

function SetInteractionRouteTarget() {
	const interaction = useSetInteraction();
	useControlSurfaceTarget(
		interaction
			? {
					id: "desk-set-interaction",
					priority: 50,
					accepts: ({ type }) => type === "set" && interaction.ready,
					handle: (intent) => {
						if (intent.type === "set") void interaction.arm(intent.source);
					},
				}
			: null,
	);
	return null;
}

function DesktopReady() {
	const bootstrapReady = useBootstrapSnapshot() !== null;
	const connectionStatus = useConnectionStatus();
	const desktop = useDesktopBridge();
	useEffect(() => {
		if (
			connectionStatus !== "connected" ||
			!bootstrapReady ||
			!desktop.available
		)
			return;
		void desktop.frontendReady();
	}, [connectionStatus, bootstrapReady, desktop]);
	return null;
}

export function App() {
	return (
		<ServerRuntime>
			<DesktopReady />
			<AppProvider>
				<DeskSetInteractionBoundary>
					<PatchFeatureBoundary>
						<AppShell />
						<QuitConfirmOverlay />
						<FileManagerPickerHost />
					</PatchFeatureBoundary>
				</DeskSetInteractionBoundary>
			</AppProvider>
			<DeskLockOverlay />
		</ServerRuntime>
	);
}
