import { useCallback, useRef, useState } from "react";
import type { RoutePath } from "../app/routes";

/** Keep the destination's manual controls unavailable until restoration is confirmed. */
export function usePreviewNavigation(stop: () => Promise<void>, navigate: (path: RoutePath) => void) {
	const pending = useRef(false);
	const [stopping, setStopping] = useState(false);
	const leave = useCallback(async (path: RoutePath) => {
		if (pending.current) return;
		pending.current = true;
		setStopping(true);
		try { await stop(); navigate(path); }
		catch { /* The preview failure remains visible on the current page. */ }
		finally { pending.current = false; setStopping(false); }
	}, [stop, navigate]);
	return { navigate: leave, stopping };
}
