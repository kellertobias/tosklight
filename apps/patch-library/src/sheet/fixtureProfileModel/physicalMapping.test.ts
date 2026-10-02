import { describe, expect, it } from "vitest";
import type { ChannelFunction, FixtureMode, PhysicalMappingCalibration } from "../../fixtureProfile";
import { blankChannel } from "./channels";
import { blankFixtureProfile, blankMode, cloneProfile } from "./defaults";
import { validateProfile } from "./validation";
import { replaceFunctionBehavior } from "../../library/channelModel";
import { removeSlotRowChecked, setRowLevel, slotRows } from "../../library/channelSlots";
import { emptyPhysicalMapping, hasZoomDegreeMapping, nextPhysicalMappingSample, physicalMappingErrors, previewPhysicalValue } from "./physicalMapping";

function zoom(mapping?: Partial<PhysicalMappingCalibration>): ChannelFunction {
	return {
		id: "zoom-function", name: "Zoom", dmx_from: 0, dmx_to: 255,
		attribute: "zoom", priority: 0,
		behavior: { type: "continuous", physical_min: 44, physical_max: 8, unit: "degrees" },
		...(mapping ? { physical_mapping: { ...emptyPhysicalMapping(), ...mapping } } : {}),
	};
}
const samples = [{ raw: 0, physical: 44 }, { raw: 128, physical: 24 }, { raw: 255, physical: 8 }];

