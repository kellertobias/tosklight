import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CueList, PlaybackDefinition, PlaybackPage } from "../../../api/types";
import type { SingleCuePreview } from "./useSingleCuePreviews";
import { singleCueCandidates } from "./useSingleCuePreviews";
import { VirtualPlaybackGrid } from "./VirtualPlaybackGrid";

const mocks = vi.hoisted(() => ({
	previews: new Map<number, SingleCuePreview>(),
}));

vi.mock("./useSingleCuePreviews", async (importOriginal) => ({
	...(await importOriginal<Record<string, unknown>>()),
	useSingleCuePreviews: () => mocks.previews,
}));
vi.mock("../../../features/controlSurfaceInteraction/SetInteractionProvider", () => ({
	useSetInteraction: () => null,
}));
vi.mock("../../../features/deskSnapshot/DeskSnapshotState", () => ({
	useActiveShowId: () => "show-a",
}));
vi.mock("../../../features/poolPresentation/poolPresentation", () => ({
	poolSurfaceKey: () => "surface",
	resolveConfiguredPoolPresentation: () => undefined,
	usePoolPresentationConfiguration: () => null,
}));

function playback(
	number: number,
	cueListId: string,
	overrides: Partial<PlaybackDefinition> = {},
): PlaybackDefinition {
	return {
		number,
		name: `Media ${number}`,
		target: { type: "cue_list", cue_list_id: cueListId },
		buttons: ["go", "none", "none"],
		button_count: 1,
		fader: "master",
		has_fader: false,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		...overrides,
	};
}

function cueList(id: string, cueIds: string[]): CueList {
	return {
		id,
		name: id,
		priority: 0,
		mode: "sequence",
		looped: false,
		cues: cueIds.map((cueId, index) => ({
			id: cueId,
			number: String(index + 1),
			name: "",
			fade_millis: 0,
			delay_millis: 0,
			trigger: { type: "manual" },
			changes: [],
		})),
	} as unknown as CueList;
}

function page(playbacks: PlaybackDefinition[]): PlaybackPage {
	return {
		number: 1,
		name: "Page 1",
		slots: {},
		virtual_playbacks: Object.fromEntries(
			playbacks.map((entry) => [String(entry.number), entry]),
		),
	};
}

function grid(pageValue: PlaybackPage, columns: number) {
	return render(
		<VirtualPlaybackGrid
			pageNumber={1}
			page={pageValue}
			pageObjectId="page-one"
			pageObjectRevision={1}
			rows={1}
			columns={columns}
			playbacks={new Map()}
			cueLists={new Map()}
			runtimes={new Map()}
			runtimeActions={null}
			zones={[]}
			selectedSlots={[]}
			configurationArmed={false}
			updateArmed={false}
			shiftArmed={false}
			onConfigure={vi.fn()}
			onToggleZone={vi.fn()}
		/>,
	);
}

afterEach(() => {
	cleanup();
	mocks.previews = new Map();
});

describe("single-Cue Virtual Playback default image", () => {
	it("qualifies only an unconfigured Cuelist with exactly one Cue", () => {
		const lists = new Map([
			["one", cueList("one", ["cue-a"])],
			["two", cueList("two", ["cue-b", "cue-c"])],
			["none", cueList("none", [])],
			["iconed", cueList("iconed", ["cue-d"])],
			["imaged", cueList("imaged", ["cue-e"])],
		]);
		const candidates = singleCueCandidates(
			page([
				playback(1001, "one"),
				playback(1002, "two"),
				playback(1003, "none"),
				playback(1004, "iconed", { presentation_icon: "☀" }),
				playback(1005, "imaged", {
					presentation_image: "data:image/png;base64,AAAA",
				}),
				playback(1006, "missing"),
				{
					...playback(1007, "one"),
					target: { type: "group", group_id: "1" },
				},
			]),
			lists,
		);
		expect([...candidates.entries()].map(([number, cue]) => [number, cue.id])).toEqual([
			[1001, "cue-a"],
		]);
	});

	it("shows each tile's own Cue preview in a grid, keeps configured images, and names fallbacks", () => {
		mocks.previews = new Map([
			[1001, { cueId: "cue-a", src: "blob:program-a", transparent: false }],
			[1002, { cueId: "cue-b", src: "blob:layer-b", transparent: true }],
			[
				1003,
				{ cueId: "cue-c", transparent: false, notice: "Media Server offline" },
			],
			[
				1004,
				{
					cueId: "cue-d",
					src: "blob:empty-d",
					transparent: false,
					notice: "Empty media",
				},
			],
		]);
		const view = grid(
			page([
				playback(1001, "a"),
				playback(1002, "b"),
				playback(1003, "c"),
				playback(1004, "d"),
				playback(1005, "e", {
					presentation_image: "data:image/png;base64,CONFIGURED",
				}),
			]),
			5,
		);
		const tile = (number: number) =>
			view.container.querySelector<HTMLElement>(
				`[data-virtual-playback-number="${number}"]`,
			)!;

		expect(tile(1001)).toHaveAttribute("data-image-source", "cue-preview");
		expect(tile(1001).querySelector("img")).toHaveAttribute(
			"src",
			"blob:program-a",
		);
		expect(tile(1001).querySelector("img")).toHaveAttribute(
			"alt",
			"Media 1001 Cue preview",
		);
		expect(tile(1002).querySelector("img")).toHaveAttribute("src", "blob:layer-b");
		expect(tile(1002)).toHaveClass("cue-preview-transparent");
		expect(tile(1001)).not.toHaveClass("cue-preview-transparent");

		expect(tile(1003).querySelector("img")).toBeNull();
		expect(tile(1003)).toHaveAttribute("data-preview-notice", "Media Server offline");
		expect(tile(1003)).toHaveTextContent("Media Server offline");

		expect(tile(1004)).toHaveAttribute("data-preview-notice", "Empty media");
		expect(tile(1004).querySelector("img")).toHaveAttribute("src", "blob:empty-d");

		expect(tile(1005)).toHaveAttribute("data-image-source", "configured");
		expect(tile(1005).querySelector("img")).toHaveAttribute(
			"src",
			"data:image/png;base64,CONFIGURED",
		);
		expect(tile(1005).querySelector("img")).toHaveAttribute(
			"alt",
			"Media 1005 artwork",
		);
	});

	it("never lets an automatic preview replace a configured image", () => {
		mocks.previews = new Map([
			[1001, { cueId: "cue-a", src: "blob:auto", transparent: true, notice: "Empty media" }],
		]);
		const view = grid(
			page([
				playback(1001, "a", {
					presentation_image: "data:image/png;base64,MINE",
				}),
			]),
			1,
		);
		const tile = view.container.querySelector<HTMLElement>(
			'[data-virtual-playback-number="1001"]',
		)!;
		expect(tile.querySelector("img")).toHaveAttribute(
			"src",
			"data:image/png;base64,MINE",
		);
		expect(tile).toHaveAttribute("data-image-source", "configured");
		expect(tile).not.toHaveAttribute("data-preview-notice");
		expect(tile).not.toHaveClass("cue-preview-transparent");
	});
});
