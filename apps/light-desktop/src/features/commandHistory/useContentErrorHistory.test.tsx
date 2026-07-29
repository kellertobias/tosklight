import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import {
	mergeCommandHistory,
	useContentErrorHistory,
} from "./useContentErrorHistory";

afterEach(cleanup);

describe("content-window command history", () => {
	it("records a pane alert once with its pane name and latest message", async () => {
		const { result } = renderHook(() => useContentErrorHistory());
		const workspace = document.createElement("main");
		workspace.className = "workspace-view";
		workspace.innerHTML = `
			<article class="desk-pane" aria-label="Dynamics pane">
				<p role="alert">Dynamic 4 could not be loaded.</p>
			</article>
		`;

		await act(async () => {
			document.body.append(workspace);
			await Promise.resolve();
		});

		expect(result.current).toHaveLength(1);
		expect(result.current[0]).toMatchObject({
			command: "DYNAMICS ERROR",
			status: "rejected",
			feedback: "Dynamic 4 could not be loaded.",
			source: "window",
		});

		await act(async () => {
			workspace.querySelector('[role="alert"]')?.append(" Please retry.");
			await Promise.resolve();
		});
		expect(result.current).toHaveLength(2);
		expect(result.current[0]?.feedback).toBe(
			"Dynamic 4 could not be loaded. Please retry.",
		);
	});

	it("merges pane errors with authoritative history newest first", () => {
		const history = mergeCommandHistory(
			[
				{
					id: "command",
					command: "FIXTURE 1 AT FULL",
					status: "accepted",
					feedback: "Applied.",
					source: "software",
					at: "2026-07-29T12:00:00.000Z",
				},
			],
			[
				{
					id: "error",
					command: "STAGE ERROR",
					status: "rejected",
					feedback: "Renderer unavailable.",
					source: "window",
					at: "2026-07-29T12:01:00.000Z",
				},
			],
		);

		expect(history.map((entry) => entry.id)).toEqual(["error", "command"]);
	});
});
