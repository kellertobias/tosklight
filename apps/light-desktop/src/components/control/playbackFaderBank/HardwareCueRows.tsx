import { type CSSProperties, useEffect, useState } from "react";
import type { Cue } from "../../../api/types";

type HardwareCueRowsProps = {
	cues: Cue[];
	cueIndex: number;
	activatedAt?: string;
	compact: boolean;
	effectiveNextCueNumber?: number | null;
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
	const rows = compact
		? effectiveNextIsLoaded
			? ([[effectiveNext, effectiveNextIndex, "next"]] as const)
			: ([[current, cueIndex, "current"]] as const)
		: ([
				[cues[cueIndex - 1], cueIndex - 1, "previous"],
				[current, cueIndex, "current"],
				[effectiveNext, effectiveNextIndex, "next"],
			] as const);
	return (
		<div className={`hardware-cue-list ${compact ? "single" : "triple"}`}>
			{rows.map(([cue, index, kind]) => (
				<div
					className={`hardware-cue-row ${kind} ${kind === "next" && effectiveNextIsLoaded ? "loaded-next" : ""}`}
					style={
						kind === "current"
							? ({ "--cue-fade-progress": progress } as CSSProperties)
							: undefined
					}
					key={`${kind}-${index}`}
				>
					<i />
					<span>{cue?.number ?? "—"}</span>
					<b>{cue?.name || (cue ? `Cue ${cue.number}` : "—")}</b>
					<small>
						{kind === "next" && effectiveNextIsLoaded
							? "LOADED NEXT"
							: cue?.fade_millis
								? `${(cue.fade_millis / 1000).toFixed(1)}s`
								: ""}
					</small>
				</div>
			))}
		</div>
	);
}
