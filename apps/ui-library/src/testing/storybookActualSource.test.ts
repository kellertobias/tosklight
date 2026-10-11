import { describe, expect, it } from "vitest";
import {
	actualSourceCoverage,
	actualSourceForStory,
} from "../../storybook/config/actualSource";

describe("Storybook actual source", () => {
	it("maps every included story module to actual source", () => {
		// The retired split VirtualPlaybackGrid story is no longer included.
		const includedStories = Object.keys(import.meta.glob([
			"../**/*.stories.tsx",
			"../../../light-desktop/src/**/*.stories.tsx",
		])).map((path) => path.startsWith("../") && !path.startsWith("../../../")
			? `apps/ui-library/src/${path.slice(3)}`
			: `apps/light-desktop/src/${path.slice("../../../light-desktop/src/".length)}`);
		expect(actualSourceCoverage.map(({ storyPath }) => storyPath)).toEqual(includedStories.sort());
		expect(actualSourceCoverage.some(({ storyPath }) => storyPath.endsWith("/VirtualPlaybackGrid.stories.tsx"))).toBe(false);
		expect(
			actualSourceCoverage.filter(
				({ implementationPaths }) => implementationPaths.length === 0,
			),
		).toEqual([]);
		expect(
			actualSourceCoverage.flatMap(
				({ missingImplementationPaths }) => missingImplementationPaths,
			),
		).toEqual([]);
	});

	it("resolves the added compact hardware encoder story without a retired grid alias", () => {
		const source = actualSourceForStory("<CompactHardwareEncoders />", {
			parameters: { fileName: "../light-desktop/src/components/CompactHardwareEncoders.stories.tsx" },
		});
		expect(source).toContain("// Actual source: apps/light-desktop/src/components/CompactHardwareEncoders.stories.tsx");
		expect(source).toContain("ParameterControlView");
		expect(source).toContain("presentation fixtures, not a simulated authoritative desk");
	});

	it("shows story usage followed by the resolved implementation", () => {
		const source = actualSourceForStory("<Button>Apply</Button>", {
			parameters: {
				fileName: "./src/common/Buttons.stories.tsx",
			},
		});

		expect(source).toContain("// Story usage\n<Button>Apply</Button>");
		expect(source).toContain(
			"// Actual source: apps/ui-library/src/common/controls/foundation.tsx",
		);
		expect(source).toContain("export const Button = forwardRef");
	});

	it("resolves desktop story filenames emitted by Storybook", () => {
		const source = actualSourceForStory("<ChannelsStory />", {
			parameters: {
				fileName: "../light-desktop/src/windows/ChannelsWindow.stories.tsx",
			},
		});

		expect(source).toContain(
			"// Actual source: apps/light-desktop/src/windows/ChannelsWindow.tsx",
		);
		expect(source).toContain("export function ChannelsWindowView");
	});
});
