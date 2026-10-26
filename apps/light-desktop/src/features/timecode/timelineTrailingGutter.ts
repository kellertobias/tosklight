/** Classic Chromium gutters reduce clientWidth but not the native scroll range. */
export function timelineTrailingGutter(
	canvasWidth: number,
	clientWidth: number,
	offsetWidth: number,
	borderWidth: number,
) {
	if (canvasWidth <= clientWidth) return 0;
	return Math.max(0, offsetWidth - clientWidth - borderWidth);
}