describe("portable physical mapping authoring", () => {
	it("keeps absent and empty calibrations linear and unknown without mutating the function", () => {
		const fn = zoom();
		expect(physicalMappingErrors(fn)).toEqual([]);
		expect(previewPhysicalValue(fn, 0)).toBe(44);
		expect(previewPhysicalValue(fn, 255)).toBe(8);
		expect(previewPhysicalValue(fn, 127.5)).toBe(26);
		expect(fn.physical_mapping).toBeUndefined();
		expect(emptyPhysicalMapping()).toEqual({ quality: "unknown", source: null, revision: 0, samples: [] });
		expect(previewPhysicalValue(zoom({ samples: [] }), 127.5)).toBe(26);
		const omittedDefaults = { ...fn, physical_mapping: {} as PhysicalMappingCalibration };
		expect(physicalMappingErrors(omittedDefaults)).toEqual([]);
		expect(previewPhysicalValue(omittedDefaults, 127.5)).toBe(26);
	});

	it("previews descending samples and clamps to the declared endpoints", () => {
		const fn = zoom({ quality: "manufacturer", source: "Manual p. 12", revision: 2, samples, opening_convention: "beam" });
		expect(physicalMappingErrors(fn, 255)).toEqual([]);
		expect(previewPhysicalValue(fn, 64)).toBe(34);
		expect(previewPhysicalValue(fn, 128)).toBe(24);
		expect(previewPhysicalValue(fn, -1)).toBe(44);
		expect(previewPhysicalValue(fn, 256)).toBe(8);
		expect(previewPhysicalValue(fn, Number.NaN)).toBeNull();
	});

	it.each([
		["missing measured source", { quality: "measured" as const }, "need a source"],
		["empty manufacturer source", { quality: "manufacturer" as const, source: "  " }, "need a source"],
		["fractional revision", { revision: 0.5 }, "whole number"],
		["negative revision", { revision: -1 }, "whole number"],
		["overflow revision", { revision: 0x1_0000_0000 }, "whole number"],
		["single sample", { samples: [samples[0]] }, "at least two"],
		["changed endpoint", { samples: [{ raw: 0, physical: 43 }, samples[2]] }, "match the function"],
		["duplicate raw knot", { samples: [samples[0], { raw: 0, physical: 30 }, samples[2]] }, "strictly increase"],
		["fractional raw", { samples: [samples[0], { raw: 128.5, physical: 24 }, samples[2]] }, "whole raw"],
		["outside raw interval", { samples: [samples[0], { raw: 256, physical: 24 }, samples[2]] }, "inside the function"],
		["nonmonotone", { samples: [samples[0], { raw: 64, physical: 20 }, { raw: 128, physical: 30 }, samples[2]] }, "strictly monotonic"],
		["flat sample", { samples: [samples[0], { raw: 128, physical: 44 }, samples[2]] }, "strictly monotonic"],
		["nonfinite sample", { samples: [samples[0], { raw: 128, physical: Infinity }, samples[2]] }, "finite physical"],
		["f32 overflow sample", { samples: [samples[0], { raw: 128, physical: 1e40 }, samples[2]] }, "finite physical"],
	] as const)("rejects %s", (_, mapping, message) => {
		const fn = zoom(mapping as Partial<PhysicalMappingCalibration>);
		expect(physicalMappingErrors(fn).join(" ")).toContain(message);
		expect(previewPhysicalValue(fn, 128)).toBeNull();
	});

	it("uses f32 equality for endpoint values that will cross the Rust JSON boundary", () => {
		const fn: ChannelFunction = { ...zoom({ samples: [{ raw: 0, physical: Math.fround(0.289) }, { raw: 255, physical: 1 }] }),
			behavior: { type: "continuous", physical_min: 0.289, physical_max: 1, unit: "normalized" } };
		expect(physicalMappingErrors(fn)).toEqual([]);
	});

	it.each(["deg", "degree", "degrees", "°", " DEGREES "])("accepts explicit Zoom degree unit %s", (unit) => {
		const fn = zoom({ opening_convention: "field" });
		if (fn.behavior.type === "continuous") fn.behavior.unit = unit;
		expect(hasZoomDegreeMapping(fn)).toBe(true);
		expect(physicalMappingErrors(fn)).toEqual([]);
	});

	it("refuses a beam/field convention for percent, missing units, or another function", () => {
		for (const unit of ["percent", "deg/s", null]) {
			const fn = zoom({ opening_convention: "beam" });
			if (fn.behavior.type === "continuous") fn.behavior.unit = unit;
			expect(physicalMappingErrors(fn).join(" ")).toContain("explicit degree unit");
		}
		expect(physicalMappingErrors({ ...zoom({ opening_convention: "beam" }), attribute: "pan" }).join(" ")).toContain("Zoom function");
	});

	it("retains mappings through clone/JSON and same-type selection, but clears incompatible behavior", () => {
		const profile = blankFixtureProfile();
		const mode = profile.modes[0];
		const fn = zoom({ quality: "measured", source: "Bench A", revision: 7, samples });
		const channel = { ...blankChannel(mode, 1), functions: [fn] };
		mode.channels = [channel];
		expect(cloneProfile(profile).modes[0].channels[0].functions[0]).toEqual(fn);
		expect(JSON.parse(JSON.stringify(fn)).physical_mapping).toEqual(fn.physical_mapping);
		expect(replaceFunctionBehavior({ ...fn, priority: 37 }, "continuous", channel)).toEqual(fn);
		const indexed = replaceFunctionBehavior(fn, "indexed", channel);
		expect(indexed.physical_mapping).toBeNull();
		expect(replaceFunctionBehavior(indexed, "continuous", channel).physical_mapping).toBeNull();
		expect(physicalMappingErrors({ ...indexed, physical_mapping: fn.physical_mapping })).toEqual(["Physical mapping requires a continuous function."]);
	});

	it("skips a wider gap with no f32 interior value and adds a valid descending sample elsewhere", () => {
		const adjacentFloat = 1.0000001192092896;
		const fn = zoom({ samples: [
			{ raw: 0, physical: adjacentFloat }, { raw: 200, physical: 1 }, { raw: 255, physical: 0 },
		] });
		fn.behavior = { type: "continuous", physical_min: adjacentFloat, physical_max: 0, unit: "degrees" };
		const next = nextPhysicalMappingSample(fn);
		expect(next).toMatchObject({ index: 2, sample: { raw: 227 } });
		expect(next!.sample.physical).toBeGreaterThan(0);
		expect(next!.sample.physical).toBeLessThan(1);
		expect(next!.sample.physical).toBe(Math.fround(next!.sample.physical));
		fn.physical_mapping!.samples.splice(next!.index, 0, next!.sample);
		expect(physicalMappingErrors(fn)).toEqual([]);
	});

	it("blocks profile save with a function-scoped error when calibration is invalid", () => {
		const profile = blankFixtureProfile(); profile.name = "Test"; profile.manufacturer = "Test";
		profile.modes[0].channels = [{ ...blankChannel(profile.modes[0], 1), functions: [zoom({ quality: "measured" })] }];
		expect(validateProfile(profile)).toContain("Default: Zoom: Manufacturer and measured mappings need a source.");
	});

	it("keeps calibrated sample raws aligned with function endpoints when adding a fine byte", () => {
		const mode = blankMode(); mode.splits[0].footprint = 2;
		const owner = { ...blankChannel(mode, 1), attribute: "zoom", functions: [zoom({ samples })] };
		const fine = { ...blankChannel(mode, 1), attribute: "zoom" };
		mode.channels = [owner, fine];
		const result = setRowLevel(mode, 1, slotRows(mode, 1)[1], 1);
		expect(result.error).toBeUndefined();
		const fn = result.mode!.channels[0].functions[0];
		expect(result.mode!.channels[0].resolution).toBe("u16");
		expect(fn.physical_mapping?.samples.map((sample) => sample.raw)).toEqual([0, 32768, 65535]);
		expect(physicalMappingErrors(fn, 65535)).toEqual([]);
	});

	it("refuses a U16→U8 reduction that would merge sample knots and leaves the whole draft unchanged", () => {
		const mode = blankMode(); mode.splits[0].footprint = 3;
		const fn = { ...zoom({ quality: "measured", source: "Bench", revision: 3,
			samples: [{ raw: 0, physical: 44 }, { raw: 1, physical: 24 }, { raw: 65535, physical: 8 }] }), dmx_to: 65535 };
		const calibrated = { ...blankChannel(mode, 1), attribute: "zoom", resolution: "u16" as const, secondary_slots: [2],
			default_raw: 32768, highlight_raw: 65535, functions: [fn] };
		const other = { ...blankChannel(mode, 1), attribute: "dimmer" };
		mode.channels = [calibrated, other];
		mode.control_actions = [{ id: "reset", name: "Reset", semantic: "reset" as never, kind: "momentary" as never,
			duration_millis: null, assignments: [{ channel_id: calibrated.id, active_raw: 65535, inactive_raw: 1 }] }];
		const original = structuredClone(mode);
		const fine = slotRows(mode, 1).find((row) => row.channel.id === calibrated.id && row.level === 1)!;
		const result = setRowLevel(mode, 1, fine, 0);
		expect(result.mode).toBeUndefined();
		expect(result.error).toContain("zoom cannot become 8-bit");
		expect(result.error).toContain("merge sample points");
		const removal = removeSlotRowChecked(mode, 1, fine);
		expect(removal.mode).toBeUndefined();
		expect(removal.error).toContain("merge sample points");
		expect(mode).toEqual(original);
		expect(mode.channels[0].functions[0].physical_mapping?.samples.map((sample) => sample.raw)).toEqual([0, 1, 65535]);
	});

	it("accepts 8/16/24/32-bit conversions of a descending curve with stable identities, defaults and native references", () => {
		const mode = blankMode(); mode.splits[0].footprint = 4;
		const fn = zoom({ quality: "manufacturer", source: "Manual p. 12", revision: 2, samples });
		const coarse = { ...blankChannel(mode, 1), attribute: "zoom", default_raw: 128, highlight_raw: 255, functions: [fn] };
		const bytes = [2, 3, 4].map(() => ({ ...blankChannel(mode, 1), attribute: "zoom" }));
		mode.channels = [coarse, ...bytes];
		mode.control_actions = [{ id: "reset", name: "Reset", semantic: "reset" as never, kind: "momentary" as never,
			duration_millis: null, assignments: [{ channel_id: coarse.id, active_raw: 255, inactive_raw: 128 }] }];
		const physical = samples.map((sample) => sample.physical);
		const expectState = (next: FixtureMode, resolution: string, secondary: number[], sampleRaws: number[],
			defaultRaw: number, highlight: number, active: number, inactive: number) => {
			const channel = next.channels.find((candidate) => candidate.id === coarse.id)!;
			const mapped = channel.functions[0];
			expect(channel.resolution).toBe(resolution);
			expect(channel.secondary_slots).toEqual(secondary);
			expect(mapped.id).toBe(fn.id);
			expect(mapped.physical_mapping).toMatchObject({ quality: "manufacturer", source: "Manual p. 12", revision: 2 });
			expect(mapped.physical_mapping?.samples.map((sample) => sample.raw)).toEqual(sampleRaws);
			expect(mapped.physical_mapping?.samples.map((sample) => sample.physical)).toEqual(physical);
			expect(mapped.dmx_to).toBe(sampleRaws[sampleRaws.length - 1]);
			expect(physicalMappingErrors(mapped, sampleRaws[sampleRaws.length - 1])).toEqual([]);
			expect([channel.default_raw, channel.highlight_raw]).toEqual([defaultRaw, highlight]);
			expect(next.control_actions[0].assignments[0]).toMatchObject({ active_raw: active, inactive_raw: inactive });
		};
		const widen = (current: FixtureMode, slot: number, level: 1 | 2 | 3) => {
			const row = slotRows(current, 1).find((candidate) => candidate.slot === slot && candidate.level === 0)!;
			const result = setRowLevel(current, 1, row, level);
			expect(result.error).toBeUndefined();
			return result.mode!;
		};
		const u16 = widen(mode, 2, 1);
		expectState(u16, "u16", [2], [0, 32768, 65535], 32768, 65535, 65535, 32768);
		const u24 = widen(u16, 3, 2);
		expectState(u24, "u24", [2, 3], [0, 0x80_0000, 0xff_ffff], 0x80_0000, 0xff_ffff, 0xff_ffff, 0x80_0000);
		const u32 = widen(u24, 4, 3);
		expectState(u32, "u32", [2, 3, 4], [0, 0x8000_0000, 0xffff_ffff], 0x8000_0000, 0xffff_ffff, 0xffff_ffff, 0x8000_0000);
		const narrow = (current: FixtureMode, level: 1 | 2 | 3) => {
			const row = slotRows(current, 1).find((candidate) => candidate.channel.id === coarse.id && candidate.level === level)!;
			const result = setRowLevel(current, 1, row, 0);
			expect(result.error).toBeUndefined();
			return result.mode!;
		};
		const back24 = narrow(u32, 3);
		expectState(back24, "u24", [2, 3], [0, 0x80_0000, 0xff_ffff], 0x80_0000, 0xff_ffff, 0xff_ffff, 0x80_0000);
		const back16 = narrow(back24, 2);
		expectState(back16, "u16", [2], [0, 32768, 65535], 32768, 65535, 65535, 32768);
		const back8 = narrow(back16, 1);
		expectState(back8, "u8", [], [0, 128, 255], 128, 255, 255, 128);
	});

	it("keeps save validation strict for a mapping that was already invalid before a conversion", () => {
		const profile = blankFixtureProfile(); profile.name = "Test"; profile.manufacturer = "Test";
		const mode = profile.modes[0]; mode.splits[0].footprint = 2;
		const fn = zoom({ quality: "measured", samples });
		mode.channels = [{ ...blankChannel(mode, 1), attribute: "zoom", functions: [fn] }, { ...blankChannel(mode, 1), attribute: "zoom" }];
		const result = setRowLevel(mode, 1, slotRows(mode, 1)[1], 1);
		expect(result.error).toBeUndefined();
		profile.modes[0] = result.mode!;
		expect(validateProfile(profile)).toContain("Default: Zoom: Manufacturer and measured mappings need a source.");
	});
});
