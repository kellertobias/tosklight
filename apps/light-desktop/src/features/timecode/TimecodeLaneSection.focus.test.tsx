import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { TimecodeLaneSection } from "./TimecodeLaneSection";
import type { TimecodeLane } from "../../api/types/timecode";

afterEach(cleanup);
it("keeps expanded Speed Group section context independent from its working lane", () => {
	const lane: TimecodeLane = {
		id: "speed",
		name: "Movement",
		content: { kind: "speed_group", group: "A", keyframes: [] },
	};
	const onToggle = vi.fn();
	const props = {
		title: "Speed Groups",
		className: "timecode-speed-section",
		lanes: [lane],
		duration: 440,
		onToggle,
		renderLane: () => (
			<div className="timecode-editor-lane lane-speed_group">Movement</div>
		),
	};
	const view = render(<TimecodeLaneSection {...props} collapsed={false} />);
	const section = screen.getByRole("region", { name: "Speed Groups lanes" });
	expect(section).toHaveClass("section-expanded");
	expect(section).toHaveAttribute("data-expanded", "true");
	fireEvent.click(screen.getByRole("button", { name: "Speed Groups" }));
	expect(onToggle).toHaveBeenCalledOnce();
	view.rerender(<TimecodeLaneSection {...props} collapsed />);
	expect(section).not.toHaveClass("section-expanded");
	expect(section).toHaveAttribute("data-expanded", "false");
});
