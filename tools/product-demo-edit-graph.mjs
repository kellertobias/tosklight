export function buildProductDemoEditFilters(timeline, clips, clock) {
	const filters = clips.map((section, index) => {
		const sourceDuration = section.sourceEndMillis - section.sourceStartMillis;
		if (!(sourceDuration > 0) || !(section.frames > 0))
			throw new Error(
				`Product-demo section ${section.id ?? index} has an invalid duration`,
			);
		const targetDuration = (section.frames / timeline.fps) * 1_000;
		const speed = targetDuration / sourceDuration;
		return `[0:v]trim=start=${(section.sourceStartMillis / 1_000).toFixed(6)}:end=${(section.sourceEndMillis / 1_000).toFixed(6)},setpts=${speed.toFixed(9)}*(PTS-STARTPTS),fps=${timeline.fps},settb=expr=1/${timeline.fps},format=yuv420p[v${index}]`;
	});
	const transitionFrames = Math.max(0, Number(timeline.transitionFrames) || 0);
	if (clips.length > 1) {
		let elapsedFrames = clips[0].frames;
		let input = "[v0]";
		for (let index = 1; index < clips.length; index++) {
			const combined = `[m${index}]`;
			const output = `[x${index}]`;
			const previous = clips[index - 1];
			const transition = Math.max(
				0,
				Number(
					previous.transitionFramesAfter ??
						(index < clips.length ? transitionFrames : 0),
				) || 0,
			);
			if (transition > 0) {
				const offsetFrames = elapsedFrames - transition;
				filters.push(
					`${input}[v${index}]xfade=transition=fade:duration=${(transition / timeline.fps).toFixed(6)}:offset=${(offsetFrames / timeline.fps).toFixed(6)}${combined}`,
				);
			} else {
				filters.push(`${input}[v${index}]concat=n=2:v=1:a=0${combined}`);
			}
			filters.push(
				`${combined}fps=${timeline.fps},settb=expr=1/${timeline.fps}${output}`,
			);
			input = output;
			elapsedFrames += clips[index].frames - transition;
		}
		filters.push(
			`${input}trim=duration=${(timeline.totalFrames / timeline.fps).toFixed(6)},setpts=PTS-STARTPTS[editv]`,
		);
	} else {
		filters.push("[v0]null[editv]");
	}
	filters.push(
		`[editv]delogo=x=1195:y=920:w=88:h=44:show=0:enable='${clock.delogoEnable}'[clockbase]`,
		`[1:v]fps=${timeline.fps},format=rgba[clock]`,
		"[clockbase][clock]overlay=x=1185:y=915:shortest=1[outv]",
	);
	return filters;
}
