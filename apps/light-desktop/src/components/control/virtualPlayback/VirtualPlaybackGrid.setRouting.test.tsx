import { fireEvent, render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PlaybackDefinition, PlaybackPage } from "../../../api/types";
import type { PlaybackRuntimeActions } from "../../../features/playbackRuntime/actionWriter";
import { VirtualPlaybackGrid } from "./VirtualPlaybackGrid";

const mocks = vi.hoisted(() => ({
	choosePlayback: vi.fn(),
}));

vi.mock(
	"../../../features/controlSurfaceInteraction/SetInteractionProvider",
	() => ({
		useSetInteraction: () => ({
			state: { phase: "group_source_pending" },
			choosePlayback: mocks.choosePlayback,
		}),
	}),
);
vi.mock("../../../features/deskSnapshot/DeskSnapshotState", () => ({
	useActiveShowId: () => "show-a",
}));
vi.mock("../../../features/poolPresentation/poolPresentation", () => ({
	poolSurfaceKey: () => "surface",
	resolveConfiguredPoolPresentation: () => undefined,
	usePoolPresentationConfiguration: () => null,
}));

const playback: PlaybackDefinition = {
	number: 1301,
	name: "Virtual Front",
	target: { type: "group", group_id: "4" },
	buttons: ["flash", "none", "none"],
	button_count: 1,
	fader: "master",
	has_fader: false,
	go_activates: true,
	auto_off: true,
	xfade_millis: 0,
};
const page: PlaybackPage = {
	number: 2,
	name: "Page 2",
	slots: {},
	virtual_playbacks: { "1301": playback },
};

describe("Virtual Playback SET routing", () => {
	beforeEach(() => mocks.choosePlayback.mockReset().mockResolvedValue(null));

	it("routes the real held-action pointer path instead of operating it", () => {
		const virtualPlaybackAction = vi.fn();
		const runtimeActions = {
			virtualPlaybackAction,
		} as unknown as PlaybackRuntimeActions;
		const view = render(
			<VirtualPlaybackGrid
				pageNumber={2}
				page={page}
				pageObjectId="page-two"
				pageObjectRevision={7}
				rows={1}
				columns={1}
				playbacks={new Map()}
				cueLists={new Map()}
				runtimes={new Map()}
				runtimeActions={runtimeActions}
				zones={[]}
				selectedSlots={[]}
				configurationArmed={false}
				updateArmed={false}
				shiftArmed={false}
				onConfigure={vi.fn()}
				onToggleZone={vi.fn()}
			/>,
		);

		const card = view.container.querySelector<HTMLElement>(
			'[data-virtual-playback-slot="1"]',
		);
		expect(card).not.toBeNull();
		if (!card) throw new Error("Expected the real Virtual Playback card");
		fireEvent.pointerDown(card, { pointerId: 1 });
		fireEvent.pointerUp(card, { pointerId: 1 });

		expect(mocks.choosePlayback).toHaveBeenCalledWith(
			{
				addressing: "virtual",
				pageNumber: 2,
				playbackNumber: 1301,
				pageObjectId: "page-two",
				pageObjectRevision: 7,
			},
			"touch",
		);
		expect(virtualPlaybackAction).not.toHaveBeenCalled();
	});
});
