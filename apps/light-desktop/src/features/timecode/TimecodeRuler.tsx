import { useLayoutEffect, useRef, useState } from "react";
import { formatFrame } from "./timecodeEditorShared";

function gridFrames(duration: number, fps: number, pixelsPerFrame: number) {
	const step = pixelsPerFrame * fps >= 52 ? fps : fps * 5;
	return Array.from({ length: Math.floor(duration / step) + 1 }, (_, index) => index * step);
}

/** Label spacing is independent of the time axis and its original stripe grid. */
export function rulerLabelFrames(duration: number, fps: number, pixelsPerFrame: number, labelWidth: number) {
	if (duration <= 0) return [0];
	const gap = 8;
	const endX = duration * pixelsPerFrame;
	const frames: number[] = endX >= labelWidth * 2 + gap ? [0] : [];
	for (const frame of gridFrames(duration, fps, pixelsPerFrame)) {
		if (frame === 0 || frame >= duration) continue;
		const x = frame * pixelsPerFrame;
		const previous = frames.at(-1);
		if ((previous === undefined || x >= previous * pixelsPerFrame + labelWidth + gap)
			&& x + labelWidth + gap <= endX - labelWidth) frames.push(frame);
	}
	return [...frames, duration];
}

export function Ruler({ duration, fps, pixelsPerFrame }: {
	duration: number;
	fps: number;
	pixelsPerFrame: number;
}) {
	const probe = useRef<HTMLElement>(null);
	const [labelWidth, setLabelWidth] = useState(80);
	useLayoutEffect(() => {
		const node = probe.current;
		if (!node) return;
		const measure = () => {
			const width = node.getBoundingClientRect().width;
			if (width > 0) setLabelWidth(Math.ceil(width) + 8);
		};
		measure();
		if (typeof ResizeObserver === "undefined") return;
		const observer = new ResizeObserver(measure);
		observer.observe(node);
		return () => observer.disconnect();
	}, [duration, fps]);
	const ticks = gridFrames(duration, fps, pixelsPerFrame);
	const labels = rulerLabelFrames(duration, fps, pixelsPerFrame, labelWidth);
	return <>
		<div className="timecode-ruler-stripes" aria-hidden="true">
			{ticks.map((tick) => <i key={tick} style={{ left: tick * pixelsPerFrame }} />)}
		</div>
		<div className="timecode-timeline-heading">
			<div className="timecode-ruler">
				<i ref={probe} className="timecode-ruler-label-probe" aria-hidden="true"
					style={{ position: "absolute", visibility: "hidden", pointerEvents: "none", fontStyle: "normal", width: "max-content", whiteSpace: "nowrap" }}>
					{formatFrame(duration, fps)}·
				</i>
				{labels.map((tick) => <span key={tick} data-frame={tick}
					className={tick === 0 ? "timecode-ruler-first-tick" : tick === duration ? "timecode-ruler-final-tick" : undefined}
					style={{ left: tick * pixelsPerFrame }}>{formatFrame(tick, fps)}</span>)}
			</div>
		</div>
	</>;
}
