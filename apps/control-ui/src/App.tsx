import { useConnectionStatus } from "./features/shellStatus/ShellStatusState";
import { AppProvider } from "./state/AppContext";
import { AppShell } from "./components/shell/AppShell";
import { QuitConfirmOverlay } from "./components/modals/QuitConfirmOverlay";
import { ServerProvider, useServer } from "./api/ServerContext";
import { useEffect } from "react";
import { DeskLockOverlay } from "./components/modals/DeskLockOverlay";
import { FileManagerPickerHost } from "./windows/FileManagerPickerHost";
import { useDesktopBridge } from "./platform/desktop";
import { PatchFeatureBoundary } from "./features/patch/PatchFeatureBoundary";

function DesktopReady() {
	const server = useServer();
	const connectionStatus = useConnectionStatus();
	const desktop = useDesktopBridge();
	useEffect(() => {
		if (
			connectionStatus !== "connected" ||
			!server.bootstrap ||
			!desktop.available
		)
			return;
		void desktop.frontendReady();
	}, [connectionStatus, server.bootstrap, desktop]);
	return null;
}

export function App() {
	return (
		<ServerProvider>
			<DesktopReady />
			<AppProvider>
				<PatchFeatureBoundary>
					<AppShell />
					<QuitConfirmOverlay />
					<FileManagerPickerHost />
				</PatchFeatureBoundary>
			</AppProvider>
			<DeskLockOverlay />
		</ServerProvider>
	);
}
