import { describe, expect, it } from "vitest";
import { formatMib, parseMib } from "./policyValues";
import { revealPatchRow } from "./revealRow";

describe("MIB values", () => {
	it("reads Off or a delay from 0 s to 30 s with decimals", () => {
		expect(parseMib("off")).toEqual({ move_in_black_enabled: false });
		expect(parseMib("Off")).toEqual({ move_in_black_enabled: false });
		expect(parseMib("0")).toEqual({
			move_in_black_enabled: true,
			move_in_black_delay_millis: 0,
		});
		expect(parseMib("2.5s")).toEqual({
			move_in_black_enabled: true,
			move_in_black_delay_millis: 2500,
		});
		expect(parseMib("30")).toEqual({
			move_in_black_enabled: true,
			move_in_black_delay_millis: 30000,
		});
		expect(parseMib("30.1")).toBeNull();
		expect(parseMib("-1")).toBeNull();
		expect(parseMib("")).toBeNull();
		expect(parseMib("soon")).toBeNull();
	});

	it("shows Off, or the delay in seconds", () => {
		const fixture = (enabled: boolean, millis: number) =>
			({
				move_in_black_enabled: enabled,
				move_in_black_delay_millis: millis,
			}) as Parameters<typeof formatMib>[0];
		expect(formatMib(fixture(false, 4000))).toBe("Off");
		expect(formatMib(fixture(true, 0))).toBe("0s");
		expect(formatMib(fixture(true, 2500))).toBe("2.5s");
	});
});

describe("revealing a patch row", () => {
	function table(rowTop: number) {
		const container = document.createElement("section");
		container.className = "patch-table-wrap";
		container.innerHTML =
			"<table><thead><tr><th>ID</th></tr></thead><tbody><tr><td>1</td></tr></tbody></table>";
		const rect = (top: number, height: number, left = 0, width = 400) =>
			({ top, bottom: top + height, left, right: left + width, height, width }) as DOMRect;
		container.getBoundingClientRect = () => rect(0, 200);
		const head = container.querySelector("thead") as HTMLElement;
		head.getBoundingClientRect = () => rect(0, 20);
		const row = container.querySelector("tbody tr") as HTMLElement;
		// A row far wider than the table, as a sheet with every column shown is.
		row.getBoundingClientRect = () => rect(rowTop, 30, -600, 2000);
		container.scrollLeft = 350;
		container.scrollTop = 100;
		return { container, row };
	}

	it("scrolls only up or down, never sideways", () => {
		const below = table(260);
		revealPatchRow(below.row);
		expect(below.container.scrollTop).toBe(190);
		expect(below.container.scrollLeft).toBe(350);

		const above = table(5);
		revealPatchRow(above.row);
		expect(above.container.scrollTop).toBe(85);
		expect(above.container.scrollLeft).toBe(350);

		const visible = table(50);
		revealPatchRow(visible.row);
		expect(visible.container.scrollTop).toBe(100);
	});
});
