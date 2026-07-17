#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { artifactPaths } from "./artifact-paths.mjs";

const application = process.argv[2];
const destination = process.argv[3];
const frontendDist = application === "control"
  ? artifactPaths.controlFrontend
  : application === "hardware"
    ? artifactPaths.hardwareFrontend
    : undefined;

if (!frontendDist || !destination) {
  console.error("usage: write-tauri-artifact-config.mjs {control|hardware} OUTPUT");
  process.exit(2);
}
fs.mkdirSync(path.dirname(destination), { recursive: true });
fs.writeFileSync(destination, `${JSON.stringify({ build: { frontendDist } }, null, 2)}\n`);
