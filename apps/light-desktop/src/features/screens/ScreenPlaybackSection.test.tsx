import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import type { ScreenConfiguration } from "../../api/types";
import { ScreensProvider } from "./ScreensContext";
import type { ScreensContextValue } from "./types";
const mocks = vi.hoisted(() => ({
	pages: { ready: true, error: null, pages: [] as unknown[] },
	pagesViewMounted: 0,
	actions: { createPage: vi.fn() },
}));
vi.mock("../playbackTopology/PlaybackTopologyView", () => ({
	usePlaybackPagesView: () => {
		mocks.pagesViewMounted += 1;
		return mocks.pages;
	},
}));
vi.mock("../playbackTopology/PlaybackTopologyProvider", () => ({
	usePlaybackTopologyActions: () => mocks.actions,
}));
vi.mock("../../components/control/PlaybackFaderBank", () => ({
	PlaybackFaderBank: ({
		pageNumber,
		surfaceId,
	}: {
		pageNumber: number;
		surfaceId: string;
	}) => (
		<div data-testid="bank" data-page={pageNumber} data-surface={surfaceId} />
	),
}));
vi.mock("../../components/control/HardwareControlSummary", () => ({
	HardwareControlSummary: ({ encoders, pageControl }: { encoders?: ReactNode; pageControl?: ReactNode }) => <div data-testid="compact-hardware-summary">{pageControl}{encoders}</div>,
}));

const { ScreenPlaybackSection } = await import("./ScreenPlaybackSection");
const configuration: ScreenConfiguration = {
	id: "screen-1",
	surface_id: "screen:screen-1",
	name: "Screen",
	layout: { desks: [], activeDeskId: "main" },
	content: { type: "desktop" },
	show_dock: false,
	show_playbacks: true,
	show_page_controls: true,
	show_programmer: false,
	not_editable: false,
	desired_open: true,
	display_id: null,
	bounds: null,
	fullscreen: false,
};
function source(
	type: "own" | "pinned" | "follow" = "own",
	page: number | null = 2,
): ScreensContextValue {
	const surfaces: import("../../api/playbackConfigurationModels").PlaybackSurfacesSnapshot =
		{
			selected_surface_id: "main",
			command_page: 1,
			resolved_pages: page === null ? {} : { "screen:screen-1": page },
			playback_surfaces: {
				"screen:screen-1": {
					name: "Screen",
					layout: {
						rows: [
							{
								first_position: 201,
								count: 50,
								presentation: "buttons" as const,
								button_count: 1,
							},
						],
					},
					page_source:
						type === "follow"
							? { type, surface_id: "main" }
							: { type, page: 2 },
				},
			},
		};
	return {
		surfaces,
		screens: {
			screens: [configuration],
			surfaces,
			programmer_control_surface: {
				owner_screen_id: null,
				visible_encoders: 6,
			},
		},
		bootstrap: null,
		session: null,
		saveScreen: vi.fn(),
		deleteScreen: vi.fn(),
		setSurfacePage: vi.fn(async () => {}),
		selectSurface: vi.fn(async () => {}),
		updatePlaybackSurface: vi.fn(async () => {}),
		updateProgrammerControlSurface: vi.fn(async () => {}),
		updateControlDesk: vi.fn(async () => {}),
		selectControlDesk: vi.fn(),
		removeClient: vi.fn(async () => true),
	};
}
function mount(value: ScreensContextValue) {
	return render(
		<ScreensProvider source={value}>
			<ScreenPlaybackSection screen={configuration} />
		</ScreensProvider>,
	);
}
beforeEach(() => {
	mocks.pagesViewMounted = 0;
	mocks.pages = {
		ready: true,
		error: null,
		pages: [
			{ body: { number: 1, name: "One", assignments: {} } },
			{
				body: {
					number: 2,
					name: "Two",
					assignments: { 1: { name: "Recorded" } },
				},
			},
		],
	};
	mocks.actions = { createPage: vi.fn(async () => ({ status: "changed" })) };
});
afterEach(cleanup);
describe("authoritative secondary Playback surface", () => {
	it.each([
		"own",
		"pinned",
		"follow",
	] as const)("renders the resolved %s surface page", (type) => {
		mount(source(type, 37));
		expect(screen.getByTestId("bank")).toHaveAttribute("data-page", "37");
		expect(screen.getByTestId("bank")).toHaveAttribute(
			"data-surface",
			"screen:screen-1",
		);
	});
	it("gates missing authority", () => {
		mount(source("own", null));
		expect(screen.queryByTestId("bank")).toBeNull();
		expect(screen.getByRole("status")).toHaveTextContent("Loading Playbacks");
	});
	it.each([
		"pinned",
		"follow",
	] as const)("does not change %s pages from local controls", async (type) => {
		const value = source(type);
		mount(value);
		fireEvent.click(screen.getByRole("button", { name: "▲ PAGE UP" }));
		await act(async () => {});
		expect(value.setSurfacePage).not.toHaveBeenCalled();
		expect(mocks.actions.createPage).not.toHaveBeenCalled();
	});
	it("selects the surface before changing its Own page", async () => {
		const value = source();
		mount(value);
		fireEvent.click(screen.getByRole("button", { name: "▲ PAGE UP" }));
		await act(async () => {});
		expect(value.selectSurface).toHaveBeenCalledWith(configuration.surface_id);
		expect(value.setSurfacePage).toHaveBeenCalledWith(
			configuration.surface_id,
			1,
		);
		expect(
			vi.mocked(value.selectSurface).mock.invocationCallOrder[0],
		).toBeLessThan(vi.mocked(value.setSurfacePage).mock.invocationCallOrder[0]);
	});
	it("creates a missing page before adopting it", async () => {
		const value = source();
		mount(value);
		fireEvent.click(screen.getByRole("button", { name: "PAGE DOWN ▼" }));
		await act(async () => {});
		expect(mocks.actions.createPage).toHaveBeenCalledWith(3);
		expect(value.setSurfacePage).toHaveBeenCalledWith(
			configuration.surface_id,
			3,
		);
	});
	it("does not adopt a rejected creation", async () => {
		mocks.actions.createPage = vi.fn(async () => null);
		const value = source();
		mount(value);
		fireEvent.click(screen.getByRole("button", { name: "PAGE DOWN ▼" }));
		await act(async () => {});
		expect(value.setSurfacePage).not.toHaveBeenCalled();
	});
	it("cannot advance beyond page99", () => {
		mount(source("own", 99));
		expect(screen.getByRole("button", { name: "PAGE DOWN ▼" })).toBeDisabled();
	});
});

