import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ScreenConfiguration, ScreenSnapshot } from "../../api/types";
import type { ShowObject } from "../showObjects/contracts";
import { ScreensProvider } from "./ScreensContext";
import type { ScreensContextValue } from "./types";

const mocks = vi.hoisted(() => ({
	desk: null as { active_page: number } | null,
	deskEnabled: [] as boolean[],
	pagesView: {
		ready: true,
		error: null as Error | null,
		pages: [] as ShowObject<"playback_page">[],
	},
	pagesViewMounted: 0,
	actions: null as { createPage: ReturnType<typeof vi.fn> } | null,
	bankPages: [] as (number | null | undefined)[],
}));

vi.mock("../playbackRuntime/PlaybackRuntimeView", () => ({
	usePlaybackDeskView: (enabled = true) => {
		mocks.deskEnabled.push(enabled);
		return enabled ? mocks.desk : null;
	},
}));
vi.mock("../playbackTopology/PlaybackTopologyView", () => ({
	usePlaybackPagesView: () => {
		mocks.pagesViewMounted += 1;
		return mocks.pagesView;
	},
}));
vi.mock("../playbackTopology/PlaybackTopologyProvider", () => ({
	usePlaybackTopologyActions: () => mocks.actions,
}));
vi.mock("../../components/control/PlaybackFaderBank", () => ({
	PlaybackFaderBank: ({ pageNumber }: { pageNumber?: number | null }) => {
		mocks.bankPages.push(pageNumber);
		return <div data-testid="fader-bank" data-page={pageNumber} />;
	},
}));

const { ScreenPlaybackSection } = await import("./ScreenPlaybackSection");

function pageObject(number: number, slots: Record<string, number> = {}) {
	return {
		kind: "playback_page",
		id: `page-${number}`,
		revision: 1,
		updated_at: "2026-07-20T10:00:00Z",
		body: { number, name: `Page ${number}`, slots },
	} as ShowObject<"playback_page">;
}

function screenConfiguration(
	overrides: Partial<ScreenConfiguration> = {},
): ScreenConfiguration {
	return {
		id: "screen-1",
		name: "Screen 1",
		layout: { desks: [], activeDeskId: "desk-1" },
		show_dock: false,
		show_playbacks: true,
		playback_count: 4,
		playback_rows: 1,
		first_playback_slot: 1,
		page_mode: "independent",
		show_page_controls: true,
		desired_open: true,
		display_id: null,
		bounds: null,
		fullscreen: false,
		...overrides,
	};
}

function source(
	partial: Partial<ScreensContextValue> = {},
): ScreensContextValue {
	return {
		screens: null,
		bootstrap: null,
		session: null,
		saveScreen: vi.fn(),
		deleteScreen: vi.fn(),
		setScreenPage: vi.fn(async () => undefined),
		updateControlDesk: vi.fn(),
		selectControlDesk: vi.fn(),
		removeClient: vi.fn(async () => true),
		...partial,
	};
}

function snapshot(activePages: Record<string, number>): ScreenSnapshot {
	return { screens: [], active_pages: activePages };
}

function mount(value: ScreensContextValue, screenConfig: ScreenConfiguration) {
	return render(
		<ScreensProvider source={value}>
			<ScreenPlaybackSection screen={screenConfig} />
		</ScreensProvider>,
	);
}

async function settled() {
	await act(async () => {
		await Promise.resolve();
	});
}

function bankPage() {
	return screen.getByTestId("fader-bank").getAttribute("data-page");
}

beforeEach(() => {
	mocks.desk = null;
	mocks.deskEnabled = [];
	mocks.pagesView = { ready: true, error: null, pages: [pageObject(1)] };
	mocks.pagesViewMounted = 0;
	mocks.actions = { createPage: vi.fn(async () => outcome()) };
	mocks.bankPages = [];
});

afterEach(cleanup);

function outcome(status: "changed" | "no_change" = "changed") {
	return {
		requestId: "request-1",
		correlationId: "correlation-1",
		showRevision: 4,
		resolution: { kind: "page" as const, page: 2 },
		objects: [],
		replayed: false,
		status,
		...(status === "changed" ? { eventSequence: 9 } : {}),
	};
}

