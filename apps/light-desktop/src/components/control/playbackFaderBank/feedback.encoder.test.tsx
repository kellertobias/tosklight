import { TouchPlaybackCardView } from "@tosklight/ui/playback";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import type {
	PlaybackDefinition,
	PlaybackRuntimeProjection,
} from "../../../api/types";
import { playbackFaderDisplay, playbackFaderValue } from "./feedback";

afterEach(cleanup);

it("accumulates fine encoder wheel steps from fresh authoritative Group projections, with coarse stepping and clamps", () => {
	const playback = {
		target: { type: "group", group_id: "1" },
		fader: "master",
	} as PlaybackDefinition;
	let master = 1;
	const submitted: number[] = [];
	const projection = () =>
		({ target: "group", master }) as PlaybackRuntimeProjection;
	const model = () => {
		const value = playbackFaderValue(playback, undefined, projection(), false);
		return {
			page: 2,
			slot: 301,
			row: 0,
			rowUnits: 4,
			name: "Group",
			assigned: true,
			hasFader: true,
			presentation: "push_encoder" as const,
			actions: [],
			faderValue: value,
			faderLabel: "Master",
			faderDisplay: playbackFaderDisplay(
				playback,
				undefined,
				value,
				projection(),
			),
		};
	};
	const callbacks = {
		onFaderChange: (value: number) => {
			master = value / 100;
			submitted.push(master);
			view.rerender(
				<TouchPlaybackCardView model={model()} callbacks={callbacks} />,
			);
		},
	};
	const view = render(
		<TouchPlaybackCardView model={model()} callbacks={callbacks} />,
	);
	const wheel = (deltaY: number, shiftKey = false) =>
		fireEvent.wheel(screen.getByRole("group", { name: "Master" }), {
			deltaY,
			shiftKey,
		});
	for (let index = 0; index < 10; index++) wheel(100);
	expect(master).toBeCloseTo(0.99, 8);
	expect(submitted[1]).toBeLessThan(submitted[0]);
	expect(screen.getByText("99% master").closest(".playback-rotary-control")).toHaveTextContent(
		"99% master",
	);
	wheel(100, true);
	expect(master).toBeCloseTo(0.98, 8);
	for (let index = 0; index < 120; index++) wheel(100, true);
	expect(master).toBe(0);
	wheel(100);
	expect(master).toBe(0);
	for (let index = 0; index < 120; index++) wheel(-100, true);
	expect(master).toBe(1);
	wheel(-100);
	expect(master).toBe(1);
});

it.each([
	["group", { target: "group", master: 0.999 }],
	[
		"speed_group",
		{ target: "speed_group", runtime: { speed_master_scale: 0.999 } },
	],
	["dynamic", { target: "dynamic", runtime: { fader_value: 0.999 } }],
	["grand_master", { target: "grand_master", runtime: { level: 0.999 } }],
] as const)("retains precise %s control values independently of rounded labels", (type, projection) => {
	const playback = { target: { type }, fader: "master" } as PlaybackDefinition;
	expect(
		playbackFaderValue(
			playback,
			undefined,
			projection as PlaybackRuntimeProjection,
			false,
		),
	).toBeCloseTo(99.9, 8);
});
