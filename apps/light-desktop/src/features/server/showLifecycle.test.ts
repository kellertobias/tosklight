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

describe("base shows and Latest Autosave", () => {
    function model() {
        const current = { id: "current", name: "Tour", is_base_show: false };
        const shows = {
            createShow: vi.fn(), downloadShow: vi.fn(), renameShow: vi.fn(),
            createFromBase: vi.fn().mockResolvedValue({ id: "copy", name: "New Show from Base" }),
            setBaseShow: vi.fn().mockResolvedValue(current), openShow: vi.fn(),
        };
        return { api: { shows }, bootstrap: {active_show: current}, shows: [current],
            refresh: vi.fn(), setError: vi.fn(), setShows: vi.fn(),
            beginDeskLoading: vi.fn().mockReturnValue(1), finishDeskLoading: vi.fn() };
    }
    it("keeps the same current autosave identity without copying unrelated data", async () => {
        const desk = model();
        expect(await createShowLifecycleActions(desk as unknown as ServerController).saveShowAs("Tour", {latest: true, baseShow: true})).toBe(true);
        expect(desk.api.shows.setBaseShow).toHaveBeenCalledWith("current", true);
        expect(desk.api.shows.createShow).not.toHaveBeenCalled();
        expect(desk.api.shows.downloadShow).not.toHaveBeenCalled();
        expect(desk.api.shows.openShow).not.toHaveBeenCalled();
    });
    it("opens the new copy returned by the base intent rather than its source", async () => {
        const desk = model();
        expect(await createShowLifecycleActions(desk as unknown as ServerController).initializeEmptyShow("base")).toBe(true);
        expect(desk.api.shows.createFromBase).toHaveBeenCalledWith("base", "New Show from Base");
        expect(desk.api.shows.openShow).toHaveBeenCalledWith("copy", "hold_current");
    });
});
