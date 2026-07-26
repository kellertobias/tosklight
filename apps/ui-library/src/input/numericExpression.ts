/**
 * Submits a single numeric value or a THRU-separated list of numeric control
 * points. The caller owns the domain meaning of either callback.
 */
export function submitNumericExpression(
  input: string,
  onValue: ((value: number) => void) | undefined,
  onRange: ((points: number[]) => void) | undefined,
): boolean {
  const points = input.split(/\s+THRU\s+/i).map((part) => Number(part.trim()));
  if (points.length > 1) {
    if (!onRange || points.some((value) => !Number.isFinite(value))) return false;
    onRange(points);
    return true;
  }
  const value = Number(input);
  if (Number.isFinite(value)) onValue?.(value);
  return true;
}
