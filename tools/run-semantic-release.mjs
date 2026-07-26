#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import semanticRelease from "semantic-release";
import { artifactPaths } from "./artifact-paths.mjs";

const require = createRequire(import.meta.url);
const configuration = require("../release.config.cjs");
const { previewPlugins } = require("./semantic-release-plugins.cjs");
const dryRun = process.argv.includes("--dry-run");
const bootstrapTag = "v0.0.0";
let disposableRemote;
let createdBootstrap = false;

const git = (arguments_, options = {}) =>
  execFileSync("git", arguments_, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], ...options }).trim();

function hasSemanticTag() {
  return git(["tag", "--list"])
    .split("\n")
    .some((tag) => /^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/u.test(tag));
}

function deleteBootstrap() {
  if (!createdBootstrap) return;
  try {
    git(["tag", "--delete", bootstrapTag]);
  } catch {
    // The verifyRelease plugin removes it before semantic-release pushes tags.
  }
  createdBootstrap = false;
  delete process.env.LIGHT_SEMANTIC_RELEASE_BOOTSTRAP_TAG;
}

try {
  if (!hasSemanticTag()) {
    const rootCommit = git(["rev-list", "--max-parents=0", "HEAD"]).split("\n")[0];
    git(["tag", bootstrapTag, rootCommit]);
    createdBootstrap = true;
    process.env.LIGHT_SEMANTIC_RELEASE_BOOTSTRAP_TAG = bootstrapTag;
  }

  const options = { ...configuration };

  if (dryRun) {
    mkdirSync(artifactPaths.tmp, { recursive: true });
    disposableRemote = mkdtempSync(join(artifactPaths.tmp, "tosklight-semantic-release-"));
    git(["init", "--bare", disposableRemote]);
    git(["push", "--quiet", disposableRemote, "HEAD:refs/heads/main", "--tags"]);
    options.repositoryUrl = pathToFileURL(disposableRemote).href;
    options.plugins = previewPlugins;
    options.dryRun = true;
    options.noCi = true;
  }

  const result = await semanticRelease(options);
  process.exitCode = result === false ? 0 : process.exitCode;
} finally {
  deleteBootstrap();
  if (disposableRemote) rmSync(disposableRemote, { recursive: true, force: true });
}
