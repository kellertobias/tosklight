import { browserDesktopBridge } from "./browserDesktopBridge";
import {
	controllableBrowserDesktopBridge,
	injectedDesktopPort,
	type ControllableDesktopWindow,
} from "./controllableBrowserDesktopBridge";
import { tauriDesktopBridge } from "./tauriDesktopBridge";

export { DesktopProvider, useDesktopBridge } from "./DesktopContext";
export { useScreenWindowPersistence } from "./useScreenWindowPersistence";
export type {
	ConsoleScreenWindow,
	DesktopBridge,
	DesktopDisplay,
	DesktopUnsubscribe,
	DesktopWindowState,
} from "./types";

export function createDesktopBridge(
	runtime: ControllableDesktopWindow | undefined = browserWindow(),
) {
	if (!runtime) return browserDesktopBridge;
	const injected = injectedDesktopPort(runtime);
	if (injected) return controllableBrowserDesktopBridge(injected);
	return "__TAURI_INTERNALS__" in runtime
		? tauriDesktopBridge
		: browserDesktopBridge;
}

function browserWindow(): ControllableDesktopWindow | undefined {
	return typeof window === "undefined"
		? undefined
		: (window as ControllableDesktopWindow);
}
