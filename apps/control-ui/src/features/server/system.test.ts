import { afterEach, describe, expect, it, vi } from "vitest";
import type { CueList, StoredGroup, StoredPreset } from "../../api/types";
import { createSystemActions } from "./system";
import type { ServerController } from "./model";

afterEach(() => vi.restoreAllMocks());

describe("Programmer lifecycle system actions", () => {
	it("clears through scoped events without reloading bootstrap", async () => {
		const client = {
			clearProgrammer: vi.fn().mockResolvedValue(undefined),
			bootstrap: vi.fn(),
		};
		const setSelectedFixtures = vi.fn();
		const setSelectedGroupId = vi.fn();
		const setCommandLineState = vi.fn();
		const setCommandLinePristine = vi.fn();
		const setError = vi.fn();
		const actions = createSystemActions({
			client,
			setError,
			bootstrap: null,
			session: { session_id: "session-a" },
			patch: null,
			playbacks: null,
			commandTargetModeRef: { current: "FIXTURE" },
			setCommandLineState,
			setCommandLinePristine,
			setSelectedFixtures,
			setSelectedGroupId,
		} as unknown as ServerController);

		await actions.clearProgrammer("session-a");

		expect(client.clearProgrammer).toHaveBeenCalledWith("session-a");
		expect(client.bootstrap).not.toHaveBeenCalled();
		expect(setSelectedFixtures).toHaveBeenCalledWith([]);
		expect(setSelectedGroupId).toHaveBeenCalledWith(null);
		expect(setCommandLineState).toHaveBeenCalledWith("FIXTURE");
		expect(setCommandLinePristine).toHaveBeenCalledWith(true);
		expect(setError).toHaveBeenCalledWith(null);
	});
});

describe("paperwork export", () => {
	it("loads ordered portable CueLists with Groups and Presets without reading playbacks", async () => {
		const groups = [versioned("2", group("Second")), versioned("1", group("First"))];
		const presets = [
			versioned("2", preset(2, "Second", { "fixture-b": {} })),
			versioned(
				"1",
				preset(1, "First", { "fixture-a": {}, "fixture-c": {} }),
			),
		];
		const cueLists = [
			versioned("cue-b", cueList("cue-b", "Second")),
			versioned("cue-a", cueList("cue-a", "First")),
		];
		const objects = vi.fn(async (_showId: string, kind: string) => {
			if (kind === "group") return groups;
			if (kind === "preset") return presets;
			if (kind === "cue_list") return cueLists;
			throw new Error(`unexpected kind ${kind}`);
		});
		const download = mockDownload();
		const setError = vi.fn();
		const model = systemModel({
			client: { objects },
			setError,
			bootstrap: { active_show: { id: "show-a", name: "Tour" } },
			patch: { fixtures: ["fixture-a"] },
		});
		Object.defineProperty(model, "playbacks", {
			get: () => {
				throw new Error("paperwork must not read the Playback facade");
			},
		});

		await createSystemActions(
			model as unknown as ServerController,
		).exportPaperwork();

		expect(objects.mock.calls).toEqual([
			["show-a", "group"],
			["show-a", "preset"],
			["show-a", "cue_list"],
		]);
		const payload = await downloadedPayload(download.createObjectURL);
		expect(payload).toMatchObject({
			show: { id: "show-a", name: "Tour" },
			patch: { fixtures: ["fixture-a"] },
			cue_lists: cueLists.map((item) => item.body),
			groups: groups.map((item) => item.body),
			presets: [
				{ id: "2", name: "Second", fixtures: 1 },
				{ id: "1", name: "First", fixtures: 2 },
			],
		});
		expect(download.anchor.download).toBe("Tour-paperwork.json");
		expect(download.click).toHaveBeenCalledOnce();
		expect(download.revokeObjectURL).toHaveBeenCalledWith("blob:paperwork");
		expect(setError).toHaveBeenLastCalledWith(null);
	});

	it("requests no portable collections when no Show is active", async () => {
		const objects = vi.fn();
		const download = mockDownload();
		const model = systemModel({
			client: { objects },
			bootstrap: null,
		});

		await createSystemActions(
			model as unknown as ServerController,
		).exportPaperwork();

		expect(objects).not.toHaveBeenCalled();
		expect(await downloadedPayload(download.createObjectURL)).toMatchObject({
			cue_lists: [],
			groups: [],
			presets: [],
		});
		expect(download.anchor.download).toBe("show-paperwork.json");
	});

	it("reports a CueList load failure without starting a download", async () => {
		const objects = vi.fn(async (_showId: string, kind: string) => {
			if (kind === "cue_list") throw new Error("CueLists unavailable");
			return [];
		});
		const download = mockDownload();
		const setError = vi.fn();
		const model = systemModel({
			client: { objects },
			setError,
			bootstrap: { active_show: { id: "show-a", name: "Tour" } },
		});

		await createSystemActions(
			model as unknown as ServerController,
		).exportPaperwork();

		expect(setError).toHaveBeenLastCalledWith("CueLists unavailable");
		expect(download.createObjectURL).not.toHaveBeenCalled();
		expect(download.click).not.toHaveBeenCalled();
	});
});

function systemModel(overrides: Record<string, unknown>) {
	return {
		client: {},
		setError: vi.fn(),
		bootstrap: null,
		session: null,
		patch: null,
		commandTargetModeRef: { current: "FIXTURE" },
		setCommandLineState: vi.fn(),
		setCommandLinePristine: vi.fn(),
		setSelectedFixtures: vi.fn(),
		setSelectedGroupId: vi.fn(),
		...overrides,
	};
}

function mockDownload() {
	const anchor = document.createElement("a");
	const click = vi.spyOn(anchor, "click").mockImplementation(() => {});
	vi.spyOn(document, "createElement").mockReturnValue(anchor);
	const createObjectURL = vi
		.spyOn(URL, "createObjectURL")
		.mockReturnValue("blob:paperwork");
	const revokeObjectURL = vi.spyOn(URL, "revokeObjectURL");
	return { anchor, click, createObjectURL, revokeObjectURL };
}

async function downloadedPayload(createObjectURL: ReturnType<typeof vi.fn>) {
	const blob = createObjectURL.mock.calls[0][0] as Blob;
	return JSON.parse(await blob.text());
}

function versioned<T>(id: string, body: T) {
	return { id, revision: 1, updated_at: "", body };
}

function group(name: string): StoredGroup {
	return { name, fixtures: [], master: 1 };
}

function preset(
	number: number,
	name: string,
	values: StoredPreset["values"],
): StoredPreset {
	return { number, name, family: "Intensity", values };
}

function cueList(id: string, name: string): CueList {
	return {
		id,
		name,
		cues: [],
		mode: "sequence",
		priority: 0,
		looped: false,
	};
}
