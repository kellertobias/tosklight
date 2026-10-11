import type { PlaybackPreloadScope } from "../../api/playbackConfigurationModels";
function ranges(value: string, max: number) {
	const items = value
		.split(/[ ,;]+/)
		.filter(Boolean)
		.map((token) => {
			const match = /^(\d+)(?:-(\d+))?$/.exec(token);
			if (!match)
				throw Error("Use numbers or ascending ranges separated by commas");
			const start = Number(match[1]),
				end = Number(match[2] ?? match[1]);
			if (start < 1 || end > max || start > end)
				throw Error(`Values must be within 1–${max} in ascending order`);
			return { start, end };
		})
		.sort((a, b) => a.start - b.start);

	const union: Array<{ start: number; end: number }> = [];
	for (const item of items) {
		const last = union.at(-1);
		if (last && item.start <= last.end + 1)
			last.end = Math.max(last.end, item.end);
		else union.push({ ...item });
	}
	return union;
}
export function parsePlaybackPreloadScope(
	positions: string,
	allPages: boolean,
	pages: string,
): PlaybackPreloadScope {
	return {
		position_ranges: ranges(positions, 1000),
		pages: allPages
			? null
			: ranges(pages, 99).flatMap((r) =>
					Array.from({ length: r.end - r.start + 1 }, (_, i) => r.start + i),
				),
	};
}
export function preloadScopeText(scope: PlaybackPreloadScope) {
	return {
		positions: scope.position_ranges
			.map((r) => (r.start === r.end ? String(r.start) : `${r.start}-${r.end}`))
			.join(", "),
		allPages: scope.pages === null,
		pages: scope.pages?.join(", ") ?? "",
	};
}
