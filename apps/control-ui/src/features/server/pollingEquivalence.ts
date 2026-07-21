/**
 * Keeps the previous polled value when the next one is equivalent.
 *
 * Polled endpoints return a freshly decoded object on every tick. Storing it unconditionally
 * replaces broad server state and rerenders every facade consumer on the poll interval even when
 * nothing changed. Returning the previous reference lets React bail out of the update entirely.
 *
 * Comparison is structural and exhaustive rather than field-by-field, so a field added to a polled
 * contract can never be silently dropped from the check. The worst case is an extra render, which
 * is the previous behaviour; it can never report a changed value as unchanged and leave a stale
 * projection on a safety-relevant surface such as Highlight.
 */
export function retainEquivalent<T>(current: T, next: T): T {
	if (current === next) return current;
	return equivalent(current, next) ? current : next;
}

function equivalent(left: unknown, right: unknown) {
	try {
		return JSON.stringify(left) === JSON.stringify(right);
	} catch {
		// A value that cannot be serialized is treated as changed.
		return false;
	}
}
