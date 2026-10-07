import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ErrorAlert, formatErrorDetails } from "./ErrorAlert";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

describe("copyable errors", () => {
	it("copies the full trace even when the visible text is a summary", async () => {
		const writeText = vi.fn().mockResolvedValue(undefined);
		Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
		const diagnostic = "Output failed\nrequest_id: desk-12\ntrace:\n  engine::render\n  output::send\n";
		render(<ErrorAlert as="p" copyText={diagnostic}>Output failed</ErrorAlert>);
		const button = screen.getByRole("button", { name: "Copy error" });
		expect(button.textContent).toBe("");
		fireEvent.click(button);
		await waitFor(() => expect(writeText).toHaveBeenCalledWith(diagnostic));
		expect(screen.getByRole("alert")).toBeVisible();
	});

	it("copies collapsed diagnostic details and excludes actions", async () => {
		const writeText = vi.fn().mockResolvedValue(undefined);
		Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
		render(<ErrorAlert><p>Output failed</p><details><summary>Trace</summary><pre>{"frame 1\nframe 2"}</pre></details><button>Dismiss</button></ErrorAlert>);
		fireEvent.click(screen.getByRole("button", { name: "Copy error" }));
		await waitFor(() => expect(writeText).toHaveBeenCalledWith("Output failed\nTrace\nframe 1\nframe 2"));
	});

	it("falls back to gesture-based copying on an HTTP desk screen", async () => {
		Object.defineProperty(navigator, "clipboard", { configurable: true, value: undefined });
		const execCommand = vi.fn(() => true);
		Object.defineProperty(document, "execCommand", { configurable: true, value: execCommand });
		render(<ErrorAlert copyText="Full error\ntrace">Failure</ErrorAlert>);
		fireEvent.click(screen.getByRole("button", { name: "Copy error" }));
		await waitFor(() => expect(execCommand).toHaveBeenCalledWith("copy"));
		expect(document.querySelector("textarea")).toBeNull();
	});

	it("reports clipboard failure without dismissing the error", async () => {
		Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: vi.fn().mockRejectedValue(new Error("denied")) } });
		Object.defineProperty(document, "execCommand", { configurable: true, value: vi.fn(() => false) });
		render(<ErrorAlert>Failure remains</ErrorAlert>);
		fireEvent.click(screen.getByRole("button", { name: "Copy error" }));
		await screen.findByRole("button", { name: "Copy failed; retry copying error" });
		expect(screen.getByRole("alert")).toHaveTextContent("Failure remains");
	});

	it("keeps causes, stacks, and structured diagnostics", () => {
		const cause = new Error("Socket closed");
		cause.stack = "Error: Socket closed\n  at output.send";
		const error = Object.assign(new Error("Render failed", { cause }), { trace: "engine::frame", request_id: "desk-12" });
		error.stack = "Error: Render failed\n  at renderer.tick";
		expect(formatErrorDetails(error)).toBe("Render failed\nError: Render failed\n  at renderer.tick\ntrace: engine::frame\nrequest_id: desk-12\nCaused by: Socket closed\nError: Socket closed\n  at output.send");
	});

	it("does not offer error copying for successful status messages", () => {
		render(<ErrorAlert role="status">Saved</ErrorAlert>);
		expect(screen.queryByRole("button")).toBeNull();
	});
});
