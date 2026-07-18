import { assignOccurrenceIds, finding, lineLocator, matchingDelimiter } from "./shared.mjs";

function maskShell(source) {
  const output = source.split("");
  let quote;
  for (let index = 0; index < source.length; ) {
    const current = source[index];
    if (!quote && current === "#") {
      while (index < source.length && source[index] !== "\n") output[index++] = " ";
    } else if (!quote && (current === "'" || current === '"')) {
      quote = current;
      output[index++] = " ";
    } else if (quote && current === quote) {
      quote = undefined;
      output[index++] = " ";
    } else if (quote && current === "\\" && quote === '"') {
      output[index++] = " ";
      if (index < source.length) output[index++] = " ";
    } else {
      if (quote && current !== "\n" && current !== "\r") output[index] = " ";
      index += 1;
    }
  }
  return output.join("");
}

export function scanShellFunctions(file, source) {
  const masked = maskShell(source);
  const lineAt = lineLocator(source);
  const findings = [];
  const pattern = /^[ \t]*(?:function[ \t]+)?([A-Za-z_][\w]*)[ \t]*(?:\([ \t]*\))?[ \t]*\{/gmu;
  for (const match of masked.matchAll(pattern)) {
    const open = match.index + match[0].lastIndexOf("{");
    const close = matchingDelimiter(masked, open);
    if (close === undefined) continue;
    findings.push(finding({
      file, language: "shell", kind: "function", name: match[1],
      header: source.slice(match.index, open + 1), lineAt, start: match.index, end: close,
    }));
  }
  return assignOccurrenceIds(findings);
}
