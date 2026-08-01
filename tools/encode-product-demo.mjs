#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { access, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { artifactPaths } from "./artifact-paths.mjs";

const directory = path.join(artifactPaths.visual, "product-demo");
const timelinePath = path.join(directory, "product-demo-edit-timeline.json");
const source = path.join(directory, "tosklight-product-demo-raw.webm");
const canonical = path.join(directory, "tosklight-product-demo.webm");
const destination = path.join(directory, "tosklight-product-demo-h265.mp4");
const bitrate = process.env.LIGHT_PRODUCT_DEMO_HEVC_BITRATE ?? "8M";
const maximumBitrate = process.env.LIGHT_PRODUCT_DEMO_HEVC_MAXRATE ?? "12M";
const bufferSize = process.env.LIGHT_PRODUCT_DEMO_HEVC_BUFSIZE ?? "16M";

await access(source).catch(() => {
	throw new Error(`Product-demo WebM source does not exist: ${source}`);
});
await access(timelinePath).catch(() => {
	throw new Error(`Product-demo edit timeline does not exist: ${timelinePath}`);
});
await mkdir(directory, { recursive: true });

const timeline = JSON.parse(await readFile(timelinePath, "utf8"));
if (
	!Number.isInteger(timeline.fps) ||
	timeline.fps <= 0 ||
	!Array.isArray(timeline.sections) ||
	!timeline.sections.length
)
	throw new Error("Product-demo edit timeline is invalid");
const clock = await buildCanonicalClock(timeline);
const filters = timeline.sections.map((section, index) => {
	const sourceDuration = section.sourceEndMillis - section.sourceStartMillis;
	if (!(sourceDuration > 0) || !(section.frames > 0))
		throw new Error(
			`Product-demo section ${section.id ?? index} has an invalid duration`,
		);
	const targetDuration = (section.frames / timeline.fps) * 1_000;
	const speed = targetDuration / sourceDuration;
	return `[0:v]trim=start=${(section.sourceStartMillis / 1_000).toFixed(6)}:end=${(section.sourceEndMillis / 1_000).toFixed(6)},setpts=${speed.toFixed(9)}*(PTS-STARTPTS),fps=${timeline.fps},format=yuv420p[v${index}]`;
});
const transitionFrames = Math.max(0, Number(timeline.transitionFrames) || 0);
if (transitionFrames > 0 && timeline.sections.length > 1) {
	const transitionSeconds = transitionFrames / timeline.fps;
	let elapsedFrames = timeline.sections[0].frames;
	let input = "[v0]";
	for (let index = 1; index < timeline.sections.length; index++) {
		const output = `[x${index}]`;
		const offsetFrames = elapsedFrames - transitionFrames;
		filters.push(
			`${input}[v${index}]xfade=transition=fade:duration=${transitionSeconds.toFixed(6)}:offset=${(offsetFrames / timeline.fps).toFixed(6)}${output}`,
		);
		input = output;
		elapsedFrames += timeline.sections[index].frames - transitionFrames;
	}
	filters.push(
		`${input}trim=duration=${(timeline.totalFrames / timeline.fps).toFixed(6)},setpts=PTS-STARTPTS[editv]`,
	);
} else {
	filters.push(
		`${timeline.sections.map((_, index) => `[v${index}]`).join("")}concat=n=${timeline.sections.length}:v=1:a=0[editv]`,
	);
}
filters.push(
	`[editv]delogo=x=1195:y=920:w=88:h=44:show=0:enable='${clock.delogoEnable}'[clockbase]`,
	`[1:v]fps=${timeline.fps},format=rgba[clock]`,
	`[clockbase][clock]overlay=x=1185:y=915:shortest=1[outv]`,
);

const edit = spawnSync(
	"ffmpeg",
	[
		"-hide_banner",
		"-loglevel",
		"warning",
		"-y",
		"-i",
		source,
		"-framerate",
		"1",
		"-start_number",
		"0",
		"-i",
		path.join(clock.overlayDirectory, "%04d.png"),
		"-filter_complex",
		filters.join(";"),
		"-map",
		"[outv]",
		"-an",
		"-c:v",
		"libvpx-vp9",
		"-crf",
		"28",
		"-b:v",
		"0",
		"-deadline",
		"good",
		"-cpu-used",
		"2",
		canonical,
	],
	{ stdio: "inherit" },
);
if (edit.error?.code === "ENOENT")
	throw new Error("ffmpeg is required to edit the product-demo video");
if (edit.status !== 0) process.exit(edit.status ?? 1);

