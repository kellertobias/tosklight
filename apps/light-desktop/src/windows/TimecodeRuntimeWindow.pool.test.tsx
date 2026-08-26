// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { MouseEvent, ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TimecodeRuntimeWindow } from "./TimecodeRuntimeWindow";

const mocks = vi.hoisted(() => ({
	api: {} as Record<string, unknown>,
	commandText: "",
	resetCommand: vi.fn(),
}));

vi.mock("@tosklight/ui/pools", () => ({
	PoolGrid: ({
		renderSlot,
	}: {
		renderSlot(item: unknown, index: number): unknown;
	}) => <div>{renderSlot(null, 0) as ReactNode}</div>,
	PoolCard: ({
		"aria-label": ariaLabel,
		onClick,
		onContextMenu,
	}: {
		"aria-label": string;
		onClick(): void;
		onContextMenu(event: MouseEvent): void;
	}) => (
		<button
			type="button"
			aria-label={ariaLabel}
			onClick={onClick}
			onContextMenu={onContextMenu}
		>
			{ariaLabel}
		</button>
	),
}));
vi.mock("../features/deskSnapshot/DeskSnapshotState", () => ({
	useActiveShowId: () => "00000000-0000-4000-8000-000000000161",
}));
vi.mock("../features/showObjects/ShowObjectsState", () => ({
	useCueLists: () => [],
	usePlaybackDefinitions: () => [],
}));
vi.mock("../features/showObjects/ShowObjectsView", () => ({
	useShowObjectView: vi.fn(),
}));
vi.mock("../features/timecode/TimecodeActionsContext", () => ({
	useTimecodeActions: () => ({ api: mocks.api }),
}));
vi.mock("../components/control/commandLine/useCommandLineSurface", () => ({
	useCommandLineSurface: () => ({
		text: mocks.commandText,
		read: () => ({ text: mocks.commandText }),
		reset: mocks.resetCommand,
	}),
}));
vi.mock("../components/files/RootConfinedFilePickerButton", () => ({
	RootConfinedFilePickerButton: ({ label }: { label: string }) => (
		<button type="button">{label}</button>
	),
}));

describe("Timecode pool gestures", () => {
	beforeEach(() => {
		mocks.commandText = "";
		mocks.resetCommand.mockReset().mockResolvedValue(undefined);
		mocks.api = {
			objects: vi.fn(async () => ({
				objects: [{ revision: 4, definition: definition() }],
			})),
			runtime: vi.fn(async () => []),
			transportAction: vi.fn(async () => ({
				timecode_id: definition().id,
				revision: 1,
				state: "playing",
				frame: 0,
				duration_frame: 440,
				audio_linked: false,
			})),
			update: vi.fn(),
			create: vi.fn(),
			waveform: vi.fn(),
		};
	});

	it("starts an occupied slot on left click and opens its editor on right click", async () => {
		render(<TimecodeRuntimeWindow />);
		const card = await screen.findByRole("button", { name: "Timecode 1 Song" });
		fireEvent.click(card);
		await waitFor(() =>
			expect(mocks.api.transportAction).toHaveBeenCalledWith(
				"00000000-0000-4000-8000-000000000161",
				definition().id,
				{ type: "go" },
			),
		);
		fireEvent.contextMenu(card);
		expect(
			await screen.findByRole("button", { name: "Settings" }),
		).toBeTruthy();
	});

	it("uses OFF then the actual Timecode tile to stop its transport", async () => {
		mocks.commandText = "OFF";
		mocks.api.transportAction = vi.fn(async () => ({
			timecode_id: definition().id,
			revision: 2,
			state: "stopped",
			frame: 0,
			duration_frame: 440,
			audio_linked: false,
		}));
		render(<TimecodeRuntimeWindow />);

		fireEvent.click(
			await screen.findByRole("button", {
				name: "Turn off Timecode 1 Song",
			}),
		);

		await waitFor(() =>
			expect(mocks.api.transportAction).toHaveBeenCalledWith(
				"00000000-0000-4000-8000-000000000161",
				definition().id,
				{ type: "stop" },
			),
		);
		expect(mocks.resetCommand).toHaveBeenCalledOnce();
	});
});

function definition() {
	return {
		id: "00000000-0000-4000-8000-000000000162",
		number: 1,
		name: "Song",
		duration_frame: 440,
		transport_offset_frame: 0,
		auto_start: false,
		markers: [],
		lanes: [],
	};
}
