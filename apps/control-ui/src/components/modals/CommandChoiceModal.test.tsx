import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProgrammingInteractionViewProvider } from "../../features/programmingInteraction/ProgrammingInteractionView";
import type { CommandLineProjection } from "../../features/programmingInteraction/contracts";
import { ProgrammingInteractionStore } from "../../features/programmingInteraction/store";
import {
	commandLine,
	DESK_ID,
	programmingSnapshot,
	SHOW_ID,
} from "../../features/programmingInteraction/testFixtures";
import { CommandChoiceModal } from "./CommandChoiceModal";

const CHOICE = {
	type: "cue_move_copy" as const,
	operation: "copy" as const,
	command: "COPY SET 1 CUE 2 AT SET 2 CUE 2",
	options: [
		{
			id: "plain" as const,
			label: "Plain Copy",
			command: "COPY PLAIN SET 1 CUE 2 AT SET 2 CUE 2",
		},
		{
			id: "status" as const,
			label: "Status Copy",
			command: "COPY STATUS SET 1 CUE 2 AT SET 2 CUE 2",
		},
	],
	cancelLabel: "Cancel",
};

function pendingCommandLine(revision = 1) {
	return { ...commandLine(revision, CHOICE.command), pendingChoice: CHOICE };
}

function renderScopedChoice({
	loadSnapshot = async () =>
		programmingSnapshot({ command: pendingCommandLine() }),
	replaceCommandLine = vi
		.fn<
			(
				deskId: string,
				text: string,
				expectedRevision: number,
			) => Promise<CommandLineProjection>
		>()
		.mockResolvedValue(commandLine(2)),
}: {
	loadSnapshot?: () => Promise<ReturnType<typeof programmingSnapshot>>;
	replaceCommandLine?: ReturnType<
		typeof vi.fn<
			(
				deskId: string,
				text: string,
				expectedRevision: number,
			) => Promise<CommandLineProjection>
		>
	>;
} = {}) {
	const store = new ProgrammingInteractionStore();
	return {
		store,
		replaceCommandLine,
		...render(
			<ProgrammingInteractionViewProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				store={store}
				transport={null}
				loadSnapshot={loadSnapshot}
				replaceCommandLine={replaceCommandLine}
			>
				<CommandChoiceModal />
			</ProgrammingInteractionViewProvider>,
		),
	};
}

const server = {
  pendingCommandChoice: {
    type: "cue_move_copy" as const,
    operation: "copy" as const,
    command: "COPY SET 1 CUE 2 AT SET 2 CUE 2",
    options: [
      { id: "plain", label: "Plain Copy", command: "COPY PLAIN SET 1 CUE 2 AT SET 2 CUE 2" },
      { id: "status", label: "Status Copy", command: "COPY STATUS SET 1 CUE 2 AT SET 2 CUE 2" },
    ],
    cancel_label: "Cancel",
  } as null | {
    type: "cue_move_copy";
    operation: "copy" | "move";
    command: string;
    options: Array<{ id: string; label: string; command: string }>;
    cancel_label: string;
  },
  executeCommandLine: vi.fn().mockResolvedValue(true),
	dismissCommandChoice: vi.fn(() => {
		server.pendingCommandChoice = null;
	}),
  cancelCommandChoice: vi.fn(),
};

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  server.pendingCommandChoice = {
    type: "cue_move_copy",
    operation: "copy",
    command: "COPY SET 1 CUE 2 AT SET 2 CUE 2",
    options: [
      { id: "plain", label: "Plain Copy", command: "COPY PLAIN SET 1 CUE 2 AT SET 2 CUE 2" },
      { id: "status", label: "Status Copy", command: "COPY STATUS SET 1 CUE 2 AT SET 2 CUE 2" },
    ],
    cancel_label: "Cancel",
  };
});

describe("CommandChoiceModal", () => {
  it("renders only the authoritative Plain, Status, and Cancel choices", async () => {
	renderScopedChoice();

	await screen.findByRole("dialog", { name: "Cue Copy choice" });
    expect(screen.getByRole("button", { name: "Plain Copy" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Status Copy" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Move/ })).not.toBeInTheDocument();
  });

  it("executes the selected explicit command", async () => {
	renderScopedChoice();
	await screen.findByRole("dialog", { name: "Cue Copy choice" });
    fireEvent.click(screen.getByRole("button", { name: "Status Copy" }));
    await waitFor(() => expect(server.executeCommandLine).toHaveBeenCalledWith(
		"COPY STATUS SET 1 CUE 2 AT SET 2 CUE 2",
		{ target: "FIXTURE", pristine: false },
	));
  });

	it("ignores a legacy response choice before scoped authority requires it", async () => {
		const store = new ProgrammingInteractionStore();
		render(
			<ProgrammingInteractionViewProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				store={store}
				transport={null}
				loadSnapshot={async () =>
					programmingSnapshot({ command: commandLine(1, CHOICE.command) })
				}
			>
				<CommandChoiceModal />
			</ProgrammingInteractionViewProvider>,
		);
		await waitFor(() => expect(store.getSnapshot().status).toBe("ready"));
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	});

	it("optimistically closes on Cancel and performs one authoritative reset", async () => {
		const replaceCommandLine = vi.fn().mockResolvedValue(commandLine(2));
		renderScopedChoice({ replaceCommandLine });
		await waitFor(() =>
			expect(
				screen.getByRole("dialog", { name: "Cue Copy choice" }),
			).toBeInTheDocument(),
		);

		fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

		await waitFor(() =>
			expect(
				screen.queryByRole("dialog", { name: "Cue Copy choice" }),
			).not.toBeInTheDocument(),
		);
		expect(server.dismissCommandChoice).not.toHaveBeenCalled();
		expect(server.cancelCommandChoice).not.toHaveBeenCalled();
		expect(server.executeCommandLine).not.toHaveBeenCalled();
		await waitFor(() =>
			expect(replaceCommandLine).toHaveBeenCalledWith(DESK_ID, "", 1),
		);
	});

	it("restores the authoritative choice when Cancel fails", async () => {
		const replaceCommandLine = vi
			.fn()
			.mockRejectedValue(new Error("reset failed"));
		renderScopedChoice({ replaceCommandLine });
		await screen.findByRole("dialog", { name: "Cue Copy choice" });

		fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

		await waitFor(() =>
			expect(replaceCommandLine).toHaveBeenCalledWith(DESK_ID, "", 1),
		);
		await waitFor(() =>
			expect(
				screen.getByRole("dialog", { name: "Cue Copy choice" }),
			).toBeInTheDocument(),
		);
	});
});
