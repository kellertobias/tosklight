#!/usr/bin/env node
// Derive the next semantic version from Conventional Commits since the last v* tag.
// Prints the version on stdout, or prints nothing and exits 0 when no release is warranted.

import { execFileSync } from "node:child_process";

const git = (...args) => execFileSync("git", args, { encoding: "utf8" }).trim();

const tags = git("tag", "--list", "v*.*.*", "--sort=-v:refname")
  .split("\n")
  .filter(Boolean);
const lastTag = tags[0];

const range = lastTag ? `${lastTag}..HEAD` : "HEAD";
const log = git("log", range, "--no-merges", "--format=%B%x00");
const commits = log.split("\0").map((entry) => entry.trim()).filter(Boolean);

if (commits.length === 0) {
  process.exit(0);
}

const HEADER = /^(?<type>[a-zA-Z]+)(?:\((?<scope>[^)]*)\))?(?<breaking>!)?:\s+(?<subject>.+)$/;

let bump = 0; // 0 none, 1 patch, 2 minor, 3 major
for (const commit of commits) {
  const [header, ...body] = commit.split("\n");
  const match = HEADER.exec(header.trim());
  if (!match) continue;
  const { type, breaking } = match.groups;
  if (breaking || body.some((line) => /^BREAKING[ -]CHANGE:/.test(line.trim()))) {
    bump = Math.max(bump, 3);
  } else if (type === "feat") {
    bump = Math.max(bump, 2);
  } else if (type === "fix" || type === "perf" || type === "revert") {
    bump = Math.max(bump, 1);
  }
}

if (bump === 0) {
  process.exit(0);
}

const [major, minor, patch] = (lastTag ?? "v0.0.0").slice(1).split(".").map(Number);
// Pre-1.0: a breaking change moves the minor, not the major.
const next =
  bump === 3
    ? major === 0
      ? [0, minor + 1, 0]
      : [major + 1, 0, 0]
    : bump === 2
      ? [major, minor + 1, 0]
      : [major, minor, patch + 1];

process.stdout.write(next.join("."));
