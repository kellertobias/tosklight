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
const inputs = [
	"package-lock.json",
	"apps/media/index.html",
	"apps/media/package.json",
	"apps/media/tsconfig.json",
	"apps/media/vite.config.ts",
	"apps/media/src",
	"apps/ui-library/package.json",
	"apps/ui-library/src",
];
const signaturePath = path.join(
	artifactPaths.root,
	"cache",
	"frontend",
	"media-signature",
);
const outputEntry = path.join(artifactPaths.mediaFrontend, "index.html");

function hashPath(hash, relativePath) {
	const absolutePath = path.join(repositoryRoot, relativePath);
	const stat = fs.lstatSync(absolutePath);
	if (stat.isDirectory()) {
		for (const entry of fs.readdirSync(absolutePath).sort()) {
			hashPath(hash, path.join(relativePath, entry));
		}
		return;
	}
	hash.update(`${relativePath}\0`);
	if (stat.isSymbolicLink()) {
		hash.update(`link\0${fs.readlinkSync(absolutePath)}\0`);
		return;
	}
	hash.update(fs.readFileSync(absolutePath));
	hash.update("\0");
}

const hash = createHash("sha256");
for (const input of inputs) hashPath(hash, input);
const signature = hash.digest("hex");
const recorded = fs.existsSync(signaturePath)
	? fs.readFileSync(signaturePath, "utf8").trim()
	: "";

if (
	process.env.LIGHT_FORCE_FRONTEND_BUILD !== "1" &&
	fs.existsSync(outputEntry) &&
	recorded === signature
) {
	console.log(
		"Media administration interface inputs are unchanged; reusing the existing bundle.",
	);
	process.exit(0);
}

console.log("Building the Media administration interface the server embeds...");
const build = spawnSync("npm", ["run", "build"], {
	cwd: path.join(repositoryRoot, "apps", "media"),
	env: process.env,
	stdio: "inherit",
});
if (build.error) {
	console.error(
		`error: failed to start the Media interface build: ${build.error.message}`,
	);
	process.exit(1);
}
if (build.status !== 0) process.exit(build.status ?? 1);

fs.mkdirSync(path.dirname(signaturePath), { recursive: true });
const temporaryStamp = `${signaturePath}.${process.pid}.tmp`;
fs.writeFileSync(temporaryStamp, `${signature}\n`);
fs.renameSync(temporaryStamp, signaturePath);
