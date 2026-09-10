#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";

const channelsOutput = fileURLToPath(
	new URL(
		"../assets/media-personalities/magicq/ToskLight Pixel Layer Channels.csv",
		import.meta.url,
	),
);
const rangesOutput = fileURLToPath(
	new URL(
		"../assets/media-personalities/magicq/ToskLight Pixel Layer Ranges.csv",
		import.meta.url,
	),
);
const hedOutput = fileURLToPath(
	new URL(
		"../assets/media-personalities/magicq/ToskLight Pixel Layer.hed",
		import.meta.url,
	),
);

// MagicQ obfuscates and reflows HED records when any channel changes, so field
// offsets are not stable. Pin the complete native file instead: this covers all
// 512 Index ranges and the hidden Playback BPM fine byte together.
const hedSha256 = "ff6db9b3f023a4056442d879ee199e6a943f810f0d4cda31f924427ca0569156";

// MagicQ's documented channel-import schema. This is the editable source for the
// console-specific attribute and encoder placement; the DMX order remains the
// canonical 39-channel ToskLight Pixel layer footprint.
const rows = [
	"1,Media Folder,LTP,9,B1Y,8 bit,yes,yes,no,0,0,0,,0,no,yes,1",
	"2,Media File,LTP,8,B1X,8 bit,yes,yes,no,0,0,0,,0,no,yes,1",
	"3,Play Mode,LTP,10,B1F,8 bit,yes,no,no,0,0,0,,0,no,yes,1",
	"4,Scale X,LTP,49,P1D,16 bit hi,no,yes,no,128,128,0,,0,no,yes,1",
	"5,Scale X Fine,LTP,49,P1D,16 bit lo,no,no,no,0,0,0,,0,no,yes,1",
	"6,Scale Y,LTP,48,P1C,16 bit hi,no,yes,no,128,128,0,,0,no,yes,1",
	"7,Scale Y Fine,LTP,48,P1C,16 bit lo,no,no,no,0,0,0,,0,no,yes,1",
	"8,Scale Mode,LTP,51,P1F,8 bit,yes,no,no,0,0,0,,0,no,yes,1",
	"9,Pos X,LTP,4,P1X,16 bit hi,no,yes,no,128,128,0,,0,no,yes,1",
	"10,Pos X Fine,LTP,4,P1X,16 bit lo,no,no,no,0,0,0,,0,no,yes,1",
	"11,Pos Y,LTP,5,P1Y,16 bit hi,no,yes,no,128,128,0,,0,no,yes,1",
	"12,Pos Y Fine,LTP,5,P1Y,16 bit lo,no,no,no,0,0,0,,0,no,yes,1",
	"13,Rotation,LTP,50,P1E,16 bit hi,no,yes,no,128,128,0,,0,no,yes,1",
	"14,Rotation Fine,LTP,50,P1E,16 bit lo,no,no,no,0,0,0,,0,no,yes,1",
	"15,Dimmer,HTP,0,I1X,8 bit,no,yes,no,255,0,255,,0,no,yes,1",
	"16,Volume,LTP,1,I1Y,8 bit,no,no,no,255,255,0,,0,no,yes,1",
	"17,Cyan,LTP,16,C1E,8 bit,no,no,no,0,0,0,,0,no,yes,1",
	"18,Magenta,LTP,17,C1F,8 bit,no,no,no,0,0,0,,0,no,yes,1",
	"19,Yellow,LTP,18,C1Y,8 bit,no,no,no,0,0,0,,0,no,yes,1",
	"20,Greyscale,LTP,19,C1X,8 bit,no,no,no,0,0,0,,0,no,yes,1",
	"21,Mask Folder,LTP,58,B5Y,8 bit,yes,yes,no,0,0,0,,0,no,yes,1",
	"22,Mask File,LTP,59,B5X,8 bit,yes,yes,no,0,0,0,,0,no,yes,1",
	"23,Mask Scale X,LTP,54,B5C,16 bit hi,no,yes,no,128,128,0,,0,no,yes,1",
	"24,Mask Scale X F,LTP,54,B5C,16 bit lo,no,no,no,0,0,0,,0,no,yes,1",
	"25,Mask Scale Y,LTP,55,B5D,16 bit hi,no,yes,no,128,128,0,,0,no,yes,1",
	"26,Mask Scale Y F,LTP,55,B5D,16 bit lo,no,no,no,0,0,0,,0,no,yes,1",
	"27,Mask Invert,LTP,56,B5E,8 bit,yes,no,no,0,0,0,,0,no,yes,1",
	"28,Mask Opacity,LTP,57,B5F,8 bit,no,no,no,0,0,0,,0,no,yes,1",
	"29,FX1 Select,LTP,28,B2X,8 bit,yes,yes,no,0,0,0,,0,no,yes,1",
	"30,FX1 Parameter,LTP,29,B2Y,8 bit,no,no,no,0,0,0,,0,no,yes,1",
	"31,FX2 Select,LTP,36,B3X,8 bit,yes,yes,no,0,0,0,,0,no,yes,1",
	"32,FX2 Parameter,LTP,37,B3Y,8 bit,no,no,no,0,0,0,,0,no,yes,1",
	"33,Speed Multiplr,LTP,11,B1E,8 bit,no,yes,no,127,127,0,,0,no,yes,1",
	"34,Playback BPM,LTP,13,B1D,16 bit hi,no,yes,no,0,0,0,,0,no,yes,1",
	// The obsolete wire byte is the hidden low half of Playback BPM. MagicQ's Reserved attribute
	// fills every otherwise-empty Media encoder with "Reserved"; a fine byte preserves the
	// 39-channel footprint without creating another visible control.
	"35,Playback BPM F,LTP,13,B1D,16 bit lo,no,no,no,0,0,0,,0,no,yes,1",
	"36,Mask Position X,LTP,52,B5A,16 bit hi,no,yes,no,128,128,0,,0,no,yes,1",
	"37,Mask Pos X Fine,LTP,52,B5A,16 bit lo,no,no,no,0,0,0,,0,no,yes,1",
	"38,Mask Position Y,LTP,53,B5B,16 bit hi,no,yes,no,128,128,0,,0,no,yes,1",
	"39,Mask Pos Y Fine,LTP,53,B5B,16 bit lo,no,no,no,0,0,0,,0,no,yes,1",
];

