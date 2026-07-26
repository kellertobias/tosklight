import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { CommandSection } from "./CommandSection";

afterEach(cleanup);

function renderSection(mode: "programmer" | "playbacks", hardware: boolean) {
	return render(
		<CommandSection
			mode={mode}
			hardware={hardware}
			commandLine={<div>Command line</div>}
			programmer={<div>Programmer encoders</div>}
			playbacks={<div>Playback bank</div>}
			programmerTools={<div>Programmer keypad</div>}
			playbackTools={<div>Playback tools</div>}
			hardwareTools={<div>Hardware summary</div>}
		/>,
	);
}

describe("CommandSection", () => {
	it("selects the complete software programmer surface", () => {
		const { container } = renderSection("programmer", false);
		expect(screen.getByText("Command line")).toBeInTheDocument();
		expect(screen.getByText("Programmer encoders")).toBeInTheDocument();
		expect(screen.getByText("Programmer keypad")).toBeInTheDocument();
		expect(screen.queryByText("Playback bank")).not.toBeInTheDocument();
		expect(screen.queryByText("Hardware summary")).not.toBeInTheDocument();
		expect(container.firstElementChild).toHaveAttribute(
			"data-hardware-connected",
			"false",
		);
	});

	it("selects playback content while keeping hardware controls external", () => {
		renderSection("playbacks", true);
		expect(screen.getByText("Playback bank")).toBeInTheDocument();
		expect(screen.getByText("Hardware summary")).toBeInTheDocument();
		expect(screen.queryByText("Playback tools")).not.toBeInTheDocument();
		expect(screen.queryByText("Programmer encoders")).not.toBeInTheDocument();
	});
});