describe("Follow Main secondary screens", () => {
	it("tracks the desk page and never writes a screen page", async () => {
		mocks.desk = { active_page: 3 };
		const setScreenPage = vi.fn(async () => undefined);
		mount(
			source({ setScreenPage, screens: snapshot({ "screen-1": 7 }) }),
			screenConfiguration({ page_mode: "follow_main" }),
		);
		expect(bankPage()).toBe("3");
		fireEvent.click(screen.getByRole("button", { name: "▲ PAGE UP" }));
		fireEvent.click(screen.getByRole("button", { name: "PAGE DOWN ▼" }));
		await settled();
		expect(setScreenPage).not.toHaveBeenCalled();
		expect(mocks.actions?.createPage).not.toHaveBeenCalled();
	});

	it("gates on the desk projection instead of falling back to Page 1", () => {
		mocks.desk = null;
		mount(
			source({ screens: snapshot({ "screen-1": 5 }) }),
			screenConfiguration({ page_mode: "follow_main" }),
		);
		expect(screen.queryByTestId("fader-bank")).toBeNull();
		expect(screen.getByRole("status")).toHaveTextContent("Loading Playbacks…");
	});
});

describe("independent secondary screens", () => {
	it("tracks only the screen's own page", () => {
		mocks.desk = { active_page: 9 };
		mount(source({ screens: snapshot({ "screen-1": 2 }) }), screenConfiguration());
		expect(bankPage()).toBe("2");
		expect(mocks.deskEnabled.every((enabled) => enabled === false)).toBe(true);
	});

	it("does not fall back to the desk page or Page 1 without its authority", () => {
		mocks.desk = { active_page: 9 };
		mount(source({ screens: snapshot({ "other": 4 }) }), screenConfiguration());
		expect(screen.queryByTestId("fader-bank")).toBeNull();
		expect(screen.getByRole("status")).toHaveTextContent("Loading Playbacks…");
	});

	it("switches to an existing Page with only setScreenPage", async () => {
		mocks.pagesView.pages = [pageObject(1), pageObject(2)];
		const setScreenPage = vi.fn(async () => undefined);
		mount(
			source({ setScreenPage, screens: snapshot({ "screen-1": 1 }) }),
			screenConfiguration(),
		);
		fireEvent.click(screen.getByRole("button", { name: "PAGE DOWN ▼" }));
		await settled();
		expect(mocks.actions?.createPage).not.toHaveBeenCalled();
		expect(setScreenPage).toHaveBeenCalledWith("screen-1", 2);
	});

	it("creates a missing Page with one typed action before one setScreenPage", async () => {
		mocks.pagesView.pages = [pageObject(1, { "1": 11 })];
		const setScreenPage = vi.fn(async () => undefined);
		mount(
			source({ setScreenPage, screens: snapshot({ "screen-1": 1 }) }),
			screenConfiguration(),
		);
		fireEvent.click(screen.getByRole("button", { name: "PAGE DOWN ▼" }));
		await settled();
		expect(mocks.actions?.createPage).toHaveBeenCalledTimes(1);
		expect(mocks.actions?.createPage).toHaveBeenCalledWith(2);
		expect(setScreenPage).toHaveBeenCalledTimes(1);
		expect(setScreenPage).toHaveBeenCalledWith("screen-1", 2);
	});

	it("adopts a no-change creation outcome as existing authority", async () => {
		mocks.pagesView.pages = [pageObject(1, { "1": 11 })];
		mocks.actions = { createPage: vi.fn(async () => outcome("no_change")) };
		const setScreenPage = vi.fn(async () => undefined);
		mount(
			source({ setScreenPage, screens: snapshot({ "screen-1": 1 }) }),
			screenConfiguration(),
		);
		fireEvent.click(screen.getByRole("button", { name: "PAGE DOWN ▼" }));
		await settled();
		expect(setScreenPage).toHaveBeenCalledWith("screen-1", 2);
	});

	it("keeps the screen page after a failed or conflicted creation", async () => {
		mocks.pagesView.pages = [pageObject(1, { "1": 11 })];
		mocks.actions = { createPage: vi.fn(async () => null) };
		const setScreenPage = vi.fn(async () => undefined);
		mount(
			source({ setScreenPage, screens: snapshot({ "screen-1": 1 }) }),
			screenConfiguration(),
		);
		fireEvent.click(screen.getByRole("button", { name: "PAGE DOWN ▼" }));
		await settled();
		expect(mocks.actions.createPage).toHaveBeenCalledTimes(1);
		expect(setScreenPage).not.toHaveBeenCalled();
		expect(bankPage()).toBe("1");
	});

	it("selects and creates Pages from the picker", async () => {
		mocks.pagesView.pages = [pageObject(1), pageObject(2)];
		const setScreenPage = vi.fn(async () => undefined);
		const value = source({
			setScreenPage,
			screens: snapshot({ "screen-1": 1 }),
		});
		const view = mount(value, screenConfiguration());
		fireEvent.click(
			view.container.querySelector(
				".screen-page-controls button:nth-of-type(2)",
			) as HTMLElement,
		);
		expect(view.container.querySelector(".screen-page-picker")).not.toBeNull();
		fireEvent.click(screen.getByRole("button", { name: "2 · Page 2" }));
		await settled();
		expect(setScreenPage).toHaveBeenCalledWith("screen-1", 2);
		expect(view.container.querySelector(".screen-page-picker")).toBeNull();

		fireEvent.click(
			view.container.querySelector(
				".screen-page-controls button:nth-of-type(2)",
			) as HTMLElement,
		);
		fireEvent.click(screen.getByRole("button", { name: "Add new page" }));
		await settled();
		expect(mocks.actions?.createPage).toHaveBeenCalledWith(3);
		expect(setScreenPage).toHaveBeenLastCalledWith("screen-1", 3);
	});
});