const channelsCsv = `${rows.join("\n")}\n`;
// Gobo1/Gobo2 are the MagicQ attributes which bind Media File/Folder to the
// Media window. One-value ranges provide the exact 000-255 labels used by the
// installable HED. MagicQ's CSV importer does not expose the HED range-type
// field, so the packaged HED additionally stores these ranges as Index.
const rangesCsv = `${[1, 2]
	.flatMap((channel) =>
		Array.from({ length: 256 }, (_, value) =>
			[
				channel,
				`${channel === 1 ? "Folder" : "File"} ${String(value).padStart(3, "0")}`,
				value,
				value,
				0,
				0,
				"",
				"",
			].join(","),
		),
	)
	.join("\n")}\n`;
const check = process.argv.includes("--check");

if (check) {
	const currentChannels = await readFile(channelsOutput, "utf8").catch(() => "");
	const currentRanges = await readFile(rangesOutput, "utf8").catch(() => "");
	const currentHed = await readFile(hedOutput).catch(() => Buffer.alloc(0));
	const hedHasIndexedSelectors =
		createHash("sha256").update(currentHed).digest("hex") === hedSha256;
	if (
		currentChannels !== channelsCsv ||
		currentRanges !== rangesCsv ||
		!hedHasIndexedSelectors
	) {
		throw new Error(
			"MagicQ personality assets are stale; regenerate the CSVs and preserve the HED Index range types",
		);
	}
	console.log("MagicQ personality CSVs and indexed HED are current.");
} else {
	await Promise.all([
		writeFile(channelsOutput, channelsCsv),
		writeFile(rangesOutput, rangesCsv),
	]);
	console.log(`Generated ${channelsOutput}`);
	console.log(`Generated ${rangesOutput}`);
}
