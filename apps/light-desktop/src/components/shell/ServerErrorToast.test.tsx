import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ServerErrorToast } from "./ServerErrorToast";

const status = vi.hoisted(() => ({
	connection: "connected" as "connected" | "offline",
	error: "Server authority stopped. Reconnect the desk." as string | null,
	showError: null as string | null,
	diagnostics:
		[] as import("../../features/deskState/deskStateDiagnostics").DeskStateDiagnostic[],
	dismiss: vi.fn(),
}));

vi.mock("../../features/shellStatus/ShellStatusState", () => ({
	useConnectionStatus: () => status.connection,
	useServerError: () => status.error,
}));
vi.mock("../../features/shellStatus/ShellStatusActionsProvider", () => ({
	useShellStatusActions: () => ({ dismissError: status.dismiss }),
}));
vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({
	useActiveShowError: () => status.showError,
}));
vi.mock("../../features/deskState/DeskStateDiagnosticsState", () => ({
	useDeskStateDiagnostics: () => status.diagnostics,
}));

afterEach(() => {
	cleanup();
	status.connection = "connected";
	status.error = "Server authority stopped. Reconnect the desk.";
	status.showError = null;
	status.diagnostics = [];
	vi.clearAllMocks();
});

describe("ServerErrorToast", () => {
	it("keeps request failures noncritical without removing operator controls", () => {
		render(<ServerErrorToast />);
		const alert = screen.getByRole("status", { name: "Action feedback" });
		expect(alert).toHaveTextContent(
			"Server authority stopped. Reconnect the desk.",
		);
		expect(screen.queryByText("Desk needs attention")).toBeNull();
		expect(screen.queryByRole("alert")).toBeNull();
		fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
		expect(status.dismiss).toHaveBeenCalledOnce();
	});

	it("leaves connection failures to the existing connection surface", () => {
		status.connection = "offline";
		render(<ServerErrorToast />);
		expect(screen.queryByRole("alert", { name: "Desk failure" })).toBeNull();
	});

	it("retires recovered request failures automatically", () => {
		const view = render(<ServerErrorToast />);
		expect(
			screen.getByRole("status", { name: "Action feedback" }),
		).toHaveTextContent("Server authority stopped. Reconnect the desk.");

		status.error = null;
		view.rerender(<ServerErrorToast />);
		expect(
			screen.queryByRole("status", { name: "Action feedback" }),
		).toBeNull();
	});

	it("retains authoritative show loss even when an unrelated request succeeds", () => {
		status.showError = "Active show failed integrity validation";
		const view = render(<ServerErrorToast />);
		expect(
			screen.getByRole("alert", { name: "Desk failure" }),
		).toHaveTextContent(status.showError);
		status.error = null;
		view.rerender(<ServerErrorToast />);
		expect(
			screen.getByRole("alert", { name: "Desk failure" }),
		).toHaveTextContent("Show recovery");
		status.showError = null;
		view.rerender(<ServerErrorToast />);
		expect(screen.queryByRole("alert")).toBeNull();
	});

	it("keeps confirmed DMX suppression critical until the capability recovers", () => {
		status.diagnostics = [
			{
				id: "usb",
				title: "Output stopped",
				summary: "USB DMX output is suppressed",
				action: "Disable the duplicate route",
				capabilityLoss: "dmx_output",
			},
		];
		const view = render(<ServerErrorToast />);
		expect(
			screen.getByRole("alert", { name: "Desk failure" }),
		).toHaveTextContent("USB DMX output is suppressed");
		status.diagnostics = [];
		view.rerender(<ServerErrorToast />);
		expect(screen.queryByRole("alert")).toBeNull();
	});

	it("does not escalate a pane diagnostic without capability loss", () => {
		status.diagnostics = [
			{
				id: "pane",
				title: "Disconnected pane",
				summary: "An event feed closed",
				action: "Reload pane",
			},
		];
		render(<ServerErrorToast />);
		expect(screen.queryByRole("alert")).toBeNull();
	});
});
