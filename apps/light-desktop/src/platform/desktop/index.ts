import { browserDesktopBridge } from "./browserDesktopBridge";
import {
	type ControllableDesktopWindow,
	controllableBrowserDesktopBridge,
	injectedDesktopPort,
} from "./controllableBrowserDesktopBridge";
import { tauriDesktopBridge } from "./tauriDesktopBridge";

export { DesktopProvider, useDesktopBridge } from "./DesktopContext";
export type {
	ConsoleScreenWindow,
	DesktopBridge,
	DesktopDisplay,
	DesktopUnsubscribe,
	DesktopWindowState,
} from "./types";
export { useScreenWindowPersistence } from "./useScreenWindowPersistence";

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

export function desktopRuntimeAvailable(
	runtime: ControllableDesktopWindow | undefined = browserWindow(),
) {
	return Boolean(
		runtime && "__TAURI_INTERNALS__" in runtime && runtime.__TAURI_INTERNALS__,
	);
}

function browserWindow(): ControllableDesktopWindow | undefined {
	return typeof window === "undefined"
		? undefined
		: (window as ControllableDesktopWindow);
}
