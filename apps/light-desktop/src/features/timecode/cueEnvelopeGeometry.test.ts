import { expect, it } from "vitest";
import { envelopePoints } from "./cueEnvelopeGeometry";

const points = (value: string) =>
	value.split(" ").map((point) => point.split(",").map(Number));
it("clips an unfinished In fade at its real partial level", () => {
	const result = points(envelopePoints(0, 0, 200, 200, 200, 100, 0, 0, 100));
	expect(result.every(([x]) => x >= 0 && x <= 100)).toBe(true);
	expect(Math.min(...result.map(([, y]) => y))).toBe(50);
});
it("interrupted In and Out fades meet once without backwards polygon edges", () => {
	const result = points(envelopePoints(0, 0, 100, 20, 80, 100, 0, 0, 60));
	expect(result).toContainEqual([50, 50]);
	expect(result.every(([x]) => x >= 0 && x <= 60)).toBe(true);
	expect(result.filter(([x, y]) => x === 50 && y === 50)).toHaveLength(1);
});
it("keeps vertical zero fades representable", () => {
	expect(envelopePoints(0, 0, 0, 40, 40, 100, 0, 0, 40)).toBe(
		"0,100 0,100 0,0 40,0 40,100",
	);
});