async function buildCanonicalClock(editTimeline) {
	for (const command of ["magick", "rsvg-convert"]) {
		const available = spawnSync(command, ["--version"], { stdio: "ignore" });
		if (available.error?.code === "ENOENT" || available.status !== 0)
			throw new Error(`${command} is required to render the canonical demo clock`);
	}
	const workDirectory = path.join(directory, ".canonical-clock");
	const sourceDirectory = path.join(workDirectory, "source");
	const overlayDirectory = path.join(workDirectory, "overlay");
	await rm(workDirectory, { recursive: true, force: true });
	await mkdir(sourceDirectory, { recursive: true });
	await mkdir(overlayDirectory, { recursive: true });
	const sourceClock = spawnSync(
		"ffmpeg",
		[
			"-hide_banner",
			"-loglevel",
			"error",
			"-y",
			"-i",
			source,
			"-vf",
			"crop=100:50:1185:915,fps=1",
			"-start_number",
			"0",
			path.join(sourceDirectory, "%04d.png"),
		],
		{ stdio: "inherit" },
	);
	if (sourceClock.error?.code === "ENOENT")
		throw new Error("ffmpeg is required to inspect the product-demo clock");
	if (sourceClock.status !== 0) process.exit(sourceClock.status ?? 1);

	const seconds = Math.ceil(editTimeline.totalFrames / editTimeline.fps);
	const sourceSeconds = Array.from({ length: seconds + 1 }, (_, second) =>
		canonicalSourceSecond(editTimeline, second),
	);
	const inspected = new Map();
	const isVisible = (sourceSecond) => inspected.get(sourceSecond) ?? false;
	for (const sourceSecond of new Set(
		sourceSeconds.flatMap((second) => [second - 1, second, second + 1]),
	)) {
		if (sourceSecond < 0) {
			inspected.set(sourceSecond, false);
			continue;
		}
		const frame = path.join(
			sourceDirectory,
			`${String(sourceSecond).padStart(4, "0")}.png`,
		);
		const measurement = spawnSync(
			"magick",
			[
				frame,
				"-channel",
				"G",
				"-separate",
				"+channel",
				"-threshold",
				"50%",
				"-format",
				"%[fx:mean]",
				"info:",
			],
			{ encoding: "utf8" },
		);
		inspected.set(
			sourceSecond,
			measurement.status === 0 && Number(measurement.stdout) > 0.005,
		);
	}
	const delogoSeconds = sourceSeconds.map((second) => isVisible(second));
	for (let second = 0; second <= seconds; second++) {
		const sourceSecond = sourceSeconds[second];
		const safe =
			isVisible(sourceSecond - 1) &&
			isVisible(sourceSecond) &&
			isVisible(sourceSecond + 1);
		const label = `${String(Math.floor(second / 60)).padStart(2, "0")}:${String(second % 60).padStart(2, "0")}`;
		const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50" viewBox="0 0 100 50"><text x="50" y="33" text-anchor="middle" fill="#79edf7" fill-opacity="${safe ? 1 : 0}" font-family="ui-monospace, SFMono-Regular, Menlo, monospace" font-size="22" font-weight="700">${label}</text></svg>`;
		const rendered = spawnSync("rsvg-convert", [], {
			input: svg,
			maxBuffer: 1024 * 1024,
		});
		if (rendered.status !== 0)
			throw new Error(`Failed to render canonical clock frame ${second}`);
		await writeFile(
			path.join(overlayDirectory, `${String(second).padStart(4, "0")}.png`),
			rendered.stdout,
		);
	}
	return {
		overlayDirectory,
		delogoEnable: enabledRanges(delogoSeconds),
	};
}

function canonicalSourceSecond(editTimeline, targetSecond) {
	const frame = targetSecond * editTimeline.fps;
	const section = [...editTimeline.sections]
		.reverse()
		.find(
			(candidate) =>
				frame >= candidate.targetStartFrame && frame < candidate.targetEndFrame,
		) ?? editTimeline.sections.at(-1);
	const progress = Math.max(
		0,
		Math.min(1, (frame - section.targetStartFrame) / section.frames),
	);
	return Math.floor(
		(section.sourceStartMillis +
			progress * (section.sourceEndMillis - section.sourceStartMillis)) /
			1_000,
	);
}

function enabledRanges(enabled) {
	const ranges = [];
	for (let start = 0; start < enabled.length; start++) {
		if (!enabled[start]) continue;
		let end = start;
		while (end + 1 < enabled.length && enabled[end + 1]) end++;
		ranges.push(`between(t\\,${start}\\,${end + 1})`);
		start = end;
	}
	return ranges.length ? ranges.join("+") : "0";
}

const encoders = spawnSync("ffmpeg", ["-hide_banner", "-encoders"], {
	encoding: "utf8",
});
if (encoders.error?.code === "ENOENT")
	throw new Error("ffmpeg is required to encode the product-demo MP4");
if (encoders.status !== 0) process.exit(encoders.status ?? 1);
if (
	!encoders.stdout.includes("hevc_videotoolbox") &&
	!encoders.stdout.includes("libx265")
)
	throw new Error("ffmpeg has neither hevc_videotoolbox nor libx265 support");

const encode = (encoder) =>
	spawnSync(
		"ffmpeg",
		[
			"-hide_banner",
			"-loglevel",
			"warning",
			"-y",
			"-i",
			canonical,
			"-map",
			"0:v:0",
			"-an",
			"-c:v",
			encoder,
			...(encoder === "hevc_videotoolbox"
				? [
						"-b:v",
						bitrate,
						"-maxrate",
						maximumBitrate,
						"-bufsize",
						bufferSize,
						"-realtime",
						"false",
						"-allow_sw",
						"1",
					]
				: [
						"-preset",
						"fast",
						"-b:v",
						bitrate,
						"-maxrate",
						maximumBitrate,
						"-bufsize",
						bufferSize,
					]),
			"-pix_fmt",
			"yuv420p",
			"-tag:v",
			"hvc1",
			"-movflags",
			"+faststart",
			destination,
		],
		{ stdio: "inherit" },
	);

let encoder = encoders.stdout.includes("hevc_videotoolbox")
	? "hevc_videotoolbox"
	: "libx265";
let ffmpeg = encode(encoder);
if (
	ffmpeg.status !== 0 &&
	encoder === "hevc_videotoolbox" &&
	encoders.stdout.includes("libx265")
) {
	console.warn("VideoToolbox H.265 was unavailable; retrying with libx265.");
	encoder = "libx265";
	ffmpeg = encode(encoder);
}
if (ffmpeg.error?.code === "ENOENT")
	throw new Error("ffmpeg is required to encode the product-demo MP4");
if (ffmpeg.status !== 0) process.exit(ffmpeg.status ?? 1);

console.log(
	`Edited ${timeline.totalFrames} canonical frames at ${timeline.fps} fps: ${canonical}`,
);
console.log(
	`Encoded H.265 product demo at ${bitrate} with ${encoder}: ${destination}`,
);
