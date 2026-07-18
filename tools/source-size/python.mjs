import { assignOccurrenceIds, finding, lineLocator } from "./shared.mjs";

function maskPython(source) {
  const output = source.split("");
  let quote;
  let triple = false;
  for (let index = 0; index < source.length; ) {
    const current = source[index];
    if (!quote && current === "#") {
      while (index < source.length && source[index] !== "\n") output[index++] = " ";
    } else if (!quote && (source.startsWith("'''", index) || source.startsWith('"""', index))) {
      quote = current;
      triple = true;
      for (let count = 0; count < 3; count += 1) output[index++] = " ";
    } else if (!quote && (current === "'" || current === '"')) {
      quote = current;
      triple = false;
      output[index++] = " ";
    } else if (quote && triple && source.startsWith(quote.repeat(3), index)) {
      for (let count = 0; count < 3; count += 1) output[index++] = " ";
      quote = undefined;
    } else if (quote && !triple && current === quote) {
      output[index++] = " ";
      quote = undefined;
    } else if (quote && current === "\\") {
      if (source[index] !== "\n") output[index] = " ";
      index += 1;
      if (index < source.length) {
        if (source[index] !== "\n") output[index] = " ";
        index += 1;
      }
    } else {
      if (quote && current !== "\n" && current !== "\r") output[index] = " ";
      index += 1;
    }
  }
  return output.join("");
}

function indentation(line) {
  const whitespace = line.match(/^[ \t]*/u)?.[0] ?? "";
  return [...whitespace].reduce((width, character) => width + (character === "\t" ? 8 : 1), 0);
}

function functionEnd(lines, startLine, definitionIndent) {
  let lastBodyLine = startLine;
  for (let line = startLine + 1; line < lines.length; line += 1) {
    if (lines[line].trim() === "") continue;
    if (indentation(lines[line]) <= definitionIndent) break;
    lastBodyLine = line;
  }
  return lastBodyLine;
}

export function scanPythonFunctions(file, source) {
  const masked = maskPython(source);
  const lineAt = lineLocator(source);
  const lines = source.split(/\r?\n/u);
  const maskedLines = masked.split(/\r?\n/u);
  const offsets = [];
  let offset = 0;
  for (const line of lines) {
    offsets.push(offset);
    offset += line.length + 1;
  }
  const findings = [];
  for (let line = 0; line < maskedLines.length; line += 1) {
    const match = maskedLines[line].match(/^([ \t]*)(?:async\s+)?def\s+([A-Za-z_]\w*)\b/u);
    if (!match) continue;
    const endLine = functionEnd(maskedLines, line, indentation(match[1]));
    const start = offsets[line] + match[1].length;
    const end = offsets[endLine] + Math.max(0, lines[endLine].length - 1);
    findings.push(finding({
      file, language: "python", kind: "function", name: match[2],
      header: maskedLines[line].trim(), lineAt, start, end,
    }));
  }
  return assignOccurrenceIds(findings);
}
