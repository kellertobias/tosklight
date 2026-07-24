import { describe, expect, it, vi } from "vitest";
import {
	dereferencedGroup,
	fixture,
	fixtureRange,
	group,
	groupRange,
} from "./selectionContract";
import {
	KeypadSelectionTransport,
	logicalSelectionEvents,
	OscSelectionTransport,
	oscSelectionEvents,
	selectionKeys,
} from "./selectionTransportScenario";

describe("selection transport compiler", () => {
	it("preserves ordered fixture/head ranges and mixed Group chunks", () => {
		expect(
			selectionKeys(
				"replace",
				[fixtureRange(101, 105, 2), group(4), dereferencedGroup(9)],
				{ defaultTarget: "FIXTURE" },
			),
		).toEqual([
			"1",
			"0",
			"1",
			".",
			"2",
			"TRU",
			"1",
			"0",
			"5",
			".",
			"2",
			"+",
			"GRP",
			"4",
			"+",
			"GRP",
			"GRP",
			"9",
			"ENT",
		]);
	});

	it("skips absent Group IDs with authority and retains stored empty Groups", () => {
		expect(
			selectionKeys("replace", [groupRange(1, 5)], {
				defaultTarget: "FIXTURE",
				groupNumbers: [1, 3, 5],
			}),
		).toEqual(["GRP", "1", "+", "GRP", "3", "+", "GRP", "5", "ENT"]);
		expect(
			selectionKeys("replace", [groupRange(1, 5)], {
				defaultTarget: "FIXTURE",
			}),
		).toEqual(["GRP", "1", "TRU", "5", "ENT"]);
	});

	it("uses the persistent target and explicit add/remove prefixes", () => {
		expect(
			selectionKeys("add", [fixture(7), group(8)], {
				defaultTarget: "GROUP",
			}),
		).toEqual(["+", "GRP", "7", "+", "8", "ENT"]);
		expect(
			selectionKeys("remove", [fixture(7, 0)], {
				defaultTarget: "FIXTURE",
			}),
		).toEqual(["-", "7", ".", "0", "ENT"]);
	});

	it("builds paired keypad and desk-qualified OSC press/release events", () => {
		expect(
			logicalSelectionEvents("replace", [fixture(1)], {
				defaultTarget: "FIXTURE",
			}),
		).toEqual([
			{ key: "1", phase: "press" },
			{ key: "1", phase: "release" },
			{ key: "ENT", phase: "press" },
			{ key: "ENT", phase: "release" },
		]);
		expect(
			oscSelectionEvents("front", "replace", [fixtureRange(1, 2)], {
				defaultTarget: "FIXTURE",
			}).map(({ address, arguments: arguments_ }) => ({
				address,
				arguments: arguments_,
			})),
		).toEqual([
			{
				address: "/light/front/programmer/digit-1",
				arguments: [true],
			},
			{
				address: "/light/front/programmer/digit-1",
				arguments: [false],
			},
			{ address: "/light/front/programmer/thru", arguments: [true] },
			{ address: "/light/front/programmer/thru", arguments: [false] },
			{
				address: "/light/front/programmer/digit-2",
				arguments: [true],
			},
			{
				address: "/light/front/programmer/digit-2",
				arguments: [false],
			},
			{ address: "/light/front/programmer/enter", arguments: [true] },
			{ address: "/light/front/programmer/enter", arguments: [false] },
		]);
	});

	it("rejects unsupported or unresolved targets before either sink mutates", async () => {
		const keypadSend = vi.fn(async () => undefined);
		const oscSend = vi.fn(async () => undefined);
		const authority = async () => ({
			defaultTarget: "FIXTURE" as const,
			groupNumbers: [1],
		});
		const keypad = new KeypadSelectionTransport(
			{ send: keypadSend },
			authority,
		);
		const osc = new OscSelectionTransport("main", { send: oscSend }, authority);

		await expect(keypad.targets(group(2))).rejects.toThrow(/not present/);
		await expect(osc.targets(groupRange(4, 6))).rejects.toThrow(
			/at least one supported target/,
		);
		expect(keypadSend).not.toHaveBeenCalled();
		expect(oscSend).not.toHaveBeenCalled();
		expect(() =>
			oscSelectionEvents("bad/desk", "replace", [fixture(1)], {
				defaultTarget: "FIXTURE",
			}),
		).toThrow(/desk alias/);
	});

	it("executes every compiled event in exact order", async () => {
		const keypadEvents: string[] = [];
		const oscEvents: string[] = [];
		const authority = async () => ({ defaultTarget: "FIXTURE" as const });
		const keypad = new KeypadSelectionTransport(
			{
				send: async (key, phase) => {
					keypadEvents.push(`${key}:${phase}`);
				},
			},
			authority,
		);
		const osc = new OscSelectionTransport(
			"main",
			{
				send: async (address, arguments_) => {
					oscEvents.push(`${address}:${arguments_[0]}`);
				},
			},
			authority,
		);
		await keypad.add(fixture(9));
		await osc.remove(group(4));

		expect(keypadEvents).toEqual([
			"+:press",
			"+:release",
			"9:press",
			"9:release",
			"ENT:press",
			"ENT:release",
		]);
		expect(oscEvents).toEqual([
			"/light/main/programmer/minus:true",
			"/light/main/programmer/minus:false",
			"/light/main/programmer/group:true",
			"/light/main/programmer/group:false",
			"/light/main/programmer/digit-4:true",
			"/light/main/programmer/digit-4:false",
			"/light/main/programmer/enter:true",
			"/light/main/programmer/enter:false",
		]);
	});
});
