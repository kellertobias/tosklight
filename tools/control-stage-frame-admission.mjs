/** Validate measured renderer frames, rather than the settings requested by Control. */
export function controlStageFrameFailures(frames) {
	const failures = [];
	if (!frames.length || frames.some((frame) => frame.quality !== "draft"))
		failures.push(
			"Control Stage presented a frame outside its fixed Draft quality",
		);
	const allowedModes = new Set([
		"lines_3d",
		"top_down",
		"front_to_back",
		"back_to_front",
		"left_to_right",
		"right_to_left",
	]);
	if (frames.some((frame) => !allowedModes.has(frame.mode)))
		failures.push(
			"Control Stage presented a full-render or unverified view mode",
		);
	if (
		!frames.some(
			(frame) => frame.lane === "normal" && frame.mode === "lines_3d",
		)
	)
		failures.push("Control Stage presented no actual wireframe 3D frame");
	return failures;
}
