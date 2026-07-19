import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProgrammingInteractionViewProvider } from "../../features/programmingInteraction/ProgrammingInteractionView";
import { ProgrammingInteractionStore } from "../../features/programmingInteraction/store";
import {
	commandLine,
	DESK_ID,
	programmingSnapshot,
	SHOW_ID,
} from "../../features/programmingInteraction/testFixtures";
import { CommandChoiceModal } from "./CommandChoiceModal";

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
  it("renders only the applicable Plain, Status, and Cancel choices", () => {
    render(<CommandChoiceModal />);

    expect(screen.getByRole("dialog", { name: "Cue Copy choice" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Plain Copy" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Status Copy" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Move/ })).not.toBeInTheDocument();
  });

  it("executes the selected explicit command and leaves Cancel mutation-free", async () => {
    const { rerender } = render(<CommandChoiceModal />);
    fireEvent.click(screen.getByRole("button", { name: "Status Copy" }));
    await waitFor(() => expect(server.executeCommandLine).toHaveBeenCalledWith("COPY STATUS SET 1 CUE 2 AT SET 2 CUE 2"));

    server.pendingCommandChoice = { ...server.pendingCommandChoice!, operation: "move", command: "MOVE SET 1 CUE 2 AT SET 2 CUE 2", options: [
      { id: "plain", label: "Plain Move", command: "MOVE PLAIN SET 1 CUE 2 AT SET 2 CUE 2" },
      { id: "status", label: "Status Move", command: "MOVE STATUS SET 1 CUE 2 AT SET 2 CUE 2" },
    ] };
    rerender(<CommandChoiceModal />);
    await waitFor(() => expect(screen.getByRole("button", { name: "Cancel" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(server.cancelCommandChoice).toHaveBeenCalledTimes(1);
    expect(server.executeCommandLine).toHaveBeenCalledTimes(1);
  });

	it("dismisses a compatibility choice while resetting scoped command state", async () => {
		const store = new ProgrammingInteractionStore();
		const choiceCommand = server.pendingCommandChoice?.command ?? "";
		const replaceCommandLine = vi.fn().mockResolvedValue(commandLine(2));
		render(
			<ProgrammingInteractionViewProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				store={store}
				transport={null}
				loadSnapshot={async () =>
					programmingSnapshot({ command: commandLine(1, choiceCommand) })
				}
				replaceCommandLine={replaceCommandLine}
			>
				<CommandChoiceModal />
			</ProgrammingInteractionViewProvider>,
		);
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
		expect(server.dismissCommandChoice).toHaveBeenCalledOnce();
		expect(server.cancelCommandChoice).not.toHaveBeenCalled();
		await waitFor(() =>
			expect(replaceCommandLine).toHaveBeenCalledWith(DESK_ID, "", 1),
		);
	});
});
