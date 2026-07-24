import { describe, expect, it, vi } from "vitest";
import { createShowLifecycleActions } from "./showLifecycle";
import type { ServerController } from "./model";

describe("show lifecycle loading state", () => {
	it("keeps the named loading operation active through refresh", async () => {
		const events: string[] = [];
		const model = {
			api: {
				shows: {
					openShow: vi.fn(async () => {
						events.push("open");
					}),
				},
			},
			bootstrap: null,
			shows: [{ id: "festival", name: "Festival" }],
			setShows: vi.fn(),
			setError: vi.fn(),
			refresh: vi.fn(async () => {
				events.push("refresh");
			}),
			beginDeskLoading: vi.fn((title: string) => {
				events.push(`begin:${title}`);
				return 8;
			}),
			finishDeskLoading: vi.fn((operationId: number) => {
				events.push(`finish:${operationId}`);
			}),
		} as unknown as ServerController;

		await createShowLifecycleActions(model).openShow("festival");

		expect(events).toEqual([
			"begin:Loading show Festival…",
			"open",
			"refresh",
			"finish:8",
		]);
	});
});
