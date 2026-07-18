import { assignOccurrenceIds, finding, lineLocator, maskRust, matchingDelimiter } from "./shared.mjs";

function bodyStart(masked, start) {
  let parentheses = 0;
  let brackets = 0;
  for (let index = start; index < masked.length; index += 1) {
    const current = masked[index];
    if (current === "(") parentheses += 1;
    else if (current === ")") parentheses -= 1;
    else if (current === "[") brackets += 1;
    else if (current === "]") brackets -= 1;
    else if (current === ";" && parentheses === 0 && brackets === 0) return undefined;
    else if (current === "{" && parentheses === 0 && brackets === 0) return index;
  }
  return undefined;
}

export function scanRustFunctions(file, source) {
  const masked = maskRust(source);
  const lineAt = lineLocator(source);
  const findings = [];
  for (const match of masked.matchAll(/\bfn\s+([A-Za-z_][\w]*)/gu)) {
    const start = match.index;
    const open = bodyStart(masked, start + match[0].length);
    if (open === undefined) continue;
    const close = matchingDelimiter(masked, open);
    if (close === undefined) continue;
    findings.push(
      finding({
        file,
        language: "rust",
        kind: "function",
        name: match[1],
        header: source.slice(start, open + 1),
        lineAt,
        start,
        end: close,
      }),
    );
  }
  return assignOccurrenceIds(findings);
}
