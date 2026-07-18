import { browserDesktopBridge } from "./browserDesktopBridge";
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

export function createDesktopBridge() {
	if (typeof window === "undefined") return browserDesktopBridge;
	return "__TAURI_INTERNALS__" in window
		? tauriDesktopBridge
		: browserDesktopBridge;
}
