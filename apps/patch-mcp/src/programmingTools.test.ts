import { readFile } from "node:fs/promises";
import { inflateSync } from "node:zlib";
import { describe, expect, it } from "vitest";
import { commandRisk, type StoredObjects } from "./commandSafety";
import { snapshotImage, xyzToRgb } from "./programmingSnapshot";
import { tools } from "./tools";

const stored: StoredObjects = {
	cues: new Map([[4, new Set(["1", "2"])]]),
	groups: new Set(["3"]),
	presets: new Set(["color:7"]),
};

describe("command safety", () => {
	it("lets selection, values and new records through", () => {
		for (const command of ["GROUP 3 AT 50", "FIXTURE 1 THRU 4 AT POSITION 30 DIV 10", "RECORD CUELIST 4", "RECORD CUELIST 4 CUE 3 TIME 2", "RECORD GROUP 9", "RECORD COLOR PRESET 8", "RECORD PBK 1"])
			expect(commandRisk(command, stored), command).toEqual({ destructive: false, reason: null });
	});

	it("holds back overwrites, merges, updates, moves, copies and deletes", () => {
		for (const command of ["RECORD CUELIST 4 CUE 2", "RECORD + CUELIST 4 CUE 1", "RECORD GROUP 3", "FIXTURE 1 RECORD GROUP 3", "RECORD COLOR PRESET 7", "UPDATE CUELIST 4 CUE 1", "DELETE GROUP 3", "MOVE CUE 1 AT CUE 3", "COPY GROUP 3 AT GROUP 9", "RECORD CUE 5"])
			expect(commandRisk(command, stored).destructive, command).toBe(true);
	});
});

describe("snapshot", () => {
	it("encodes a PNG with each fixture coloured and brightened by its output", () => {
		const fixtures = [
			{ fixture_id: "a", fixture_number: 1, name: "One", location: { x: 0, y: 0 } },
			{ fixture_id: "b", fixture_number: 2, name: "Two", location: { x: 4, y: 2 } },
		] as never;
		const { png, summary } = snapshotImage(fixtures, [
			{ fixture_id: "a", attribute: "intensity", value: { kind: "normalized", value: 1 } },
			{ fixture_id: "a", attribute: "color", value: { kind: "color_xyz", value: { x: 0.4124, y: 0.2126, z: 0.0193 } } },
			{ fixture_id: "b", attribute: "intensity", value: { kind: "normalized", value: 0 } },
		]);
		const bytes = Uint8Array.from(atob(png), (character) => character.charCodeAt(0));
		expect([...bytes.slice(0, 8)]).toEqual([137, 80, 78, 71, 13, 10, 26, 10]);
		expect(summary).toEqual([
			{ fixture: "1", x: 0, y: 0, intensity_percent: 100, color: "#ff0000" },
			{ fixture: "2", x: 4, y: 2, intensity_percent: 0, color: null },
		]);
		// The IDAT payload decompresses to height rows of filter byte + RGB.
		const view = new DataView(bytes.buffer);
		const idatLength = view.getUint32(33);
		const raw = inflateSync(bytes.subarray(41, 41 + idatLength));
		expect(raw.length).toBe(560 * (900 * 3 + 1));
	});

	it("converts XYZ white and primaries to sRGB", () => {
		expect(xyzToRgb({ x: 0.9505, y: 1, z: 1.089 })).toEqual([255, 255, 255]);
		expect(xyzToRgb({ x: 0.1805, y: 0.0722, z: 0.9505 })).toEqual([0, 0, 255]);
	});
});

describe("programming tools", () => {
	it("are registered with schemas, and refuse a product without a Programmer", async () => {
		const names = tools.map((tool) => tool.name);
		for (const name of ["get_show_summary", "list_groups", "list_presets", "list_cue_lists", "list_playbacks", "get_programmer", "get_command_line", "run_command", "select", "set_values", "apply_preset", "clear", "record_cue", "update_cue", "playback", "preload", "snapshot"])
			expect(names).toContain(name);
		expect(new Set(names).size).toBe(names.length);
		const architect = { product: "The Architect" } as never;
		await expect(tools.find((tool) => tool.name === "get_programmer")!.run(architect, {})).rejects.toThrow("The Architect cannot program");
	});
});

describe("help", () => {
	it("documents every tool and every argument", async () => {
		const help = await readFile(new URL("../../../docs/help/99-Integration/05-mcp-server.md", import.meta.url).pathname, "utf8");
		for (const tool of tools) {
			expect(help, tool.name).toContain(`#### \`${tool.name}\``);
			const section = help.slice(help.indexOf(`#### \`${tool.name}\``)).split("\n#### ")[0];
			for (const argument of Object.keys(tool.inputSchema.properties ?? {}))
				expect(section, `${tool.name}.${argument}`).toContain(`| \`${argument}\` |`);
		}
	});
});
