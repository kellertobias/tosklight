import { fireEvent, render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { aVisualizer } from "../../testing/server";
import { LiveVisualizerPreview } from "./LiveVisualizerPreview";

describe("the live visualizer preview", () => {
	it("asks the server for frames of the selected visualizer and the next once one arrives", async () => {
		const { container } = render(
			<LiveVisualizerPreview
				visualizer={aVisualizer({ typeId: 42 })}
				aspectRatio={16 / 9}
			/>,
		);
		const image = container.querySelector("img") as HTMLImageElement;
		expect(image.dataset.live).toBe("true");
		expect(image.getAttribute("src")).toMatch(
			/\/visualizers\/250\/1\/preview\?width=480&height=270&frame=0$/u,
		);

		fireEvent.load(image);
		await new Promise((resolve) => setTimeout(resolve, 120));
		expect(
			(container.querySelector("img") as HTMLImageElement).getAttribute("src"),
		).toMatch(/frame=1$/u);
	});

	it("shows the kind's shipped picture when the server cannot draw one", () => {
		const { container } = render(
			<LiveVisualizerPreview
				visualizer={aVisualizer({ typeId: 42 })}
				aspectRatio={16 / 9}
			/>,
		);
		fireEvent.error(container.querySelector("img") as HTMLImageElement);
		const image = container.querySelector("img") as HTMLImageElement;
		expect(image.dataset.live).toBe("false");
		expect(image.getAttribute("src")).toMatch(/042-matrix-digital-rain\.png$/u);
	});
});
