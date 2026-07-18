#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { access, mkdir } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { artifactPaths } from "./artifact-paths.mjs";

const directory = path.join(artifactPaths.visual, "product-demo");
const source = path.join(directory, "tosklight-product-demo.webm");
const destination = path.join(directory, "tosklight-product-demo-h265.mp4");
const bitrate = process.env.LIGHT_PRODUCT_DEMO_HEVC_BITRATE ?? "8M";
const maximumBitrate = process.env.LIGHT_PRODUCT_DEMO_HEVC_MAXRATE ?? "12M";
const bufferSize = process.env.LIGHT_PRODUCT_DEMO_HEVC_BUFSIZE ?? "16M";

await access(source).catch(() => {
  throw new Error(`Product-demo WebM source does not exist: ${source}`);
});
await mkdir(directory, { recursive: true });

const encoders = spawnSync("ffmpeg", ["-hide_banner", "-encoders"], { encoding: "utf8" });
if (encoders.error?.code === "ENOENT") throw new Error("ffmpeg is required to encode the product-demo MP4");
if (encoders.status !== 0) process.exit(encoders.status ?? 1);
if (!encoders.stdout.includes("hevc_videotoolbox") && !encoders.stdout.includes("libx265"))
  throw new Error("ffmpeg has neither hevc_videotoolbox nor libx265 support");

const encode = (encoder) => spawnSync("ffmpeg", [
  "-hide_banner", "-loglevel", "warning", "-y",
  "-i", source,
  "-map", "0:v:0", "-an",
  "-c:v", encoder,
  ...(encoder === "hevc_videotoolbox"
    ? ["-b:v", bitrate, "-maxrate", maximumBitrate, "-bufsize", bufferSize, "-realtime", "false", "-allow_sw", "1"]
    : ["-preset", "fast", "-b:v", bitrate, "-maxrate", maximumBitrate, "-bufsize", bufferSize]),
  "-pix_fmt", "yuv420p",
  "-tag:v", "hvc1",
  "-movflags", "+faststart",
  destination,
], { stdio: "inherit" });

let encoder = encoders.stdout.includes("hevc_videotoolbox") ? "hevc_videotoolbox" : "libx265";
let ffmpeg = encode(encoder);
if (ffmpeg.status !== 0 && encoder === "hevc_videotoolbox" && encoders.stdout.includes("libx265")) {
  console.warn("VideoToolbox H.265 was unavailable; retrying with libx265.");
  encoder = "libx265";
  ffmpeg = encode(encoder);
}
if (ffmpeg.error?.code === "ENOENT") throw new Error("ffmpeg is required to encode the product-demo MP4");
if (ffmpeg.status !== 0) process.exit(ffmpeg.status ?? 1);

console.log(`Encoded H.265 product demo at ${bitrate} with ${encoder}: ${destination}`);
