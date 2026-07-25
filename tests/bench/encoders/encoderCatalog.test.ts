import { describe, expect, expectTypeOf, it } from "vitest";
import {
	BeamAttribute,
	ColorAttribute,
	EncoderGroup,
	FocusAttribute,
	IntensityAttribute,
	PositionAttribute,
	ProgrammerToken,
	ShapersAttribute,
	encoderCatalogEntry,
	normalizedEncoderValue,
} from "./encoderCatalog";
import type { BrowserEncoders, NormalizedEncoder } from "./encoderScenario";

describe("encoder catalog", () => {
	it("maps typed group-specific attributes to stable Programmer IDs", () => {
		expect(
			encoderCatalogEntry(EncoderGroup.Intensity, IntensityAttribute.Dimmer),
		).toMatchObject({
			attribute: "intensity",
			label: "Dimmer",
		});
		expect(
			encoderCatalogEntry(EncoderGroup.Position, PositionAttribute.Pan),
		).toMatchObject({
			attribute: "pan",
			label: "Pan",
		});
	});

	it("normalizes single, two-point, and multi-point expressions", () => {
		expect(normalizedEncoderValue(50)).toEqual({
			kind: "normalized",
			value: 0.5,
		});
		expect(
			normalizedEncoderValue([100, ProgrammerToken.Thru, 0]),
		).toEqual({
			kind: "spread",
			value: [1, 0],
		});
		expect(
			normalizedEncoderValue([
				100,
				ProgrammerToken.Thru,
				0,
				ProgrammerToken.Thru,
				100,
			]),
		).toEqual({
			kind: "spread",
			value: [1, 0, 1],
		});
	});

	it.each([
		[[ProgrammerToken.Thru, 50], /lead/],
		[[50, ProgrammerToken.Thru], /end/],
		[
			[50, ProgrammerToken.Thru, ProgrammerToken.Thru, 60],
			/repeat/,
		],
		[[50, 60], /separated/],
		[[101], /between 0 and 100/],
	] as const)("rejects malformed expressions before mutation", (value, error) => {
		expect(() => normalizedEncoderValue(value)).toThrow(error);
	});

	it("keeps attributes beneath their compile-time group", () => {
		expectTypeOf<BrowserEncoders["intensity"][IntensityAttribute.Dimmer]>()
			.toEqualTypeOf<NormalizedEncoder>();
		expectTypeOf<BrowserEncoders["color"][ColorAttribute.Red]>()
			.toEqualTypeOf<NormalizedEncoder>();
		expectTypeOf<BrowserEncoders["position"][PositionAttribute.Pan]>()
			.toEqualTypeOf<NormalizedEncoder>();
		expectTypeOf<BrowserEncoders["beam"][BeamAttribute.Iris]>()
			.toEqualTypeOf<NormalizedEncoder>();
		expectTypeOf<BrowserEncoders["shapers"][ShapersAttribute.Rotation]>()
			.toEqualTypeOf<NormalizedEncoder>();
		expectTypeOf<BrowserEncoders["focus"][FocusAttribute.Zoom]>()
			.toEqualTypeOf<NormalizedEncoder>();
	});
});
