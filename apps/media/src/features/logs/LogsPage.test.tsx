import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { aLog, stubServer } from "../../testing/server";
import { LogsPage, elapsed } from "./LogsPage";

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe("the log viewer", () => {
	it("shows the records the server is holding", async () => {
		stubServer();
		render(<LogsPage />);

		const records = await screen.findByRole("list", { name: "Log records" });
		expect(records).toHaveTextContent("media server starting");
		expect(records).toHaveTextContent("no audio input");
	});

	it("asks only for what arrived after the record it already holds", async () => {
		stubServer();
		render(<LogsPage />);
		await screen.findByText(/media server starting/u);

		await userEvent.click(screen.getByRole("button", { name: "Read now" }));

		const asked = () => vi.mocked(globalThis.fetch).mock.calls.map((call) => String(call[0]));
		await waitFor(() =>
			expect(asked().some((url) => url.includes("after=2"))).toBe(true),
		);
	});

	it("says out loud when the server has had to discard records", async () => {
		stubServer({ logs: aLog({ dropped: 12 }) });
		render(<LogsPage />);

		expect(await screen.findByText(/discarded 12 older records/u)).toBeInTheDocument();
	});

	it("starts again when the level changes, so two filters are not mixed", async () => {
		stubServer();
		render(<LogsPage />);
		await screen.findByText(/media server starting/u);

		await userEvent.click(screen.getByRole("button", { name: "Information and above" }));
		await userEvent.click(screen.getByRole("option", { name: "Errors only" }));

		const asked = () => vi.mocked(globalThis.fetch).mock.calls.map((call) => String(call[0]));
		await waitFor(() =>
			expect(
				asked().some((url) => url.includes("level=error") && !url.includes("after")),
			).toBe(true),
		);
	});

	it("says the server is not answering rather than showing an empty log", async () => {
		stubServer();
		vi.mocked(globalThis.fetch).mockRejectedValue(new Error("no route to host"));
		render(<LogsPage />);

		expect(await screen.findByRole("alert")).toHaveTextContent("not answering");
	});
});

describe("when a record was emitted", () => {
	it("is reported as time since this server started", () => {
		expect(elapsed(340)).toBe("00:00");
		expect(elapsed(65_000)).toBe("01:05");
		expect(elapsed(3_725_000)).toBe("1:02:05");
	});
});
