import { useEffect, useState } from "react";
import { HardwareCueRowsView } from "@tosklight/ui/playback";
import type { Cue } from "../../../api/types";

type HardwareCueRowsProps = {
	cues: Cue[];
	cueIndex: number;
	activatedAt?: string;
	compact: boolean;
	effectiveNextCueNumber?: string | null;
	effectiveNextIsLoaded?: boolean;
};

export function HardwareCueRows({
	cues,
	cueIndex,
	activatedAt,
	compact,
	effectiveNextCueNumber,
	effectiveNextIsLoaded,
}: HardwareCueRowsProps) {
	const [now, setNow] = useState(() => Date.now());
	const current = cues[cueIndex];
	useEffect(() => {
		setNow(Date.now());
		if (!current?.fade_millis || !activatedAt) return;
		const timer = window.setInterval(() => setNow(Date.now()), 50);
		return () => window.clearInterval(timer);
	}, [current?.fade_millis, cueIndex, activatedAt]);
	const elapsed = activatedAt
		? now - Date.parse(activatedAt)
		: Number.POSITIVE_INFINITY;
	const progress =
		current?.fade_millis && elapsed < current.fade_millis
			? elapsed / current.fade_millis
			: 0;
	const effectiveNextIndex =
		effectiveNextCueNumber == null
			? -1
			: cues.findIndex((cue) => cue.number === effectiveNextCueNumber);
	const effectiveNext =
		effectiveNextIndex < 0 ? undefined : cues[effectiveNextIndex];
	return (
		<HardwareCueRowsView
			previous={cueView(cues[cueIndex - 1])}
			current={cueView(current)}
			next={cueView(effectiveNext)}
			compact={compact}
			nextLoaded={effectiveNextIsLoaded}
			progress={progress}
		/>
	);
}

function cueView(cue: Cue | undefined) {
	return cue
		? {
				number: cue.number,
				name: cue.name,
				fadeMillis: cue.fade_millis,
			}
		: undefined;
}
