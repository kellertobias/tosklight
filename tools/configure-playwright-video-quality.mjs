#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { createRequire } from "node:module";
import process from "node:process";
import { repositoryRoot } from "./artifact-paths.mjs";

const require = createRequire(path.join(repositoryRoot, "apps/control-ui/package.json"));
const playwrightPackage = require.resolve("playwright-core/package.json");
const coreBundle = path.join(path.dirname(playwrightPackage), "lib/coreBundle.js");
const bitrate = process.env.LIGHT_PLAYWRIGHT_VIDEO_BITRATE ?? "12M";
const speed = process.env.LIGHT_PLAYWRIGHT_VIDEO_SPEED ?? "4";
const threads = process.env.LIGHT_PLAYWRIGHT_VIDEO_THREADS ?? "4";
const videoArguments = /-c:v vp8 -qmin 0 -qmax \d+ -crf \d+ -deadline realtime -speed \d+ -b:v \S+ -threads \d+/g;
const replacement = `-c:v vp8 -qmin 0 -qmax 24 -crf 4 -deadline realtime -speed ${speed} -b:v ${bitrate} -threads ${threads}`;

const source = await readFile(coreBundle, "utf8");
const matches = [...source.matchAll(videoArguments)];
if (matches.length !== 1) {
  throw new Error(`Expected one Playwright VP8 recorder command in ${coreBundle}, found ${matches.length}`);
}
if (matches[0][0] === replacement) {
  console.log(`Playwright visual recording already configured for ${bitrate}`);
  process.exit(0);
}
await writeFile(coreBundle, source.replace(videoArguments, replacement));
console.log(`Configured Playwright visual recording source for ${bitrate} VP8 (${threads} threads, speed ${speed})`);
