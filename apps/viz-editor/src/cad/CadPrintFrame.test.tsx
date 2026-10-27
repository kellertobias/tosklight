import { render } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { PrintFrame } from "./CadPrintFrame";
import type { CadPrintPage } from "./types";

it("uses white paperwork on a dark ground in CAD with a blue-accent screen logo", () => {
	const page: CadPrintPage = {
		id: "stage", tileId: "tile", name: "Stage Detail", view: "top_down",
		rotationQuarterTurns: 0, centreMillimetres: [0, 0], widthMillimetres: 15000,
		included: true, orientation: "landscape", showFixtureIds: true,
		showDmxAddresses: false, showMountingHardware: true,
	};
	const view = render(<PrintFrame page={page} entities={[]} underlays={[]}
		camera={{ zoom: 0.1, pan: [0, 0] }} selected={false}
		onSelect={vi.fn()} onChange={vi.fn()} onCamera={vi.fn()} />);
	const block = view.container.querySelector('[aria-label="Stage Detail information"]')!;
	expect(block.querySelector("rect")?.getAttribute("fill")).toBe("#0b0d10");
	const title = [...block.querySelectorAll("text")].find((node) => node.textContent === "Stage Detail");
	expect(title?.getAttribute("fill")).toBe("#f5f5f5");
	expect(block.querySelector("image")?.getAttribute("href")).toContain("tosklight-icon-print-dark.svg");
});
