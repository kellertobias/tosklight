import { describe, expect, it, vi } from "vitest";
import { createShowLifecycleActions } from "./showLifecycle";
import { ApiRequestError } from "../../api/ApiRequestError";
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
            shows: vi.fn().mockResolvedValue([current]), createShow: vi.fn(), downloadShow: vi.fn(), renameShow: vi.fn(),
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

describe("empty-show recovery with an incomplete library snapshot", () => {
    function recovery() {
        const saved = [{id:"damaged",name:"New Empty Show"},{id:"second",name:"new empty show 2"}];
        const api = { shows: { shows:vi.fn().mockResolvedValue(saved),
            createShow:vi.fn().mockImplementation(async (name: string) => {
                if (saved.some(show => show.name.toLowerCase()===name.toLowerCase()))
                    throw new ApiRequestError("invalid data: a show with that name already exists",400);
                return {id:"empty",name};
            }), openShow:vi.fn(), createFromBase:vi.fn() } };
        return {api, shows:[], bootstrap:{active_show:saved[0]},setShows:vi.fn(),setError:vi.fn(),refresh:vi.fn(),
            beginDeskLoading:vi.fn().mockReturnValue(1),finishDeskLoading:vi.fn()};
    }
    it("chooses a separate unique empty name from the authoritative library", async () => {
        const desk = recovery();
        expect(await createShowLifecycleActions(desk as unknown as ServerController).initializeEmptyShow()).toBe(true);
        expect(desk.api.shows.createShow).toHaveBeenCalledWith("New Empty Show 3");
        expect(desk.api.shows.openShow).toHaveBeenCalledExactlyOnceWith("empty","hold_current");
        expect(desk.bootstrap.active_show.id).toBe("damaged");
    });
    it("reloads names after a concurrent creation collision, never requesting overwrite", async () => {
        const desk = recovery();
        desk.api.shows.shows.mockResolvedValueOnce([]).mockResolvedValueOnce([{id:"other",name:"New Empty Show"}]);
        desk.api.shows.createShow.mockRejectedValueOnce(new ApiRequestError("a show with that name already exists",409)).mockResolvedValueOnce({id:"empty",name:"New Empty Show 2"});
        expect(await createShowLifecycleActions(desk as unknown as ServerController).initializeEmptyShow()).toBe(true);
        expect(desk.api.shows.createShow.mock.calls).toEqual([["New Empty Show"],["New Empty Show 2"]]);
        expect(desk.api.shows.openShow).toHaveBeenCalledExactlyOnceWith("empty","hold_current");
    });
    it("does not retry an unknown creation outcome or open the damaged show", async () => {
        const desk = recovery();
        desk.api.shows.createShow.mockRejectedValueOnce(new Error("connection lost while creating"));
        expect(await createShowLifecycleActions(desk as unknown as ServerController).initializeEmptyShow()).toBe(false);
        expect(desk.api.shows.createShow).toHaveBeenCalledTimes(1);
        expect(desk.api.shows.openShow).not.toHaveBeenCalled();
        expect(desk.refresh).not.toHaveBeenCalled();
    });
    it("reports an actionable library failure without creating or opening anything, then permits retry", async () => {
        const desk = recovery();
        desk.api.shows.shows.mockRejectedValueOnce(new Error("connection interrupted"));
        const actions = createShowLifecycleActions(desk as unknown as ServerController);
        expect(await actions.initializeEmptyShow()).toBe(false);
        expect(desk.api.shows.createShow).not.toHaveBeenCalled();
        expect(desk.api.shows.openShow).not.toHaveBeenCalled();
        expect(desk.setError.mock.calls.at(-1)?.[0]).toMatch(/retry/i);
        expect(await actions.initializeEmptyShow()).toBe(true);
    });
});
