import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ChannelFunction, FixtureMode } from "../fixtureProfile";
import { blankChannel } from "../sheet/fixtureProfileModel/channels";
import { blankMode } from "../sheet/fixtureProfileModel/defaults";
import { physicalMappingErrors } from "../sheet/fixtureProfileModel/physicalMapping";
import { SlotTable } from "./slotTable";

afterEach(cleanup);

function calibratedMode(knots: number[]): FixtureMode {
	const mode = blankMode();
	mode.splits[0].footprint = 3;
	const physical = [44, 24, 8];
	const fn: ChannelFunction = {
		id: "zoom-function", name: "Zoom", attribute: "zoom", priority: 0, dmx_from: 0, dmx_to: 65535,
		behavior: { type: "continuous", physical_min: 44, physical_max: 8, unit: "degrees" },
		physical_mapping: { quality: "measured", source: "Bench", revision: 4,
			samples: knots.map((raw, index) => ({ raw, physical: physical[index] })) },
	};
	mode.channels = [
		{ ...blankChannel(mode, 1), id: "zoom", attribute: "zoom", resolution: "u16", secondary_slots: [2],
			default_raw: 32768, highlight_raw: 65535, functions: [fn] },
		{ ...blankChannel(mode, 1), id: "dimmer", attribute: "dimmer" },
	];
	return mode;
}

function Harness({ initial, onChange }: { initial: FixtureMode; onChange: (mode: FixtureMode) => void }) {
	const [mode, setMode] = useState(initial);
	return (
		<>
			<SlotTable mode={mode} split={1} attributeRegistry={[]} onEditMapping={() => undefined}
				onChange={(next) => { onChange(next); setMode(next); }} />
			<output data-testid="mode">{JSON.stringify(mode)}</output>
		</>
	);
}
const current = () => JSON.parse(screen.getByTestId("mode").textContent ?? "{}") as FixtureMode;

async function chooseLevel(slot: number, from: string, to: string) {
	fireEvent.click(screen.getByRole("button", { name: `Level for slot ${slot}: ${from}` }));
	fireEvent.click(await screen.findByRole("option", { name: to }));
}

describe("slot table resolution edits", () => {
	it("refuses a knot-merging reduction visibly and never publishes the invalid mode", async () => {
		const initial = calibratedMode([0, 1, 65535]);
		const original = structuredClone(initial);
		const onChange = vi.fn();
		render(<Harness initial={initial} onChange={onChange} />);

		await chooseLevel(2, "Fine", "Coarse");
		expect(screen.getByRole("alert")).toHaveTextContent("cannot become 8-bit");
		expect(screen.getByRole("alert")).toHaveTextContent("merge sample points");
		expect(onChange).not.toHaveBeenCalled();
		expect(current()).toEqual(original);

		// Switching controls after the refusal must not reach the damaging layout another way.
		await chooseLevel(2, "Fine", "Fine");
		await chooseLevel(2, "Fine", "Coarse");
		fireEvent.click(screen.getByRole("button", { name: "Remove zoom fine" }));
		fireEvent.click(await screen.findByRole("button", { name: "Remove slot" }));
		expect(screen.getByRole("alert")).toHaveTextContent("merge sample points");
		expect(onChange).not.toHaveBeenCalled();
		expect(current()).toEqual(original);
		const fn = current().channels[0].functions[0];
		expect(fn.physical_mapping?.samples.map((sample) => sample.raw)).toEqual([0, 1, 65535]);
		expect(physicalMappingErrors(fn, 65535)).toEqual([]);
	});

	it("publishes an accepted reduction and clears the previous refusal", async () => {
		const onChange = vi.fn();
		render(<Harness initial={calibratedMode([0, 1, 65535])} onChange={onChange} />);
		await chooseLevel(2, "Fine", "Coarse");
		expect(screen.getByRole("alert")).toBeInTheDocument();
		cleanup();

		render(<Harness initial={calibratedMode([0, 32768, 65535])} onChange={onChange} />);
		await chooseLevel(2, "Fine", "Coarse");
		expect(screen.queryByRole("alert")).not.toBeInTheDocument();
		expect(onChange).toHaveBeenCalledTimes(1);
		const zoom = current().channels.find((channel) => channel.id === "zoom")!;
		expect(zoom).toMatchObject({ resolution: "u8", secondary_slots: [], default_raw: 128, highlight_raw: 255 });
		expect(zoom.functions[0].physical_mapping?.samples).toEqual([
			{ raw: 0, physical: 44 }, { raw: 128, physical: 24 }, { raw: 255, physical: 8 },
		]);
		expect(physicalMappingErrors(zoom.functions[0], 255)).toEqual([]);
	});
});
