/**
 * A viewport's camera while it is being panned or zoomed.
 *
 * The tile layout is CadApp state, and handing it every pointer move re-rendered the whole CAD and
 * rewrote the stored workspace once per move, so panning lagged and the grid and drawing landed a
 * frame apart. The camera in flight lives here instead and the viewport redraws from it alone; the
 * layout hears of it once, when the gesture ends or the wheel has settled.
 */
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { TileCamera } from "./types";

/** How long the wheel must rest before a zoom is handed to the layout. */
export const CAMERA_SETTLE_MILLISECONDS = 150;

interface LiveCamera {
	/** The committed camera the gesture started from; a different one means it was replaced. */
	base: TileCamera;
	camera: TileCamera;
}

export function useLiveCamera(
	camera: TileCamera,
	onCamera: (camera: TileCamera) => void,
) {
	const [live, setLive] = useState<LiveCamera | null>(null);
	const liveRef = useRef<LiveCamera | null>(null);
	const committed = useRef({ camera, onCamera });
	const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
	useLayoutEffect(() => {
		committed.current = { camera, onCamera };
	});

	// A camera chosen elsewhere — Fit, a view change, a shortcut — replaces the one in flight.
	const valid = (value: LiveCamera | null) =>
		value && value.base === committed.current.camera ? value : null;

	function stopTimer() {
		clearTimeout(timer.current);
		timer.current = undefined;
	}

	function show(next: TileCamera) {
		const value = { base: valid(liveRef.current)?.base ?? camera, camera: next };
		liveRef.current = value;
		setLive(value);
	}

	function commit() {
		stopTimer();
		const value = valid(liveRef.current);
		liveRef.current = null;
		setLive(null);
		if (value) committed.current.onCamera(value.camera);
	}

	function settle(next: TileCamera) {
		show(next);
		stopTimer();
		timer.current = setTimeout(commit, CAMERA_SETTLE_MILLISECONDS);
	}

	useEffect(
		() => () => {
			stopTimer();
			const value = valid(liveRef.current);
			if (value) committed.current.onCamera(value.camera);
		},
		[],
	);

	return {
		camera: live && live.base === camera ? live.camera : camera,
		/** The camera the next wheel step starts from, even before React has rendered the last. */
		latest: () => valid(liveRef.current)?.camera ?? committed.current.camera,
		show,
		commit,
		settle,
	};
}
