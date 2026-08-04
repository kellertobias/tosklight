import { describe, expect, it } from "vitest";
import { decodeShowObjectBody } from "../api/showObjectBodyWire";
import { windowChoices } from "../components/modals/WindowPicker";
import { builtIns } from "../components/shell/LeftDock";
import builtInWindowTypes from "../types.ts?raw";
import stories from "./MediaPaneWindow.stories.tsx?raw";
import surface from "./media/MediaPaneSurface.tsx?raw";
import { windowRegistry } from "./WindowRegistry";

describe("Media pane Storybook boundary", () => {
	it("keeps the pure Media surface outside every production window launch and persistence surface", () => {
		expect(builtInWindowTypes).not.toMatch(/^\s*\|\s*"media"\s*$/m);
		expect("media" in (windowRegistry as Record<string, unknown>)).toBe(false);
		expect(builtIns.map(([kind]) => String(kind))).not.toContain("media");
		expect(windowChoices.map(([kind]) => String(kind))).not.toContain("media");
		expect(() =>
			decodeShowObjectBody(
				"user_layout",
				{
					desks: [
						{
							id: "desk",
							name: "Desk",
							panes: [
								{
									id: "media",
									kind: "media",
									title: "Media",
									x: 1,
									y: 1,
									width: 12,
									height: 10,
								},
							],
						},
					],
					activeDeskId: "desk",
					windowSettings: {},
				},
				"Media Storybook boundary",
			),
		).toThrow(/known pane kind/);
	});

	it("owns deterministic data and both review compositions in Storybook", () => {
		expect(stories).toMatch(/from\s+["']\.\/media\/MediaPaneSurface["']/);
		expect(stories).toMatch(
			/from\s+["']\.\.\/\.\.\/\.\.\/ui-library\/storybook\/fixtures\/media["']/,
		);
		expect(stories).toMatch(/export\s+const\s+FullBuiltIn\b/);
		expect(stories).toMatch(/export\s+const\s+ConfigurablePane\b/);
		expect(stories).toMatch(/\bPaneView\b/);
		expect(stories).toMatch(/\bMediaPaneSurface\b/);
		expect(surface).not.toMatch(/storybook\/fixtures\/media/);
	});
});
