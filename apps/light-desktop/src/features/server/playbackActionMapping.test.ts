import { expect, it } from "vitest";
import { playbackRequest } from "./playbackActionMapping";

it.each([
	["master", { type: "master", value: 0.999 }],
	["master-absolute", { type: "master_absolute", value: 0.999 }],
	["configured-fader", { type: "configured_fader", number: 2, level: 0.999 }],
	[
		"configured-fader-absolute",
		{ type: "configured_fader_absolute", number: 2, level: 0.999 },
	],
] as const)("keeps %s delivery explicit without changing address or control ownership", (action, expected) => {
	const control_source = {
		surface_id: "software:mounted-control",
		control_id: 8,
	};
	expect(
		playbackRequest({ page: 4, position: 301 }, action, {
			value: 0.999,
			fader: 2,
			control_source,
		}),
	).toEqual({
		address: { kind: "playback", page: 4, position: 301 },
		action: expected,
		control_source,
	});
});
