import type {
	PlaybackSurfaceConfiguration,
	FixedScreenPane,
	PlaybackSurfaceLayout,
	ScreenConfiguration,
} from "../../api/types";
import type { DeskModel } from "../../types";

interface ScreenLayout {
	desks: DeskModel[];
	activeDeskId: string;
}

export const DEFAULT_FIXED_SCREEN_PANE: FixedScreenPane = {
	type: "fixture_sheet",
	included_heads: "all",
	order: "fixture_id",
	active_only: false,
	compact_mode: "off",
	cue_list_id: null,
	columns: [
		"id",
		"icon",
		"name",
		"intensity",
		"color",
		"position",
		"beam",
		"focus",
	],
	show_type: true,
	show_group_shortcuts: true,
};

export const DEFAULT_FIXED_SIDE_WIDTH_PERCENT = 25;

export function browserScreenUrl(screenId: string, deskServerUrl: string) {
	const url = new URL(deskServerUrl);
	url.pathname = "/";
	url.search = "";
	url.searchParams.set("screen", screenId);
	url.hash = "";
	return url.toString();
}

export function updateScreenConfiguration(
	screen: ScreenConfiguration,
	changes: Partial<ScreenConfiguration>,
): ScreenConfiguration {
	const next = { ...screen, ...changes };
	if (next.content.type !== "desktop") next.show_dock = false;
	return next;
}

/** Presentation defaults for a newly authored secondary surface. */
export function defaultScreenSurface(
	name = "Screen",
): PlaybackSurfaceConfiguration {
	return {
		name,
		page_source: { type: "follow", surface_id: "main" },
		layout: {
			rows: [
				...Array.from({ length: 5 }, (_, index) => ({
					first_position: 201 + index * 10,
					count: 10,
					presentation: "buttons" as const,
					button_count: 1,
				})),
				{
					first_position: 301,
					count: 5,
					presentation: "push_encoder",
					button_count: 1,
				},
			],
		},
	};
}
export function defaultDeskPlaybackLayout(
	surface: PlaybackSurfaceConfiguration | undefined,
): PlaybackSurfaceLayout | null {
	return surface?.layout ?? null;
}
export function screenPlaybackLayout(
	surface: PlaybackSurfaceConfiguration | undefined,
): PlaybackSurfaceLayout | null {
	return surface?.layout ?? null;
}

export function screenForAddAction(
	screens: ScreenConfiguration[],
	layout: ScreenLayout,
	createId: () => string = () => crypto.randomUUID(),
): ScreenConfiguration {
	const configuredScreen = screens.find((screen) => !screen.desired_open);
	if (configuredScreen) return { ...configuredScreen, desired_open: true };

	return createScreenConfiguration(screens, layout, true, createId);
}

export function createScreenConfiguration(
	screens: ScreenConfiguration[],
	layout: ScreenLayout,
	desiredOpen = false,
	createId: () => string = () => crypto.randomUUID(),
): ScreenConfiguration {
	const id = createId();
	return {
		id,
		surface_id: `screen:${id}`,
		name: `Screen ${screens.length + 1}`,
		layout,
		content: { type: "desktop" },
		show_dock: true,
		show_playbacks: true,
		show_page_controls: true,
		show_programmer: false,
		not_editable: false,
		desired_open: desiredOpen,
		display_id: null,
		bounds: null,
		fullscreen: false,
	};
}
