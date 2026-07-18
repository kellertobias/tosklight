import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { exemptionReason, functionLanguage } from "./config.mjs";
import { scanJavaScriptFunctions } from "./javascript.mjs";
import { scanPythonFunctions } from "./python.mjs";
import { scanRustFunctions } from "./rust.mjs";
import { logicalLineCount } from "./shared.mjs";
import { scanShellFunctions } from "./shell.mjs";

const DECODER = new TextDecoder("utf-8", { fatal: true });

function repositoryFiles(repositoryRoot) {
  return execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z"], {
    cwd: repositoryRoot,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
  })
    .split("\0")
    .filter(Boolean)
    .sort();
}

function textSource(file) {
  const bytes = fs.readFileSync(file);
  if (bytes.includes(0)) return undefined;
  try {
    return DECODER.decode(bytes);
  } catch {
    return undefined;
  }
}

function scanFunctions(repositoryPath, source) {
  switch (functionLanguage(repositoryPath, source)) {
    case "javascript": return scanJavaScriptFunctions(repositoryPath, source);
    case "python": return scanPythonFunctions(repositoryPath, source);
    case "rust": return scanRustFunctions(repositoryPath, source);
    case "shell": return scanShellFunctions(repositoryPath, source);
    default: return [];
  }
}

export function scanRepository(repositoryRoot) {
  const files = [];
  const functions = [];
  const exemptions = [];
  for (const repositoryPath of repositoryFiles(repositoryRoot)) {
    const absolutePath = path.join(repositoryRoot, repositoryPath);
    if (!fs.existsSync(absolutePath) || !fs.statSync(absolutePath).isFile()) continue;
    const source = textSource(absolutePath);
    if (source === undefined) continue;
    const reason = exemptionReason(repositoryPath);
    if (reason) {
      exemptions.push({ path: repositoryPath, reason });
      continue;
    }
    files.push({ id: repositoryPath, lines: logicalLineCount(source), path: repositoryPath });
    functions.push(...scanFunctions(repositoryPath, source));
  }
  return { exemptions, files, functions };
}