describe("secondary screen page authority gating", () => {
	it("rejects page actions while topology is loading", async () => {
		mocks.pagesView = { ready: false, error: null, pages: [pageObject(1)] };
		const setScreenPage = vi.fn(async () => undefined);
		mount(
			source({ setScreenPage, screens: snapshot({ "screen-1": 2 }) }),
			screenConfiguration(),
		);
		fireEvent.click(screen.getByRole("button", { name: "▲ PAGE UP" }));
		fireEvent.click(screen.getByRole("button", { name: "PAGE DOWN ▼" }));
		await settled();
		expect(setScreenPage).not.toHaveBeenCalled();
		expect(mocks.actions?.createPage).not.toHaveBeenCalled();
	});

	it("rejects page actions while the topology writer is being replaced", async () => {
		mocks.actions = null;
		const setScreenPage = vi.fn(async () => undefined);
		mount(
			source({ setScreenPage, screens: snapshot({ "screen-1": 2 }) }),
			screenConfiguration(),
		);
		fireEvent.click(screen.getByRole("button", { name: "▲ PAGE UP" }));
		await settled();
		expect(setScreenPage).not.toHaveBeenCalled();
	});

	it("does not adopt a Page created under replaced authority", async () => {
		mocks.pagesView.pages = [pageObject(1, { "1": 11 })];
		let release: (() => void) | null = null;
		const createPage = vi.fn(
			() =>
				new Promise((resolve) => {
					release = () => resolve(outcome());
				}),
		);
		mocks.actions = { createPage };
		const setScreenPage = vi.fn(async () => undefined);
		const value = source({
			setScreenPage,
			screens: snapshot({ "screen-1": 1 }),
		});
		const view = mount(value, screenConfiguration());
		fireEvent.click(screen.getByRole("button", { name: "PAGE DOWN ▼" }));
		mocks.actions = { createPage: vi.fn() };
		view.rerender(
			<ScreensProvider source={{ ...value }}>
				<ScreenPlaybackSection screen={screenConfiguration()} />
			</ScreensProvider>,
		);
		await act(async () => {
			release?.();
			await Promise.resolve();
		});
		expect(setScreenPage).not.toHaveBeenCalled();
	});
});

describe("dormant secondary screen Playback authority", () => {
	it("mounts no Playback or Page authority when Playbacks are hidden", () => {
		mocks.desk = { active_page: 3 };
		render(
			<ScreensProvider source={source({ screens: snapshot({ "screen-1": 2 }) })}>
				{screenConfiguration({ show_playbacks: false }).show_playbacks ? (
					<ScreenPlaybackSection screen={screenConfiguration()} />
				) : null}
			</ScreensProvider>,
		);
		expect(mocks.deskEnabled).toEqual([]);
		expect(mocks.pagesViewMounted).toBe(0);
		expect(mocks.bankPages).toEqual([]);
	});

	it("mounts no page-control authority when page controls are hidden", () => {
		mount(
			source({ screens: snapshot({ "screen-1": 2 }) }),
			screenConfiguration({ show_page_controls: false }),
		);
		expect(bankPage()).toBe("2");
		expect(mocks.pagesViewMounted).toBe(0);
		expect(document.querySelector(".screen-page-controls")).toBeNull();
	});
});

describe("removed broad Playback facade", () => {
	it("exposes no bootstrap Playback snapshot or savePlaybackPage action", () => {
		const value = source();
		expect(Object.keys(value)).not.toContain("playbacks");
		expect(Object.keys(value)).not.toContain("savePlaybackPage");
	});

	it("performs no network request for the secondary screen surface", () => {
		const fetchSpy = vi.spyOn(globalThis, "fetch");
		mount(source({ screens: snapshot({ "screen-1": 2 }) }), screenConfiguration());
		expect(fetchSpy).not.toHaveBeenCalled();
		fetchSpy.mockRestore();
	});
});
