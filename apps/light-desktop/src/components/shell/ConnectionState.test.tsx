import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ConnectionState } from "./ConnectionState";

const state = vi.hoisted(() => ({
	bootstrapReady: true,
	connectionStatus: "connecting" as "connecting" | "connected" | "offline",
	serverError: null as string | null,
	retry: null as null | { role: "primary" | "secondary"; retry: () => void },
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
vi.mock("../../features/deskConnection/ConnectionRetryContext", () => ({
	useConnectionRetry: () => state.retry,
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
	state.retry = null;
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

	it("tells a screen window it joins the desk and never offers its own server", () => {
		const retry = vi.fn();
		state.retry = { role: "secondary", retry };
		state.bootstrapReady = false;
		state.serverError =
			"The ToskLight server at http://127.0.0.1:5000 is not reachable.";
		render(<ConnectionState />);

		expect(screen.getByRole("heading")).toHaveTextContent(
			"Screen cannot join the desk",
		);
		expect(screen.getByRole("alert")).toHaveTextContent(
			"http://127.0.0.1:5000 is not reachable",
		);
		expect(screen.queryByLabelText("Light server URL")).not.toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Retry now" }));
		fireEvent.click(screen.getByRole("button", { name: "Retry now" }));
		expect(retry).toHaveBeenCalledTimes(2);
	});

	it("shows a screen window joining the desk session while it connects", () => {
		state.retry = { role: "secondary", retry: vi.fn() };
		state.bootstrapReady = false;
		render(<ConnectionState />);
		expect(screen.getByText("Joining the desk")).toBeInTheDocument();
		expect(
			screen.getByText(/same server and session as the main ToskLight window/),
		).toBeInTheDocument();
	});
});
