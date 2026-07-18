import { createHash } from "node:crypto";

export function logicalLineCount(source) {
  if (source.length === 0) return 0;
  const lines = source.split(/\r\n|\n|\r/u);
  return lines.length - (lines.at(-1) === "" ? 1 : 0);
}

export function lineLocator(source) {
  const offsets = [];
  for (let index = source.indexOf("\n"); index >= 0; index = source.indexOf("\n", index + 1))
    offsets.push(index);
  return (offset) => {
    let low = 0;
    let high = offsets.length;
    while (low < high) {
      const middle = Math.floor((low + high) / 2);
      if (offsets[middle] < offset) low = middle + 1;
      else high = middle;
    }
    return low + 1;
  };
}

export function matchingDelimiter(masked, start, open = "{", close = "}") {
  let depth = 0;
  for (let index = start; index < masked.length; index += 1) {
    if (masked[index] === open) depth += 1;
    if (masked[index] === close && --depth === 0) return index;
  }
  return undefined;
}

export function skipSpace(masked, start) {
  let index = start;
  while (/\s/u.test(masked[index] ?? "")) index += 1;
  return index;
}

export function finding({ file, language, kind, name, header, lineAt, start, end }) {
  const normalized = header.replace(/\s+/gu, " ").trim();
  const hash = createHash("sha256").update(normalized).digest("hex").slice(0, 12);
  const startLine = lineAt(start);
  const endLine = lineAt(end);
  return {
    baseId: `${file}::${language}:${kind}:${name}:${hash}`,
    display: `${name} (${kind})`,
    endLine,
    lines: endLine - startLine + 1,
    startLine,
  };
}

export function assignOccurrenceIds(findings) {
  const occurrences = new Map();
  return findings
    .sort((left, right) => left.startLine - right.startLine || left.baseId.localeCompare(right.baseId))
    .map((candidate) => {
      const occurrence = (occurrences.get(candidate.baseId) ?? 0) + 1;
      occurrences.set(candidate.baseId, occurrence);
      const { baseId, ...rest } = candidate;
      return { ...rest, id: `${baseId}:${occurrence}` };
    });
}

function blank(output, index) {
  if (output[index] !== "\n" && output[index] !== "\r") output[index] = " ";
}

function maskQuoted(source, output, start, quote) {
  let index = start;
  blank(output, index++);
  while (index < source.length) {
    const current = source[index];
    blank(output, index++);
    if (current === "\\") {
      if (index < source.length) blank(output, index++);
    } else if (current === quote) {
      break;
    }
  }
  return index;
}

function maskBlockComment(source, output, start, nested) {
  let index = start;
  let depth = 0;
  while (index < source.length) {
    if (source.startsWith("/*", index)) {
      depth += 1;
      blank(output, index++);
      blank(output, index++);
      continue;
    }
    if (source.startsWith("*/", index)) {
      depth -= 1;
      blank(output, index++);
      blank(output, index++);
      if (depth === 0 || !nested) break;
      continue;
    }
    blank(output, index++);
  }
  return index;
}

export function maskRust(source) {
  const output = source.split("");
  for (let index = 0; index < source.length; ) {
    if (source.startsWith("//", index)) {
      while (index < source.length && source[index] !== "\n") blank(output, index++);
    } else if (source.startsWith("/*", index)) {
      index = maskBlockComment(source, output, index, true);
    } else {
      const raw = source.slice(index).match(/^(?:br|r)(#*)"/u);
      if (raw && !/[\w]/u.test(source[index - 1] ?? "")) {
        const terminator = `"${raw[1]}`;
        let end = source.indexOf(terminator, index + raw[0].length);
        end = end < 0 ? source.length : end + terminator.length;
        while (index < end) blank(output, index++);
      } else if (source[index] === '"') {
        index = maskQuoted(source, output, index, '"');
      } else if (source[index] === "'") {
        const character = source.slice(index).match(/^'(?:\\.|[^'\\\n])'/u);
        if (character) {
          const end = index + character[0].length;
          while (index < end) blank(output, index++);
        } else index += 1;
      } else index += 1;
    }
  }
  return output.join("");
}

const REGEX_PREFIX = new Set(["", "(", "[", "{", "=", ":", ",", ";", "!", "?", "&", "|", "+", "-", "*", "%", "^", "~", "<", ">"]);
const REGEX_KEYWORDS = new Set(["await", "case", "delete", "do", "else", "in", "instanceof", "new", "of", "return", "throw", "typeof", "void", "yield"]);

function regexCanStart(lastToken) {
  return REGEX_PREFIX.has(lastToken) || REGEX_KEYWORDS.has(lastToken);
}

function maskRegex(source, output, start) {
  let index = start;
  let characterClass = false;
  blank(output, index++);
  while (index < source.length) {
    const current = source[index];
    blank(output, index++);
    if (current === "\\") {
      if (index < source.length) blank(output, index++);
    } else if (current === "[") characterClass = true;
    else if (current === "]") characterClass = false;
    else if (current === "/" && !characterClass) break;
    else if (current === "\n") break;
  }
  while (/[a-z]/iu.test(source[index] ?? "")) blank(output, index++);
  return index;
}

export function maskJavaScript(source) {
  const output = source.split("");
  const templateParents = [];
  let templateDepth;
  let mode = "code";
  let lastToken = "";
  for (let index = 0; index < source.length; ) {
    const current = source[index];
    if (mode === "template") {
      if (current === "`") {
        blank(output, index++);
        templateDepth = templateParents.pop();
        mode = "code";
      } else if (source.startsWith("${", index)) {
        blank(output, index++);
        index += 1;
        templateDepth = 1;
        mode = "code";
        lastToken = "{";
      } else {
        blank(output, index++);
      }
      continue;
    }
    if (templateDepth && current === "{") templateDepth += 1;
    if (templateDepth && current === "}" && --templateDepth === 0) {
      index += 1;
      mode = "template";
      continue;
    }
    if (source.startsWith("//", index)) {
      while (index < source.length && source[index] !== "\n") blank(output, index++);
    } else if (source.startsWith("/*", index)) index = maskBlockComment(source, output, index, false);
    else if (current === '"' || current === "'") {
      index = maskQuoted(source, output, index, current);
      lastToken = "value";
    } else if (current === "`") {
      templateParents.push(templateDepth);
      blank(output, index++);
      templateDepth = undefined;
      mode = "template";
      lastToken = "value";
    } else if (current === "/" && source[index - 1] !== "<" && regexCanStart(lastToken)) {
      index = maskRegex(source, output, index);
      lastToken = "value";
    } else {
      const word = source.slice(index).match(/^[A-Za-z_$][\w$]*/u)?.[0];
      if (word) {
        lastToken = word;
        index += word.length;
      } else {
        if (!/\s/u.test(current)) lastToken = current;
        index += 1;
      }
    }
  }
  return output.join("");
}
