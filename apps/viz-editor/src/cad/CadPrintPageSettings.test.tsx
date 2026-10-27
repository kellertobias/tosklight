import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CadPrintPageSettings } from "./CadPrintPageSettings";
import { printPageScale, rotatePrintPage, withFixedPrintScale } from "./printLayout";
import type { CadPrintPage } from "./types";
import type { CadPrintPages } from "./useCadPrintPages";

const page: CadPrintPage = {
	id: "ground", tileId: "top", name: "Ground plan", kind: "plan", view: "top_down",
	rotationQuarterTurns: 0, centreMillimetres: [0, 0], widthMillimetres: 12000,
	included: true, orientation: "landscape", showFixtureIds: true,
	showDmxAddresses: false, showMountingHardware: true,
};
function controls() {
	return { change: vi.fn(), remove: vi.fn(), rotate: vi.fn() } as unknown as CadPrintPages;
}

describe("selected print page controls", () => {
	afterEach(cleanup);
	it("offers actual page deletion for both plans and fixture-list sheets", () => {
		const printPages = controls();
		const { rerender } = render(<CadPrintPageSettings page={page} printPages={printPages} />);
		fireEvent.click(screen.getByRole("button", { name: "Delete page" }));
		expect(printPages.remove).toHaveBeenLastCalledWith(page.id);
		rerender(<CadPrintPageSettings page={{ ...page, kind: "fixture_list" }} printPages={printPages} />);
		fireEvent.click(screen.getByRole("button", { name: "Delete page" }));
		expect(printPages.remove).toHaveBeenCalledTimes(2);
		expect(screen.queryByRole("spinbutton", { name: "Scale denominator" })).toBeNull();
	});

	it("sets numeric fixed scale to the true PDF ratio and releases the resize lock", () => {
		const printPages = controls();
		const { rerender } = render(<CadPrintPageSettings page={page} printPages={printPages} />);
		fireEvent.change(screen.getByRole("spinbutton", { name: "Scale denominator" }), { target: { value: "75" } });
		const fixed = vi.mocked(printPages.change).mock.calls[0][1] as CadPrintPage;
		expect(fixed.fixedScaleDenominator).toBe(75);
		expect(printPageScale(fixed)).toBeCloseTo(1 / 75, 12);
		expect(printPageScale(rotatePrintPage(fixed))).toBeCloseTo(1 / 75, 12);
		rerender(<CadPrintPageSettings page={fixed} printPages={printPages} />);
		fireEvent.click(screen.getByRole("checkbox", { name: "Fixed scale" }));
		expect(printPages.change).toHaveBeenLastCalledWith(page.id, { ...fixed, fixedScaleDenominator: undefined });
	});

	it("keeps the frame unchanged for empty or invalid numeric scale input", () => {
		const printPages = controls();
		render(<CadPrintPageSettings page={withFixedPrintScale(page, 50)} printPages={printPages} />);
		const input = screen.getByRole("spinbutton", { name: "Scale denominator" });
		for (const value of ["", "0", "-10", "1000001"]) fireEvent.change(input, { target: { value } });
		expect(printPages.change).not.toHaveBeenCalled();
	});
});
