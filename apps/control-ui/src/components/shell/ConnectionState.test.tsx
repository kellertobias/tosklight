import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ConnectionState } from "./ConnectionState";

const state = vi.hoisted(() => ({
	bootstrapReady: true,
	connectionStatus: "connecting" as "connecting" | "connected" | "offline",
	serverError: null as string | null,
}));

vi.mock("../../features/shellStatus/ShellStatusState", () => ({
	useConnectionStatus: () => state.connectionStatus,
	useServerError: () => state.serverError,
}));
vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({
	useBootstrapReady: () => state.bootstrapReady,
}));
vi.mock("../../features/deskConnection/DeskConnectionContext", () => ({
	useDeskConnection: () => null,
}));
vi.mock("../../api/client/serverLocation", () => ({
	configuredServerUrl: () => "http://127.0.0.1:5000",
}));
vi.mock("../../platform/desktop", () => ({
	useDesktopBridge: () => ({ available: false }),
}));

beforeEach(() => {
	state.bootstrapReady = true;
	state.connectionStatus = "connecting";
	state.serverError = null;
});
afterEach(cleanup);

describe("ConnectionState", () => {
	it("keeps the full boot cover after bootstrap until the desk is connected", () => {
		render(<ConnectionState />);

		expect(screen.getByRole("status")).toHaveClass("connection-cover");
		expect(screen.getByRole("heading")).toHaveTextContent(
			"Connecting to ToskLight",
		);
		expect(screen.getByText(/bootstrap, operator session, and desk stores/i))
			.toBeInTheDocument();
	});

	it("retains the interactive desk and uses a compact banner during reconnect", () => {
		state.connectionStatus = "connected";
		const rendered = render(<ConnectionState />);
		expect(screen.queryByRole("status")).not.toBeInTheDocument();

		act(() => {
			state.connectionStatus = "connecting";
			rendered.rerender(<ConnectionState />);
		});

		expect(screen.getByRole("status")).toHaveClass("connection-banner");
		expect(screen.getByText("Reconnecting to server…")).toBeInTheDocument();
	});
});
