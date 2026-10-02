import { describe, expect, it } from "vitest";
import type { AttributeEncoderGroup, AttributeEncoderPlacement } from "../attributeEncoderPages";
import { familyEncoderLayout, NATIVE_FIRST_PAGE } from "./familyEncoderLayout";
import { FIXTURE_A, FIXTURE_B, pagesSnapshot } from "./familyEncoderTestSupport";
import {
	currentRaw,
	functionFor,
	nativeColorSlot,
	nativeEncoderPages,
	nativeOperation,
	nativeStep,
} from "./nativeColorSlots";
import { nativeControl, nativePages, wheelControl } from "./nativeColorTestSupport";

const NONE: AttributeEncoderGroup<AttributeEncoderPlacement> | undefined = undefined;

function layout(count: number, native: ReturnType<typeof nativeEncoderPages>, advanced = false) {
	const snapshot = pagesSnapshot(true);
	if (advanced) {
		const color = snapshot.families.find((group) => group.family === "color");
		color?.pages.push({ ...color.pages[0], number: 2 });
	}
	return familyEncoderLayout({
		snapshot,
		family: "Color",
		registryGroup: NONE,
		visibleEncoderCount: count,
		supportsAttribute: () => true,
		nativePages: native,
	});
}

const ids = (page: ReturnType<typeof layout> extends infer L ? L : never, index: number) =>
	(page as NonNullable<ReturnType<typeof layout>>).pages[index].map((slot) =>
		slot?.kind === "component" ? slot.slot.id : null,
	);

describe("Direct Color encoder slots (TL-554)", () => {
	it("keeps full-width native integers and identifies controls by UUID, never by label", () => {
		const widths = [0, 1, 2, 3].map((index) => {
			const control = nativeControl(index);
			const slot = nativeColorSlot(control, control.functions[0], [FIXTURE_A]);
			expect(slot.component).toEqual({
				kind: "native_color",
				component: {
					channel_id: control.channel_id,
					function_id: control.functions[0].function_id,
				},
			});
			expect(slot.descriptor.unit).toBe("native_integer");
			expect(slot.edit).toBe("scalar");
			return [slot.limits?.max, slot.descriptor.step];
		});
		expect(widths).toEqual([
			[255, 1],
			[65_535, 257],
			[16_777_215, 65_793],
			[4_294_967_295, 16_843_009],
		]);
		// A typed 32-bit maximum is sent exactly; out-of-range values clamp to the function.
		const top = nativeColorSlot(nativeControl(3), nativeControl(3).functions[0], []);
		expect(
			nativeOperation(top, { kind: "set", value: { kind: "value", value: 4_294_967_295 } }),
		).toEqual({ kind: "set", value: 4_294_967_295 });
		expect(
			nativeOperation(top, { kind: "set", value: { kind: "value", value: 5e9 } }),
		).toEqual({ kind: "set", value: 4_294_967_295 });
		expect(nativeOperation(top, { kind: "relative", value: 0.4 })).toBeNull();
		expect(nativeOperation(top, { kind: "relative", value: -257.9 })).toEqual({
			kind: "relative",
			value: -257,
		});
	});

	it("edits a discrete wheel only within its current function, as choices elsewhere", () => {
		const wheel = wheelControl(0);
		expect(functionFor(wheel, 15)?.label).toBe("Red");
		expect(functionFor(wheel, null)?.label).toBe("Open");
		const slot = nativeColorSlot(wheel, functionFor(wheel, 15)!, []);
		expect(slot.edit).toBe("unavailable");
		expect(slot.limits).toEqual({ min: 10, max: 19 });
		expect(nativeStep(wheel.functions[1])).toBe(1);
	});

	it("shows the requested Direct recipe first, then the displayed premaster value", () => {
		const pages = nativePages(4);
		const channel = pages.pages[0].controls[1]!.channel_id;
		expect(currentRaw(pages, new Map(), channel)).toBe(32_767);
		expect(currentRaw(pages, new Map([[channel, 9]]), channel)).toBe(9);
		expect(currentRaw(nativePages(4, false), new Map(), channel)).toBeNull();
	});

	it("places native pages on pages 3 and 4 of a 4-encoder layout in Easy and Advanced", () => {
		const native = nativeEncoderPages(nativePages(9), new Map(), [FIXTURE_A, FIXTURE_B]);
		for (const advanced of [false, true]) {
			const color = layout(4, native, advanced);
			expect(color?.pages.length).toBe(4);
			expect(ids(color, NATIVE_FIRST_PAGE - 1)[0]).toMatch(/^native\./);
			expect(ids(color, 3).filter(Boolean)).toHaveLength(4);
		}
		expect(ids(layout(4, native), 1)).toEqual([null, null, null, null]);
		// Nothing beyond eight controls reaches the encoders: the ninth is modal overflow.
		const all = layout(4, native)?.pages.flat().filter((slot) => slot?.kind === "component");
		expect(all?.filter((slot) => slot?.kind === "component" && slot.slot.id.startsWith("native."))).toHaveLength(8);
		// Six encoders fill sequentially after the semantic slots.
		const wide = layout(6, native);
		expect(wide?.pages.length).toBe(3);
		expect(ids(wide, 0).slice(0, 2)).toEqual(["color.red", null]);
		// No verified reference: no native pages, the semantic layout is unchanged.
		expect(nativeEncoderPages({ ...nativePages(4), reference: null }, new Map(), [])).toEqual([]);
		expect(layout(4, [])?.pages.length).toBe(1);
	});
});
