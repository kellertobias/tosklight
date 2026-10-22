import { formatFrame } from "./timecodeEditorShared";
export function Ruler({
	duration,
	fps,
	pixelsPerFrame,
}: {
	duration: number;
	fps: number;
	pixelsPerFrame: number;
}) {
	const step = pixelsPerFrame * fps >= 52 ? fps : fps * 5;
	const ticks = Array.from(
		{ length: Math.floor(duration / step) + 1 },
		(_, index) => index * step,
	);
	return (
		<>
			<div className="timecode-ruler-stripes" aria-hidden="true">
				{ticks.map((tick) => (
					<i key={tick} style={{ left: tick * pixelsPerFrame }} />
				))}
			</div>
			<div className="timecode-timeline-heading">
				<div className="timecode-ruler">
					{ticks.map((tick) => (
						<span
							key={tick}
							className={
								tick === 0
									? "timecode-ruler-first-tick"
									: tick === duration
										? "timecode-ruler-final-tick"
										: undefined
							}
							style={{ left: tick * pixelsPerFrame }}
						>
							{formatFrame(tick, fps)}
						</span>
					))}
				</div>
			</div>
		</>
	);
}
