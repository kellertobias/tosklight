import { act, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../../app/ToastContext";
import { useVisualizerEditing } from "../../features/visualizers/editing";
import { aVisualizer } from "../../testing/server";
import { ApiFailure, api } from "./client";
import { useEditing } from "./editing";

afterEach(() => {
	vi.useRealTimers();
	vi.restoreAllMocks();
});

const refusal = new ApiFailure("configuration-not-written", "The change could not be saved", 500);

describe.each(["text/audio", "visualizer"] as const)("%s background save failures", (kind) => {
	function editor(save: () => Promise<never>) {
		let schedule!: () => void;
		vi.spyOn(api, "updateVisualizer").mockImplementation(save);
		function Probe() {
			const generic = useEditing(vi.fn());
			const visualizer = useVisualizerEditing(vi.fn());
			const editing = kind === "visualizer" ? visualizer : generic;
			schedule = () => {
				if (kind === "visualizer")
					visualizer.saveLive(aVisualizer(), { requestId: "edit", name: "Updated" });
				else generic.saveLive(save);
			};
			return editing.failure ? <p role="alert">{editing.failure.message}</p> : null;
		}
		const rendered = render(<ToastProvider><Probe /></ToastProvider>);
		return {
			schedule: () => schedule(),
			navigate: () => rendered.rerender(<ToastProvider><p>Next page</p></ToastProvider>),
		};
	}

	it("reports a failure from the pending edit flushed by navigation", async () => {
		vi.useFakeTimers();
		const page = editor(async () => { throw refusal; });
		act(() => page.schedule());
		await act(async () => page.navigate());
		expect(screen.getAllByRole("alert")).toHaveLength(1);
		expect(screen.getByRole("alert")).toHaveTextContent(refusal.message);
	});

	it("reports a save already in flight when the operator navigates", async () => {
		vi.useFakeTimers();
		let reject!: (error: unknown) => void;
		const page = editor(() => new Promise<never>((_, fail) => { reject = fail; }));
		act(() => page.schedule());
		await act(async () => vi.advanceTimersByTimeAsync(180));
		act(() => page.navigate());
		await act(async () => reject(refusal));
		expect(screen.getAllByRole("alert")).toHaveLength(1);
		expect(screen.getByRole("alert")).toHaveTextContent(refusal.message);
	});

	it("keeps a failure on the mounted page without adding a duplicate shell toast", async () => {
		vi.useFakeTimers();
		const page = editor(async () => { throw refusal; });
		act(() => page.schedule());
		await act(async () => vi.advanceTimersByTimeAsync(180));
		expect(screen.getAllByRole("alert")).toHaveLength(1);
		expect(screen.getByRole("alert").tagName).toBe("P");
	});
});
