#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { artifactPaths } from "./artifact-paths.mjs";

const repositoryRoot = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	"..",
);
const lockfile = path.join(repositoryRoot, "package-lock.json");
const installedLockfile = path.join(
	repositoryRoot,
	"node_modules",
	".package-lock.json",
);
const stampDirectory = path.join(artifactPaths.root, "cache", "npm-ci");
const stampPath = path.join(stampDirectory, "signature");

if (!fs.existsSync(lockfile)) {
	console.error(`error: npm lockfile not found: ${lockfile}`);
	process.exit(1);
}

const nodeMajor = process.versions.node.split(".")[0];
const signature = createHash("sha256")
	.update(fs.readFileSync(lockfile))
	.update(
		`\nnode-major=${nodeMajor}\nplatform=${process.platform}\narch=${process.arch}\n`,
	)
	.digest("hex");
const recorded = fs.existsSync(stampPath)
	? fs.readFileSync(stampPath, "utf8").trim()
	: "";
const dependenciesAreReusable =
	process.env.LIGHT_FORCE_NPM_CI !== "1" &&
	fs.existsSync(installedLockfile) &&
	recorded === signature;

if (dependenciesAreReusable) {
	console.log(
		"Workspace dependencies match package-lock.json; reusing node_modules.",
	);
	process.exit(0);
}

console.log("Installing workspace dependencies from package-lock.json...");
const installation = spawnSync("npm", ["ci"], {
	cwd: repositoryRoot,
	env: process.env,
	stdio: "inherit",
});
if (installation.error) {
	console.error(`error: failed to start npm ci: ${installation.error.message}`);
	process.exit(1);
}
if (installation.status !== 0) process.exit(installation.status ?? 1);

fs.mkdirSync(stampDirectory, { recursive: true });
const temporaryStamp = `${stampPath}.${process.pid}.tmp`;
fs.writeFileSync(temporaryStamp, `${signature}\n`);
fs.renameSync(temporaryStamp, stampPath);
