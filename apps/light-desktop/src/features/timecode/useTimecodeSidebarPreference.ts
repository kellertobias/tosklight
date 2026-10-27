import { createContext, useContext, useState } from "react";
import { useDeskConnection } from "../deskConnection/DeskConnectionContext";

export function timecodeSidebarStorageKey(scope: string, paneId: string) {
	return `tosklight.timecode-selection-sidebar.v1:${scope}:${paneId}`;
}
function read(key: string): boolean {
	try { return localStorage.getItem(key) === "true"; } catch { return false; }
}
/** Local desk/window presentation, deliberately separate from the portable Timecode. */
export const TimecodePaneScope = createContext("built-in");
export const TimelineInspectorPreferenceContext = createContext({
	visible: false, setVisible: (_visible: boolean) => {},
});
export function useTimelineInspectorPreference() { return useContext(TimelineInspectorPreferenceContext); }
export function useTimecodeSidebarPreference() {
	const paneId = useContext(TimecodePaneScope);
	const scope = useDeskConnection()?.deskLayoutScope ?? "local";
	const key = timecodeSidebarStorageKey(scope, paneId);
	const [stored, setStored] = useState(() => ({ key, value: read(key) }));
	const value = stored.key === key ? stored.value : read(key);
	return [value, (next: boolean) => {
		try { localStorage.setItem(key, String(next)); } catch { /* The local window still updates. */ }
		setStored({ key, value: next });
	}] as const;
}
