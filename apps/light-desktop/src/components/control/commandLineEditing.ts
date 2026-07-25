export function removeCommandToken(value: string): string {
  const trimmed = value.trimEnd();
  if (!trimmed) return "";
  const last = trimmed.at(-1)!;
  if (/\d|[.\-]/.test(last)) return trimmed.slice(0, -1).trimEnd();
  return trimmed.replace(/\s*[A-Za-z]+$/, "").trimEnd();
}
