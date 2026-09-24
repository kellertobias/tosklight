import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { CadAnnotation } from "./annotations";
import { CadAnnotationLayer } from "./CadAnnotationLayer";

const text = (id: string, font?: string): CadAnnotation => ({
	id,
	view: "top_down",
	kind: "text",
	points: [[0, 0]],
	closed: false,
	text: id,
	textHeightMillimetres: 250,
	...(font === undefined ? {} : { font }),
});

describe("drawn words", () => {
	it("are set in the text's own typeface, and in the screen's own without one", () => {
		render(
			<CadAnnotationLayer
				annotations={[text("Plain"), text("Lettered", "hershey-simplex"), text("Later", "not-shipped")]}
				rotationQuarterTurns={0}
				camera={{ pan: [0, 0], zoom: 0.1 }}
				pendingText={null}
				onCommitText={vi.fn()}
				onCancelText={vi.fn()}
			/>,
		);
		expect(screen.getByText("Lettered").style.fontFamily).toContain("ToskLight CAD Hershey Simplex");
		expect(screen.getByText("Plain").style.fontFamily).toBe("");
		expect(screen.getByText("Later").style.fontFamily).toBe("");
	});
});
