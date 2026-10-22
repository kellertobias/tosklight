type Point = [number, number];

/** Clip in time after constructing the real fade slopes; a clipped fade never becomes faster. */
function clipAt(
	points: Point[],
	boundary: number,
	keepRight: boolean,
): Point[] {
	const inside = (point: Point) =>
		keepRight ? point[0] >= boundary : point[0] <= boundary;
	const result: Point[] = [];
	for (let index = 0; index < points.length; index++) {
		const current = points[index];
		const previous = points[(index + points.length - 1) % points.length];
		if (inside(current) !== inside(previous)) {
			const fraction = (boundary - previous[0]) / (current[0] - previous[0]);
			result.push([
				boundary,
				previous[1] + fraction * (current[1] - previous[1]),
			]);
		}
		if (inside(current)) result.push(current);
	}
	return result;
}

/** Shared by editable envelopes and collapsed lane miniatures, including interrupted fades. */
export function envelopePoints(
	left: number,
	inStart: number,
	inEnd: number,
	outStart: number,
	outEnd: number,
	base: number,
	peak: number,
	clipStart = -Infinity,
	clipEnd = Infinity,
): string {
	let points: Point[];
	if (inEnd <= outStart)
		points = [
			[left, base],
			[inStart, base],
			[inEnd, peak],
			[outStart, peak],
			[outEnd, base],
		];
	else if (outEnd <= inStart)
		points = [
			[left, base],
			[outEnd, base],
		];
	else {
		const inDuration = inEnd - inStart;
		const outDuration = outEnd - outStart;
		const crossing =
			(inStart * outDuration + outEnd * inDuration) /
			(inDuration + outDuration || 1);
		const level = Math.max(
			0,
			Math.min(1, inDuration ? (crossing - inStart) / inDuration : 1),
		);
		points = [
			[left, base],
			[inStart, base],
			[crossing, base + (peak - base) * level],
			[outEnd, base],
		];
	}
	if (Number.isFinite(clipStart)) points = clipAt(points, clipStart, true);
	if (Number.isFinite(clipEnd)) points = clipAt(points, clipEnd, false);
	return points.map((point) => point.join(",")).join(" ");
}
