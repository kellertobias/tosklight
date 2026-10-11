import { useScreensOptional } from "../../../features/screens/ScreensContext";

/** Identify this window from the same route that mounts ScreenApp. */
export function useCommandLinePlacement() {
	const screens = useScreensOptional();
	const screenId = new URLSearchParams(window.location.search).get("screen");
	const screen = screenId
		? screens?.screens?.screens.find((item) => item.id === screenId)
		: null;
	const ownerId = screens?.screens?.programmer_control_surface?.owner_screen_id;
	const owner = screens?.screens?.screens.find((item) => item.id === ownerId);
	return {
		surfaceId: screenId ? screen?.surface_id ?? null : "main",
		programmerAssignment: ownerId && ownerId !== screenId && owner
			? `Programmer is assigned to ${owner.name}`
			: undefined,
	};
}