describe("compact hardware screen page controls", () => {
	it("uses the Own surface page picker and writes only that surface", async () => {
		mocks.pages.pages.push({ body: { number: 3, name: "Three", assignments: {} } });
		const value = source("own", 2);
		value.surfaces!.command_page = 9;
		render(
			<ScreensProvider source={value}>
				<ScreenPlaybackSection screen={configuration} encoders={<div data-testid="compact-encoders" />} />
			</ScreensProvider>,
		);
		expect(screen.getByTestId("bank")).toHaveAttribute("data-page", "2");
		expect(screen.getByRole("button", { name: "Page 2" })).toBeEnabled();
		expect(screen.queryByRole("button", { name: "PAGE DOWN ▼" })).toBeNull();
		fireEvent.click(screen.getByRole("button", { name: "Page 2" }));
		fireEvent.click(screen.getByRole("button", { name: "3 · Three" }));
		await act(async () => {});
		expect(value.selectSurface).toHaveBeenCalledExactlyOnceWith("screen:screen-1");
		expect(value.setSurfacePage).toHaveBeenCalledExactlyOnceWith("screen:screen-1", 3);
	});

	it("keeps Follow surface compact page read-only", () => {
		render(
			<ScreensProvider source={source("follow", 9)}>
				<ScreenPlaybackSection screen={configuration} encoders={<div />} />
			</ScreensProvider>,
		);
		expect(screen.getByTestId("bank")).toHaveAttribute("data-page", "9");
		expect(screen.getByRole("button", { name: "Page 9" })).toBeDisabled();
	});

	it("preserves hidden page controls in the compact summary", () => {
		render(
			<ScreensProvider source={source("own", 2)}>
				<ScreenPlaybackSection screen={{ ...configuration, show_page_controls: false }} encoders={<div data-testid="compact-encoders" />} />
			</ScreensProvider>,
		);
		expect(screen.getByTestId("compact-encoders")).toBeInTheDocument();
		expect(screen.queryByRole("button", { name: "Page 2" })).toBeNull();
		expect(mocks.pagesViewMounted).toBe(0);
	});
});
