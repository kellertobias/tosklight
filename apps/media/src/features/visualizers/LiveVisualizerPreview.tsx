// The selected visualizer as it looks right now.
//
// Frames are drawn by the Media Server from the stored parameters and the room it hears, so an
// edit shows on the next frame without saving anything by hand. The next frame is asked for once
// the last one has arrived, which paces the stream to what the server and the network manage. A
// server that cannot draw one falls back to the shipped picture of the kind and tries again later.

import { useEffect, useRef, useState } from "react";
import { api } from "../../shared/api/client";
import type { VisualizerView } from "../../shared/api/generated/media-wire";
import { visualizerPreviewUrl } from "./preview";

/** At most this many frames a second, so an open editor never floods the server. */
const FRAME_INTERVAL_MS = 1000 / 15;
/** How long a failed stream waits before trying again. */
const RETRY_MS = 5000;
const PREVIEW_WIDTH = 480;

export function LiveVisualizerPreview({
	visualizer,
	aspectRatio,
}: {
	visualizer: VisualizerView;
	aspectRatio: number;
}) {
	const { folder, file } = visualizer.address;
	const [frame, setFrame] = useState(0);
	const [live, setLive] = useState(true);
	const size = {
		width: PREVIEW_WIDTH,
		height: Math.max(1, Math.round(PREVIEW_WIDTH / (aspectRatio || 16 / 9))),
	};

	// biome-ignore lint/correctness/useExhaustiveDependencies: a different visualizer starts a fresh stream.
	useEffect(() => {
		setLive(true);
		setFrame(0);
	}, [folder, file]);

	useEffect(() => {
		if (live) return;
		const retry = window.setTimeout(() => setLive(true), RETRY_MS);
		return () => window.clearTimeout(retry);
	}, [live]);

	const timer = useRef<number | undefined>(undefined);
	useEffect(() => () => window.clearTimeout(timer.current), []);
	const next = () => {
		timer.current = window.setTimeout(() => {
			// A hidden page asks for nothing until it is looked at again.
			if (document.visibilityState === "hidden") {
				next();
				return;
			}
			setFrame((current) => current + 1);
		}, FRAME_INTERVAL_MS);
	};

	return live ? (
		<img
			className="media-live-visualizer-preview"
			src={api.visualizerPreviewUrl(folder, file, frame, size)}
			alt=""
			data-live="true"
			onLoad={next}
			onError={() => setLive(false)}
		/>
	) : (
		<img src={visualizerPreviewUrl(visualizer)} alt="" data-live="false" />
	);
}
