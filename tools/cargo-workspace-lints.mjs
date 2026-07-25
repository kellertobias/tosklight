import fs from "node:fs";

export function manifestInheritsWorkspaceLints(source) {
  let table = "";

  for (const line of source.split(/\r?\n/u)) {
    const tableMatch = line.match(/^\s*\[([^\]]+)\]\s*(?:#.*)?$/u);
    if (tableMatch) {
      table = tableMatch[1].trim();
      continue;
    }

    if (table === "lints" && /^\s*workspace\s*=\s*true\s*(?:#.*)?$/u.test(line)) {
      return true;
    }
  }

  return false;
}

export function workspaceLintInheritanceFailures(metadata, readFile = fs.readFileSync) {
  const workspaceIds = new Set(metadata.workspace_members);

  return metadata.packages
    .filter((candidate) => workspaceIds.has(candidate.id))
    .filter((candidate) => !manifestInheritsWorkspaceLints(readFile(candidate.manifest_path, "utf8")))
    .map(
      (candidate) =>
        `${candidate.name} must inherit workspace lints with [lints] workspace = true in ${candidate.manifest_path}`,
    )
    .sort();
}
