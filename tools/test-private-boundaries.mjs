import fs from "node:fs";
import path from "node:path";

export function readPrivateBoundarySources(repositoryRoot) {
  return ["tests", "apps/control-ui/e2e"]
    .flatMap((directory) => walk(path.join(repositoryRoot, directory)))
    .filter((file) => /\.[cm]?tsx?$/u.test(file))
    .map((file) => ({
      file: path.relative(repositoryRoot, file).split(path.sep).join("/"),
      source: fs.readFileSync(file, "utf8"),
    }));
}

export function scanPrivateTestBoundaries(sources) {
  const failures = [];
  for (const { file, source } of sources) {
    if (source.includes("light.primary-session"))
      failures.push(`${file} reads the private primary-session storage record`);
    if (source.includes("__TAURI_INTERNALS__"))
      failures.push(`${file} fabricates private Tauri runtime internals`);
		for (const event of rawLightEvents(source))
			failures.push(
				`${file} dispatches private ${event} instead of a public adapter`,
			);
  }
  return failures;
}

function rawLightEvents(source) {
  const expression = /window\s*\.\s*dispatchEvent\s*\(\s*new\s+(?:CustomEvent|Event)(?:<[^>]+>)?\s*\(\s*["'`](light:[^"'`]+)["'`]/gu;
  return [...source.matchAll(expression)].map((match) => match[1]);
}

function walk(directory) {
  if (!fs.existsSync(directory)) return [];
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const target = path.join(directory, entry.name);
    return entry.isDirectory() ? walk(target) : [target];
  });
}
