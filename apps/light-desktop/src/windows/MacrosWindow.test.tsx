import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
	publishObjectEditorRequest,
	resetObjectEditorRequestsForTests,
} from "../features/controlSurfaceInteraction/objectEditorRequest";

const mocks = vi.hoisted(() => ({
	command: {
		text: "COPY",
		read: vi.fn(() => ({ text: "COPY" })),
		replace: vi.fn().mockResolvedValue(undefined),
		reset: vi.fn().mockResolvedValue(undefined),
	},
	objects: vi.fn(),
	runtime: vi.fn(),
	copy: vi.fn(),
	cancel: vi.fn(),
}));

vi.mock("../components/control/commandLine/useCommandLineSurface", () => ({
	useCommandLineSurface: () => mocks.command,
}));
vi.mock("../features/deskSnapshot/DeskSnapshotState", () => ({
	useActiveShowId: () => "show-a",
}));
vi.mock("../features/macros/MacroActionsContext", () => ({
	useMacroActions: () => ({
		macros: {
			runtime: mocks.runtime,
			copy: mocks.copy,
			cancel: mocks.cancel,
			run: vi.fn(),
		},
		showObjects: { objects: mocks.objects },
		events: { onEvent: vi.fn(() => vi.fn()) },
	}),
}));
vi.mock("../features/macros/MacroEditor", () => ({
	MacroEditor: ({ macro }: { macro: { id: string } }) => (
		<div>Editing Macro {macro.id}</div>
	),
}));

const { MacrosWindow } = await import("./MacrosWindow");

const existing = {
	id: "00000000-0000-4000-8000-000000000001",
	revision: 4,
	updated_at: "2026-08-11T12:00:00Z",
	body: {
		id: "00000000-0000-4000-8000-000000000001",
		number: 1,
		name: "Front wash",
		source: "FIXTURE 1 AT 50",
		presentation: { color: "#315cab", icon: "play" },
	},
};

beforeEach(() => {
	mocks.command.text = "COPY";
	mocks.command.read.mockImplementation(() => ({ text: mocks.command.text }));
	mocks.command.replace.mockClear();
	mocks.command.reset.mockClear();
	mocks.objects.mockResolvedValue([existing]);
	mocks.runtime.mockResolvedValue({ active: [], recent: [] });
	mocks.copy.mockResolvedValue({});
	mocks.cancel.mockReset().mockResolvedValue({
		execution_id: "execution-1",
		macro_id: existing.id,
		state: "cancelled",
	});
});

afterEach(() => {
	cleanup();
	resetObjectEditorRequestsForTests();
});

describe("MacrosWindow external Copy command", () => {
	it("uses OFF then the actual Macro tile to cancel its running execution", async () => {
		mocks.command.text = "OFF";
		mocks.runtime.mockResolvedValue({
			active: [
				{
					execution_id: "execution-1",
					macro_id: existing.id,
					state: "running",
				},
			],
			recent: [],
		});
		render(<MacrosWindow />);

		fireEvent.click(
			await screen.findByRole("button", {
				name: "Cancel Macro 1 Front wash",
			}),
		);

		await waitFor(() =>
			expect(mocks.cancel).toHaveBeenCalledWith("show-a", "execution-1"),
		);
		expect(mocks.command.reset).toHaveBeenCalledOnce();
		expect(mocks.copy).not.toHaveBeenCalled();
	});

	it("consumes OFF without a cancel request when the Macro is already stopped", async () => {
		mocks.command.text = "OFF";
		mocks.runtime.mockResolvedValue({ active: [], recent: [] });
		render(<MacrosWindow />);

		fireEvent.click(
			await screen.findByRole("button", {
				name: "Cancel Macro 1 Front wash",
			}),
		);

		await waitFor(() => expect(mocks.command.reset).toHaveBeenCalledOnce());
		expect(mocks.cancel).not.toHaveBeenCalled();
	});

	it("addresses an occupied source card without opening the editor", async () => {
		render(<MacrosWindow />);
		const source = await screen.findByRole("button", {
			name: "Macro 1 Front wash",
		});

		fireEvent.click(source);

		expect(mocks.command.replace).toHaveBeenCalledWith("COPY MACRO 1 AT");
	});

	it("copies the saved Macro into an empty destination and resets the command", async () => {
		mocks.command.text = "COPY MACRO 1 AT";
		render(<MacrosWindow />);
		const destination = await screen.findByRole("button", {
			name: "Empty Macro 2",
		});

		fireEvent.click(destination);
		fireEvent.click(destination);

		await waitFor(() => expect(mocks.copy).toHaveBeenCalledOnce());
		expect(mocks.copy).toHaveBeenCalledWith(
			"show-a",
			existing.id,
			existing.revision,
			2,
		);
		expect(mocks.command.reset).toHaveBeenCalledOnce();
	});

	it("opens the exact Macro requested by the shared command surface", async () => {
		render(<MacrosWindow />);
		await screen.findByRole("button", { name: "Macro 1 Front wash" });

		publishObjectEditorRequest({ kind: "macro", objectId: existing.id });

		expect(
			await screen.findByText(`Editing Macro ${existing.id}`),
		).toBeVisible();
		expect(mocks.command.reset).not.toHaveBeenCalled();
	});
});
