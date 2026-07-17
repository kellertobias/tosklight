#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { mkdir, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { artifactPaths } from "./artifact-paths.mjs";

const results = artifactPaths.results;
const destinationDirectory = artifactPaths.visual;
const destination = path.join(destinationDirectory, "light-ui-test-catalog.webm");
const manifest = path.join(destinationDirectory, "catalog-videos.txt");

async function videosIn(directory) {
  const result = [];
  for (const entry of await readdir(directory, { withFileTypes: true }).catch(() => [])) {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) result.push(...await videosIn(absolute));
    else if (entry.isFile() && entry.name === "video.webm") result.push(absolute);
  }
  return result;
}

const videos = (await videosIn(results)).sort();
if (!videos.length) {
  console.error(`No Playwright videos were found below ${results}`);
  process.exit(1);
}

await mkdir(destinationDirectory, { recursive: true });
await writeFile(
  manifest,
  videos.map((video) => `file '${video.replaceAll("'", "'\\''")}'`).join("\n") + "\n",
);

const ffmpeg = spawnSync("ffmpeg", [
  "-hide_banner",
  "-loglevel", "warning",
  "-y",
  "-f", "concat",
  "-safe", "0",
  "-i", manifest,
  "-c", "copy",
  destination,
], { stdio: "inherit" });

if (ffmpeg.error?.code === "ENOENT") {
  console.error(`ffmpeg is required to assemble the catalog reel; the individual videos remain in ${results}.`);
  process.exit(1);
}
if (ffmpeg.status !== 0) process.exit(ffmpeg.status ?? 1);

console.log(`Assembled ${videos.length} UI test videos into ${destination}`);
