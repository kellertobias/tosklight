import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	CueList,
	PlaybackDefinition,
	PlaybackRuntimeProjection,
} from "../../api/types";
import type { PlaybackBankController } from "./playbackFaderBank/controller";
import { playbackFaderDisplay } from "./playbackFaderBank/feedback";
import { PlaybackSlot } from "./playbackFaderBank/PlaybackSlot";

vi.mock(
	"../../features/controlSurfaceInteraction/SetInteractionProvider",
	() => ({
		useSetInteraction: () => null,
	}),
);
vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({
	useActiveShowId: () => "show",
}));
vi.mock("../../features/poolPresentation/poolPresentation", () => ({
	poolSurfaceKey: () => "surface",
	resolveConfiguredPoolPresentation: () => undefined,
	usePoolPresentationConfiguration: () => null,
}));
vi.mock("./playbackFaderBank/slotActions", () => ({
	buildPlaybackActions: () => ({ actions: [], faderActions: [] }),
	createSlotInterceptors: () => ({
		interceptPointer: vi.fn(),
		interceptClick: vi.fn(),
	}),
}));
vi.mock("./playbackFaderBank/SlotControls", () => ({
	PlaybackAssignmentTarget: () => null,
	PlaybackCommandTargetBadge: () => null,
	PlaybackConfigurationTarget: () => null,
	PlaybackOffTarget: () => null,
}));
vi.mock("./playbackFaderBank/ExpandedPlaybackControls", () => ({
	ExpandedPlaybackControls: () => null,
}));

const playback = {
	go_activates: true,
	auto_off: true,
	xfade_millis: 0,
	name: "Dynamic",
	target: {
		type: "dynamic",
		assignment: { last_known_pool_number: 34, fader_mode: "size_and_master" },
	},
	buttons: ["off", "pause", "flash"],
	button_count: 0,
	has_fader: false,
	fader: "master",
} as PlaybackDefinition;

function projection(
	state: "failed" | "off" | "active" | "hidden" | "zero" | "pending",
	missing = 0,
): PlaybackRuntimeProjection {
	return {
		target: "dynamic",
		runtime: {
			state,
			enabled: state !== "off",
			controller_status:
				state === "off" || state === "failed" || state === "pending"
					? "missing"
					: "losing",
			size: 1,
			master: 1,
			fader_value: 1,
			effective_speed_multiplier: 1,
			speed_source: "fixed",
			controller_id: "controller",
			target_count: 1,
			compatible_target_count: 1,
			missing_target_count: missing,
			unpatched_target_count: 0,
			lane_count: 1,
			supported_address_count: 1,
		},
	} as PlaybackRuntimeProjection;
}

const cuePlayback = {
	...playback,
	target: { type: "cue_list", cue_list_id: "cue" },
} as PlaybackDefinition;
const cueList = { id: "cue", name: "Cue", cues: [] } as unknown as CueList;
type CueBinding = "present" | "missing";

function physical(
	hardware: boolean,
	value: PlaybackRuntimeProjection | undefined,
	assigned = true,
	cueBinding?: CueBinding,
	position = 1,
) {
	const controller = {
		hardware,
		state: {},
		runtimeProjections: new Map([[position, value]]),
		heldActions: { releaseSlot: vi.fn() },
		runtimeActions: null,
		activePageNumber: 4,
		playbackDesk: null,
	} as unknown as PlaybackBankController;
	return render(
		<PlaybackSlot
			controller={controller}
			slotData={{
				playback: assigned
					? {
							...(cueBinding ? cuePlayback : playback),
							address: { page: 4, position },
						}
					: null,
				cue: cueBinding === "present" ? cueList : null,
				group: null,
				slot: position,
				row: null,
				rowIndex: 0,
				columnIndex: 0,
				footprint: null,
			}}
		/>,
	).container.querySelector("article")!;
}

afterEach(cleanup);
type Surface = readonly [
	string,
	(
		projection: PlaybackRuntimeProjection | undefined,
		assigned?: boolean,
		cueBinding?: CueBinding,
	) => Element,
];
const surfaces: readonly Surface[] = [
	["software physical", (p, a = true, c) => physical(false, p, a, c)],
	["hardware physical", (p, a = true, c) => physical(true, p, a, c)],
	["pane positions", (p, a = true, c) => physical(false, p, a, c, 401)],
];
for (const [surface, show] of surfaces) {
	describe(`${surface} actual error outline`, () => {
		it("outlines failed Dynamic and missing bound fixture targets", () => {
			expect(show(projection("failed"))).toHaveClass("playback-runtime-error");
			cleanup();
			expect(show(projection("active", 1))).toHaveClass(
				"playback-runtime-error",
			);
		});
		it("outlines an assigned playback whose authoritative target is missing", () => {
			expect(
				show({ target: "missing" } as PlaybackRuntimeProjection),
			).toHaveClass("playback-runtime-error");
		});
		it("keeps idle missing controller, inactive runtime, hidden and zero normal", () => {
			for (const value of [
				projection("off"),
				projection("pending"),
				projection("hidden"),
				projection("zero"),
				{ target: "dynamic", runtime: null } as PlaybackRuntimeProjection,
				undefined,
			]) {
				expect(show(value)).not.toHaveClass("playback-runtime-error");
				cleanup();
			}
		});
		it("keeps empty slots normal even when a stale missing projection exists", () => {
			expect(
				show({ target: "missing" } as PlaybackRuntimeProjection, false),
			).not.toHaveClass("playback-runtime-error");
		});
		it("distinguishes a missing Cuelist binding from an idle existing Cuelist", () => {
			const idle = {
				target: "cue_list",
				cue_list_id: "cue",
				runtime: null,
			} as PlaybackRuntimeProjection;
			expect(show(idle, true, "missing")).toHaveClass("playback-runtime-error");
			cleanup();
			expect(show(idle, true, "present")).not.toHaveClass(
				"playback-runtime-error",
			);
		});
		it("keeps unpatched targets normal while the Dynamic runs", () => {
			const value = projection("active");
			if (value.target === "dynamic" && value.runtime)
				value.runtime.unpatched_target_count = 1;
			expect(show(value)).not.toHaveClass("playback-runtime-error");
		});
	});
}

describe("Dynamic controller text", () => {
	it("does not call the normal Off controller missing, but retains failed diagnostics", () => {
		const off = playbackFaderDisplay(playback, undefined, 0, projection("off"));
		expect(off).toMatch(/^OFF · Size/);
		expect(off).not.toContain("MISSING");
		const failed = playbackFaderDisplay(
			playback,
			undefined,
			100,
			projection("failed"),
		);
		expect(failed).toMatch(/^FAILED · MISSING/);
	});
});
