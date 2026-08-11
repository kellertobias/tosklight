import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ServerErrorToast } from "./ServerErrorToast";

const status = vi.hoisted(() => ({
	connection: "connected" as "connected" | "offline",
	error: "Server authority stopped. Reconnect the desk.",
	dismiss: vi.fn(),
}));

vi.mock("../../features/shellStatus/ShellStatusState", () => ({
	useConnectionStatus: () => status.connection,
	useServerError: () => status.error,
}));
vi.mock("../../features/shellStatus/ShellStatusActionsProvider", () => ({
	useShellStatusActions: () => ({ dismissError: status.dismiss }),
}));

afterEach(() => {
	cleanup();
	status.connection = "connected";
	status.error = "Server authority stopped. Reconnect the desk.";
	vi.clearAllMocks();
});

describe("ServerErrorToast", () => {
	it("owns connected critical failures in one actionable top-level surface", () => {
		render(<ServerErrorToast />);
		const alert = screen.getByRole("alert", { name: "Desk failure" });
		expect(alert).toHaveTextContent("Server authority stopped. Reconnect the desk.");
		expect(alert).toHaveTextContent(
			"Correct the named condition, then retry the action.",
		);
		fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
		expect(status.dismiss).toHaveBeenCalledOnce();
	});

	it("leaves connection failures to the existing connection surface", () => {
		status.connection = "offline";
		render(<ServerErrorToast />);
		expect(screen.queryByRole("alert", { name: "Desk failure" })).toBeNull();
	});
});
