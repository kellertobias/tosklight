#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { artifactPaths } from "./artifact-paths.mjs";

const application = process.argv[2];
const destination = process.argv[3];
const releaseVersion = process.env.LIGHT_RELEASE_VERSION;
const frontendDist = {
  control: artifactPaths.controlFrontend,
  hardware: artifactPaths.hardwareFrontend,
  "viz-editor": artifactPaths.vizEditorFrontend,
}[application];

if (!frontendDist || !destination) {
  console.error("usage: write-tauri-artifact-config.mjs {control|hardware|viz-editor} OUTPUT");
  process.exit(2);
}
if (
  releaseVersion &&
  !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/u.test(releaseVersion)
) {
  console.error(`error: LIGHT_RELEASE_VERSION is not valid SemVer: ${releaseVersion}`);
  process.exit(2);
}
const config = {
  ...(releaseVersion ? { version: releaseVersion } : {}),
  build: { frontendDist },
};
fs.mkdirSync(path.dirname(destination), { recursive: true });
fs.writeFileSync(destination, `${JSON.stringify(config, null, 2)}\n`);
