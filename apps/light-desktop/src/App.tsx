import { useConnectionStatus } from "./features/shellStatus/ShellStatusState";
import { AppProvider } from "./state/AppContext";
import { AppShell } from "./components/shell/AppShell";
import { QuitConfirmOverlay } from "./components/modals/QuitConfirmOverlay";
import { ServerRuntime } from "./api/ServerRuntime";
import { useBootstrapSnapshot } from "./features/deskSnapshot/DeskSnapshotState";
import { useEffect } from "react";
import { DeskLockOverlay } from "./components/modals/DeskLockOverlay";
import { FileManagerPickerHost } from "./windows/FileManagerPickerHost";
import { useDesktopBridge } from "./platform/desktop";
import { PatchFeatureBoundary } from "./features/patch/PatchFeatureBoundary";

function DesktopReady() {
	const bootstrapReady = useBootstrapSnapshot() !== null;
	const connectionStatus = useConnectionStatus();
	const desktop = useDesktopBridge();
	useEffect(() => {
		if (connectionStatus !== "connected" || !bootstrapReady || !desktop.available)
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
				<PatchFeatureBoundary>
					<AppShell />
					<QuitConfirmOverlay />
					<FileManagerPickerHost />
				</PatchFeatureBoundary>
			</AppProvider>
			<DeskLockOverlay />
		</ServerRuntime>
	);
}
