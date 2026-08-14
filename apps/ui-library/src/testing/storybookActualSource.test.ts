import { describe, expect, it } from "vitest";
import {
	actualSourceCoverage,
	actualSourceForStory,
} from "../../storybook/config/actualSource";

describe("Storybook actual source", () => {
	it("maps every included story module to actual source", () => {
		expect(actualSourceCoverage).toHaveLength(64);
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
