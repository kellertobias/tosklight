import { describe, expect, it } from "vitest";
import { formatNormalizedRange, formatNormalizedValue } from "./model";
import {
	attributeDomain,
	domainStep,
	domainValue,
	formatAttributeValue,
	normalizedValue,
} from "./attributeDomain";

describe("attribute domains", () => {
	it("reads intensity and colour mix as plain percentages", () => {
		const domain = attributeDomain("intensity", "percent");
		expect(formatAttributeValue(domain, 0)).toBe("0%");
		expect(formatAttributeValue(domain, 1)).toBe("100%");
		expect(normalizedValue(domain, 40)).toBeCloseTo(0.4);
	});

	it("reads pan and tilt out from home in both directions", () => {
		const domain = attributeDomain("pan", "deg");
		expect(formatAttributeValue(domain, 0)).toBe("-100%");
		expect(formatAttributeValue(domain, 0.5)).toBe("0%");
		expect(formatAttributeValue(domain, 1)).toBe("100%");
		expect(normalizedValue(domain, 0)).toBeCloseTo(0.5);
		expect(attributeDomain("head.2.tilt", "deg").kind).toBe("signed-percent");
	});

	it("never reads a hair below home as a negative zero", () => {
		const domain = attributeDomain("pan", "deg");
		expect(formatAttributeValue(domain, 0.4999)).toBe("0%");
	});

	it("reads colour temperature in Kelvin, from the fixture's range when it states one", () => {
		expect(formatAttributeValue(attributeDomain("color.temperature", "K"), 0)).toBe(
			"1000 K",
		);
		expect(formatAttributeValue(attributeDomain("color.temperature", "K"), 1)).toBe(
			"12000 K",
		);
		const stated = attributeDomain("color.temperature", "K", {
			minimum: 2_700,
			maximum: 6_500,
		});
		expect(formatAttributeValue(stated, 0)).toBe("2700 K");
		expect(formatAttributeValue(stated, 1)).toBe("6500 K");
	});

	it("reads an angle in degrees only when the fixture states the travel", () => {
		const stated = attributeDomain("shaper.blade.1.angle", "deg", {
			minimum: -45,
			maximum: 45,
		});
		expect(formatAttributeValue(stated, 0.5)).toBe("0.0°");
		// Without a stated range the desk cannot invent one, so it stays channel percentage.
		expect(attributeDomain("shaper.blade.1.angle", "deg").kind).toBe("percent");
		expect(
			attributeDomain("shaper.blade.1.position", "percent", {
				minimum: -45,
				maximum: 45,
			}).kind,
		).toBe("percent");
	});

	it("steps one detent in the units the operator reads", () => {
		const kelvin = attributeDomain("color.temperature", "K");
		expect(domainStep(kelvin, false)).toBeCloseTo(10 / 11_000);
		expect(domainStep(kelvin, true)).toBeCloseTo(100 / 11_000);
		expect(domainValue(kelvin, domainStep(kelvin, true))).toBeCloseTo(1_100);
	});

	it("carries the domain through the display the encoder actually shows", () => {
		// The encoder reads through formatNormalizedValue/Range, so a domain that stops there
		// never reaches the operator however well the domain itself formats.
		const kelvin = attributeDomain("color.temperature", "K");
		expect(formatNormalizedValue(0.5, kelvin)).toBe("6500 K");
		expect(formatNormalizedRange([0, 1], kelvin)).toBe("1000 K...12000 K");
		expect(formatNormalizedRange([0.5, 0.5], kelvin)).toBe("6500 K");
		const pan = attributeDomain("pan", "deg");
		expect(formatNormalizedValue(0.5, pan)).toBe("0%");
		expect(formatNormalizedRange([0, 1], pan)).toBe("-100%...100%");
	});

	it("still reads as a plain percentage when no domain is given", () => {
		expect(formatNormalizedValue(0.4)).toBe("40%");
		expect(formatNormalizedRange([0.12, 0.8])).toBe("12%...80%");
	});

	it("reads the unit however the profile spells it", () => {
		// The attribute registry says "K" and "deg"; a fixture channel says "kelvin" and "degrees".
		expect(attributeDomain("color.temperature", "kelvin").kind).toBe("kelvin");
		expect(
			attributeDomain("shaper.blade.1.angle", "degrees", {
				minimum: -45,
				maximum: 45,
			}).kind,
		).toBe("degrees");
	});
});
