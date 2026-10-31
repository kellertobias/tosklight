/** Deterministic, bounded label placement in drawing coordinates. Text is never shortened. */
export interface LabelBounds { x: number; y: number; width: number; height: number }
export interface LabelRequest {
	key: string;
	anchor: [number, number];
	lines: string[];
	width: number;
	height: number;
	symbolBounds?: LabelBounds;
}
export interface PlacedLabel extends LabelRequest {
	box: LabelBounds;
	leader: [[number, number], [number, number]];
}
export interface LabelLayout {
	labels: PlacedLabel[];
	unplaced: LabelRequest[];
	intersectionChecks: number;
}
export const LABEL_CAPACITY_MESSAGE = "Fixture labels cannot fit at readable size. Use closer framing or split the plan into pages.";

export function boundsOverlap(a: LabelBounds, b: LabelBounds, gap = 0): boolean {
	return a.x < b.x + b.width + gap && a.x + a.width + gap > b.x
		&& a.y < b.y + b.height + gap && a.y + a.height + gap > b.y;
}
function contains(bounds: LabelBounds, box: LabelBounds) {
	return box.x >= bounds.x && box.y >= bounds.y
		&& box.x + box.width <= bounds.x + bounds.width + 0.001
		&& box.y + box.height <= bounds.y + bounds.height + 0.001;
}

/** Uniform cells keep candidate queries local even with hundreds of fixture instances. */
class LabelIndex {
	private cells = new Map<string, LabelBounds[]>();
	checks = 0;
	constructor(private cellSize = 32) {}
	private keys(box: LabelBounds, gap: number) {
		const keys: string[] = [];
		for (let y = Math.floor((box.y - gap) / this.cellSize); y <= Math.floor((box.y + box.height + gap) / this.cellSize); y++)
			for (let x = Math.floor((box.x - gap) / this.cellSize); x <= Math.floor((box.x + box.width + gap) / this.cellSize); x++) keys.push(`${x}:${y}`);
		return keys;
	}
	add(box: LabelBounds) {
		for (const key of this.keys(box, 0)) {
			const values = this.cells.get(key) ?? [];
			values.push(box); this.cells.set(key, values);
		}
	}
	collides(box: LabelBounds, gap: number) {
		const seen = new Set<LabelBounds>();
		for (const key of this.keys(box, gap)) for (const other of this.cells.get(key) ?? []) {
			if (seen.has(other)) continue;
			seen.add(other); this.checks++;
			if (boundsOverlap(box, other, gap)) return true;
		}
		return false;
	}
}
function placed(request: LabelRequest, box: LabelBounds): PlacedLabel {
	return { ...request, box, leader: [request.anchor, [
		Math.max(box.x, Math.min(request.anchor[0], box.x + box.width)),
		Math.max(box.y, Math.min(request.anchor[1], box.y + box.height)),
	]] };
}
function* candidates(request: LabelRequest, gap: number) {
	const [x, y] = request.anchor;
	const body = request.symbolBounds;
	if (body) {
		const right = body.x + body.width + gap, left = body.x - gap - request.width;
		const above = body.y - gap - request.height, below = body.y + body.height + gap;
		for (const [cx, cy] of [[right, y - request.height / 2], [left, y - request.height / 2],
			[x - request.width / 2, above], [x - request.width / 2, below],
			[right, above], [left, above], [right, below], [left, below]])
			yield { x: cx, y: cy, width: request.width, height: request.height };
	}
	for (let ring = 0; ring < 7; ring++) {
		const distance = 3 + ring * (request.height + gap);
		for (const [cx, cy] of [
			[x + distance, y - request.height / 2], [x - distance - request.width, y - request.height / 2],
			[x - request.width / 2, y - distance - request.height], [x - request.width / 2, y + distance],
			[x + distance, y - distance - request.height], [x + distance, y + distance],
			[x - distance - request.width, y - distance - request.height], [x - distance - request.width, y + distance],
		]) yield { x: cx, y: cy, width: request.width, height: request.height };
	}
}

/** Search remaining space by distance when the local rings are exhausted. */
function nearestGridBox(request: LabelRequest, bounds: LabelBounds, index: LabelIndex, gap: number) {
	let best: LabelBounds | undefined;
	let distance = Infinity;
	for (let y = bounds.y; y + request.height <= bounds.y + bounds.height; y += request.height + gap)
		for (let x = bounds.x; x + request.width <= bounds.x + bounds.width; x += request.width + gap) {
			const nextDistance = (x + request.width / 2 - request.anchor[0]) ** 2 + (y + request.height / 2 - request.anchor[1]) ** 2;
			if (nextDistance >= distance) continue;
			const box = { x, y, width: request.width, height: request.height };
			if (!index.collides(box, gap)) { best = box; distance = nextDistance; }
		}
	return best;
}

export function placeLabels(requests: readonly LabelRequest[], bounds: LabelBounds, gap = 1.5, obstacles: readonly LabelBounds[] = []): LabelLayout {
	const ordered = [...requests].sort((a, b) => a.anchor[1] - b.anchor[1] || a.anchor[0] - b.anchor[0] || (a.key < b.key ? -1 : a.key > b.key ? 1 : 0));
	const index = new LabelIndex();
	for (const obstacle of obstacles) index.add(obstacle);
	const labels: PlacedLabel[] = [];
	const unplaced: LabelRequest[] = [];
	for (const request of ordered) {
		const box = Array.from(candidates(request, gap)).find((candidate) => contains(bounds, candidate) && !index.collides(candidate, gap))
			?? nearestGridBox(request, bounds, index, gap);
		if (!box) { unplaced.push(request); continue; }
		index.add(box); labels.push(placed(request, box));
	}
	return { labels, unplaced, intersectionChecks: index.checks };
}

/** Conservative Helvetica-bold advances, plus a clear pad around the complete label. */
export function labelWidth(lines: readonly string[], size: number, padding = 2) {
	return Math.max(...lines.map((line) => Array.from(line).reduce((width, character) => width + (/\s/.test(character) ? 0.35 : /[ilI1.:]/.test(character) ? 0.5 : /[MW@]/.test(character) ? 1.1 : 0.76), 0) * size), 0) + padding * 2;
}
