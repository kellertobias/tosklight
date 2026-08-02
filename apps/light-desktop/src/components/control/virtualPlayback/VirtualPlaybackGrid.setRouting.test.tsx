import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PlaybackDefinition, PlaybackPage } from "../../../api/types";
import type { PlaybackRuntimeActions } from "../../../features/playbackRuntime/actionWriter";
import { VirtualPlaybackGrid } from "./VirtualPlaybackGrid";

const mocks = vi.hoisted(() => ({
	choosePlayback: vi.fn(),
}));

vi.mock("@tosklight/ui/playback", () => ({
	VirtualPlaybackGridView: ({
		callbacks,
	}: {
		callbacks: { onAction(slot: number): void };
	}) => (
		<button type="button" onClick={() => callbacks.onAction(1)}>
			Virtual Playback 1
		</button>
	),
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
	buttons: ["select", "none", "none"],
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

	it("routes the exact Page-contained Virtual identity instead of operating it", () => {
		const virtualPlaybackAction = vi.fn();
		const runtimeActions = {
			virtualPlaybackAction,
		} as unknown as PlaybackRuntimeActions;
		render(
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
				configurationArmed
				updateArmed={false}
				shiftArmed={false}
				onConfigure={vi.fn()}
				onToggleZone={vi.fn()}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Virtual Playback 1" }));

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
