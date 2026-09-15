/**
 * One number across several selected elements, written as the desk writes a range.
 *
 * A field for a selection shows the one value every element shares, or the two ends of an even
 * spread as `first THRU last`, and is empty when the values follow no such line. What is typed back
 * is a single value for all, or two ends joined by `THRU`, `…` or `...`; the ends are spread evenly
 * over the elements in the order they were selected.
 */
import { formatNumber } from "./cadFields";

export interface ThruRange {
	first: number;
	last: number;
}

const NUMBER = String.raw`[-+]?(?:\d+(?:[.,]\d*)?|[.,]\d+)`;
const SINGLE = new RegExp(`^(${NUMBER})$`, "u");
const RANGE = new RegExp(String.raw`^(${NUMBER})\s*(?:\s+THRU\s+|…|\.\.\.)\s*(${NUMBER})$`, "iu");

function toNumber(text: string) {
	return Number(text.replace(",", "."));
}

/** A typed value or range, or null when the text is neither. */
export function parseThru(text: string): ThruRange | null {
	const trimmed = text.trim();
	const range = RANGE.exec(trimmed);
	if (range) {
		const [first, last] = [toNumber(range[1]), toNumber(range[2])];
		return Number.isFinite(first) && Number.isFinite(last) ? { first, last } : null;
	}
	const single = SINGLE.exec(trimmed);
	if (!single) return null;
	const value = toNumber(single[1]);
	return Number.isFinite(value) ? { first: value, last: value } : null;
}

/** `count` values from `first` to `last`, evenly apart; one element takes the first. */
export function spreadThru({ first, last }: ThruRange, count: number): number[] {
	if (count <= 1) return count === 1 ? [first] : [];
	return Array.from({ length: count }, (_, index) => first + ((last - first) * index) / (count - 1));
}

/**
 * How a field shows the selection's values: one shared value, an even spread as a range, or empty
 * with `mixed` set when they follow no line. Absent values — a fixture with no barn doors — are
 * shared only when every element lacks one.
 */
export function describeThru(
	values: readonly (number | null)[],
	digits: number,
): { text: string; mixed: boolean } {
	if (!values.length) return { text: "", mixed: false };
	if (values.every((value) => value == null)) return { text: "", mixed: false };
	if (values.some((value) => value == null)) return { text: "", mixed: true };
	const numbers = values as number[];
	const tolerance = 0.5 * 10 ** -digits;
	const [first] = numbers;
	const last = numbers[numbers.length - 1];
	if (numbers.every((value) => Math.abs(value - first) <= tolerance))
		return { text: formatNumber(first, digits), mixed: false };
	const even = spreadThru({ first, last }, numbers.length).every(
		(expected, index) => Math.abs(expected - numbers[index]) <= tolerance,
	);
	return even
		? { text: `${formatNumber(first, digits)} THRU ${formatNumber(last, digits)}`, mixed: false }
		: { text: "", mixed: true };
}
