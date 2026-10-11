import { describe, expect, it } from "vitest";
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
