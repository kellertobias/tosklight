import { describe, expect, it } from "vitest";
import { sameKnownPlayback } from "./playbackTopologyKnownBodies";
import type { PlaybackDefinition } from "./types";
import {
	decodePlaybackBody,
	decodePlaybackPageBody,
} from "./showObjectPlaybackWire";

const definition = {
	name: "Fixture master",
	target: { type: "group", group_id: "310" },
	buttons: ["select", "select_dereferenced", "flash"],
	button_count: 3,
	fader: "master",
	has_fader: true,
	go_activates: true,
	auto_off: true,
	xfade_millis: 0,
	color: "#20c997",
	flash_release: "release_all",
	protect_from_swap: false,
};
describe("sparse unified portable Playback Pages", () => {
	it("retains only authored sparse assignments up to page 99 position 1000", () => {
		const page = decodePlaybackPageBody(
			{
				number: 99,
				name: "Sparse",
				assignments: { "1": definition, "1000": definition },
			},
			"$",
		);
		expect(Object.keys(page.assignments)).toEqual(["1", "1000"]);
		expect(page.assignments["1000"]).not.toHaveProperty("number");
	});
	it.each([0, 100])("rejects page %s", (number) => {
		expect(() =>
			decodePlaybackPageBody({ number, name: "Bad", assignments: {} }, "$"),
		).toThrow();
	});
	it.each([
		"0",
		"1001",
		"01",
		"1.5",
	])("rejects invalid/noncanonical position %s", (position) => {
		expect(() =>
			decodePlaybackPageBody(
				{ number: 1, name: "Bad", assignments: { [position]: definition } },
				"$",
			),
		).toThrow();
	});
	it("rejects split identities and redundant persisted numbers", () => {
		expect(() =>
			decodePlaybackPageBody(
				{
					number: 1,
					name: "Legacy",
					slots: {},
					virtual_playbacks: {},
					assignments: {},
				},
				"$",
			),
		).toThrow();
		expect(() =>
			decodePlaybackBody({ ...definition, number: 1 }, "$"),
		).toThrow();
	});
});

function dynamicAssignment(
	targetBinding: object,
	targetScope: object | null = null,
) {
	return {
		revision: 1,
		target_scope: targetScope,
		dynamic: {
			dynamic_id: "d",
			last_known_pool_number: 1,
			embedded_fallback: {
				definition: { id: "d", target_binding: targetBinding },
			},
		},
		fader_mode: "size_and_master",
		priority: 0,
		resume_policy: "follow_dynamic",
		local_speed_multiplier: { numerator: 1, denominator: 1 },
		crossfade_non_intensity: false,
		auto_off_at_zero: false,
		auto_off_flash_release: false,
		auto_off_full_control: true,
	};
}

describe("canonical Playback configuration reader", () => {
	it("rejects an assignment-owned target scope even when its fallback is bound", () => {
		expect(() =>
			decodePlaybackBody(
				{
					...definition,
					target: {
						type: "dynamic",
						assignment: dynamicAssignment(
							{ type: "live_group", group_id: "1" },
							{ type: "live_group", group_id: "2" },
						),
					},
				},
				"$",
			),
		).toThrow(/target_scope/);
	});
	it("rejects targetless assignment fallback while accepting canonical LiveGroup", () => {
		expect(() =>
			decodePlaybackBody(
				{
					...definition,
					target: {
						type: "dynamic",
						assignment: dynamicAssignment({ type: "targetless" }),
					},
				},
				"$",
			),
		).toThrow(/target_binding/);
		const bound = decodePlaybackBody(
			{
				...definition,
				target: {
					type: "dynamic",
					assignment: dynamicAssignment({ type: "live_group", group_id: "1" }),
				},
			},
			"$",
		);
		expect(bound.target).toMatchObject({
			type: "dynamic",
			assignment: {
				target_scope: null,
				embedded_fallback: {
					target_binding: { type: "live_group", group_id: "1" },
				},
			},
		});
	});
	it("rejects retired SpeedGroup Speed instead of interpreting it as learned percentage", () => {
		expect(() =>
			decodePlaybackBody(
				{
					...definition,
					target: { type: "speed_group", group: "A" },
					fader: "speed",
				},
				"$",
			),
		).toThrow(/fader/);
		const canonical = decodePlaybackBody(
			{
				...definition,
				target: { type: "speed_group", group: "A" },
				fader: "learned_percentage",
			},
			"$",
		);
		expect(canonical.fader).toBe("learned_percentage");
		expect(
			sameKnownPlayback(
				{ ...canonical, fader: "speed" } as PlaybackDefinition,
				canonical,
			),
		).toBe(false);
	});
});
