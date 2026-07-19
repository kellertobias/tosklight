/** Structural equality for small decoded projections without serialization churn. */
export function sameValue(left: unknown, right: unknown): boolean {
	if (Object.is(left, right)) return true;
	if (Array.isArray(left) || Array.isArray(right))
		return (
			Array.isArray(left) &&
			Array.isArray(right) &&
			left.length === right.length &&
			left.every((value, index) => sameValue(value, right[index]))
		);
	if (!isRecord(left) || !isRecord(right)) return false;
	const leftKeys = Object.keys(left).sort();
	const rightKeys = Object.keys(right).sort();
	return (
		leftKeys.length === rightKeys.length &&
		leftKeys.every(
			(key, index) =>
				key === rightKeys[index] && sameValue(left[key], right[key]),
		)
	);
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}
