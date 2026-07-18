import {
  assignOccurrenceIds,
  finding,
  lineLocator,
  maskJavaScript,
  matchingDelimiter,
  skipSpace,
} from "./shared.mjs";

const NON_METHOD_NAMES = new Set([
  "catch",
  "for",
  "if",
  "new",
  "switch",
  "while",
  "with",
]);

function previousWord(masked, start) {
  const prefix = masked.slice(Math.max(0, start - 80), start).trimEnd();
  return prefix.match(/([A-Za-z_$][\w$]*)$/u)?.[1];
}

function openingParenthesis(masked, start) {
  let index = skipSpace(masked, start);
  if (masked[index] === "<") {
    const close = matchingDelimiter(masked, index, "<", ">");
    if (close === undefined) return undefined;
    index = skipSpace(masked, close + 1);
  }
  return masked[index] === "(" ? index : undefined;
}

function bodyAfterParameters(masked, closeParenthesis) {
  let index = skipSpace(masked, closeParenthesis + 1);
  if (masked[index] === "{") return index;
  if (masked[index] !== ":") return undefined;
  index += 1;
  while (index < masked.length) {
    index = skipSpace(masked, index);
    if (masked[index] === ";" || masked.startsWith("=>", index)) return undefined;
    if (masked[index] !== "{") {
      index += 1;
      continue;
    }
    const close = matchingDelimiter(masked, index);
    if (close === undefined) return undefined;
    const next = skipSpace(masked, close + 1);
    if (masked[next] === "{" || masked[next] === "|" || masked[next] === "&") {
      index = close + 1;
      continue;
    }
    return index;
  }
  return undefined;
}

function namedFunctions(file, source, masked, lineAt) {
  const findings = [];
  for (const match of masked.matchAll(/\bfunction\s*\*?\s*([A-Za-z_$][\w$]*)?/gu)) {
    const openParenthesis = masked.indexOf("(", match.index + match[0].length);
    if (openParenthesis < 0) continue;
    const closeParenthesis = matchingDelimiter(masked, openParenthesis, "(", ")");
    if (closeParenthesis === undefined) continue;
    const open = bodyAfterParameters(masked, closeParenthesis);
    const close = open === undefined ? undefined : matchingDelimiter(masked, open);
    if (open === undefined || close === undefined) continue;
    const name = match[1] ?? "anonymous";
    findings.push(finding({
      file, language: "javascript", kind: "function", name,
      header: source.slice(match.index, open + 1), lineAt, start: match.index, end: close,
    }));
  }
  return findings;
}

function methods(file, source, masked, lineAt) {
  const findings = [];
  for (const match of masked.matchAll(/\b([A-Za-z_$][\w$]*|constructor)\b/gu)) {
    const name = match[1];
    if (NON_METHOD_NAMES.has(name) || previousWord(masked, match.index) === "function") continue;
    if (masked.slice(0, match.index).trimEnd().endsWith(".")) continue;
    const openParenthesis = openingParenthesis(masked, match.index + match[0].length);
    if (openParenthesis === undefined) continue;
    const closeParenthesis = matchingDelimiter(masked, openParenthesis, "(", ")");
    if (closeParenthesis === undefined) continue;
    const open = bodyAfterParameters(masked, closeParenthesis);
    const close = open === undefined ? undefined : matchingDelimiter(masked, open);
    if (open === undefined || close === undefined) continue;
    findings.push(finding({
      file, language: "javascript", kind: "method", name,
      header: source.slice(match.index, open + 1), lineAt, start: match.index, end: close,
    }));
  }
  return findings;
}

function arrowIdentity(masked, arrow) {
  const windowStart = Math.max(0, arrow - 1_500);
  const prefix = masked.slice(windowStart, arrow);
  const lineStart = masked.lastIndexOf("\n", arrow - 1) + 1;
  const linePrefix = masked.slice(lineStart, arrow);
  const lineDeclaration = linePrefix.match(/\b(?:const|let|var)\s+([A-Za-z_$][\w$]*)\b[^;]*=/u)?.[1];
  if (lineDeclaration) return { name: lineDeclaration, start: lineStart };
  const property = linePrefix.match(/([A-Za-z_$][\w$]*)\s*[:=][^:=]*$/u)?.[1];
  if (property) return { name: property, start: lineStart };
  let declaration;
  for (const match of prefix.matchAll(/\b(?:const|let|var)\s+([A-Za-z_$][\w$]*)\b/gu)) declaration = match;
  if (declaration && !prefix.slice(declaration.index).includes(";")) {
    return { name: declaration[1], start: windowStart + declaration.index };
  }
  return { name: "arrow", start: lineStart };
}

function expressionEnd(masked, start) {
  const first = skipSpace(masked, start);
  const pairs = { "(": ")", "[": "]" };
  if (pairs[masked[first]]) return matchingDelimiter(masked, first, masked[first], pairs[masked[first]]);
  const depths = { "(": 0, "[": 0, "{": 0 };
  const closing = { ")": "(", "]": "[", "}": "{" };
  for (let index = first; index < masked.length; index += 1) {
    const current = masked[index];
    if (current in depths) depths[current] += 1;
    else if (current in closing) {
      const opener = closing[current];
      if (depths[opener] === 0) return index - 1;
      depths[opener] -= 1;
    } else if ((current === ";" || current === ",") && Object.values(depths).every((depth) => depth === 0)) {
      return index - 1;
    }
  }
  return masked.length - 1;
}

function arrows(file, source, masked, lineAt) {
  const findings = [];
  for (const match of masked.matchAll(/=>/gu)) {
    const open = skipSpace(masked, match.index + match[0].length);
    const end = masked[open] === "{" ? matchingDelimiter(masked, open) : expressionEnd(masked, open);
    if (end === undefined || end < open) continue;
    const identity = arrowIdentity(masked, match.index);
    findings.push(finding({
      file, language: "javascript", kind: "arrow", name: identity.name,
      header: source.slice(identity.start, match.index + 2), lineAt, start: identity.start, end,
    }));
  }
  return findings;
}

export function scanJavaScriptFunctions(file, source) {
  const masked = maskJavaScript(source);
  const lineAt = lineLocator(source);
  return assignOccurrenceIds([
    ...namedFunctions(file, source, masked, lineAt),
    ...methods(file, source, masked, lineAt),
    ...arrows(file, source, masked, lineAt),
  ]);
}
