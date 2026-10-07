import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it } from "vitest";
import type { FixtureChannel } from "../fixtureProfile";
import { blankChannel } from "../sheet/fixtureProfileModel/channels";
import { blankMode } from "../sheet/fixtureProfileModel/defaults";
import { FunctionTable } from "./functionTable";

afterEach(cleanup);

function Editor({ functions }: { functions?: FixtureChannel["functions"] }) {
	const [channel, setChannel] = useState<FixtureChannel>(() => ({
		...blankChannel(blankMode(), 1), attribute: "zoom",
		functions: functions ?? [{ id: "zoom", name: "Zoom", attribute: "zoom", dmx_from: 0, dmx_to: 255, priority: 0,
			behavior: { type: "continuous", physical_min: 44, physical_max: 8, unit: "degrees" } }],
	}));
	return <><FunctionTable channel={channel} attributeRegistry={[]} actionIds={[]} onChange={setChannel} />
		<output data-testid="channel">{JSON.stringify(channel)}</output></>;
}
function readFunction() {
	return (JSON.parse(screen.getByTestId("channel").textContent ?? "{}") as FixtureChannel).functions[0];
}

describe("function physical mapping editor", () => {
	it("opens inside function details, defaults to unknown without writing, and authors a descending sampled curve", async () => {
		render(<Editor />);
		fireEvent.click(screen.getByRole("button", { name: "Details for Zoom" }));
		expect(screen.getByRole("region", { name: "Physical mapping calibration" })).toBeInTheDocument();
		expect(readFunction().physical_mapping).toBeUndefined();
		expect(screen.getByLabelText("Physical mapping midpoint")).toHaveTextContent("Raw 127 → 26.0706 degrees");
		fireEvent.click(screen.getByRole("button", { name: "Use sampled mapping" }));
		fireEvent.click(screen.getByRole("button", { name: "Add intermediate sample" }));
		fireEvent.change(screen.getByLabelText("Sample 2 physical"), { target: { value: "30" } });
		expect(screen.getByLabelText("Physical mapping midpoint")).toHaveTextContent("Raw 127 → 30 degrees");
		fireEvent.click(screen.getByRole("button", { name: /Mapping quality/ }));
		fireEvent.click(await screen.findByRole("option", { name: "Measured" }));
		expect(screen.getByRole("alert")).toHaveTextContent("need a source");
		fireEvent.change(screen.getByLabelText("Mapping source"), { target: { value: "Bench measurement 2026-09-28" } });
		fireEvent.change(screen.getByLabelText("Mapping revision"), { target: { value: "2" } });
		fireEvent.click(screen.getByRole("button", { name: /Zoom opening convention/ }));
		fireEvent.click(await screen.findByRole("option", { name: "Beam angle" }));
		expect(screen.queryByRole("alert")).not.toBeInTheDocument();
		expect(readFunction().physical_mapping).toMatchObject({ quality: "measured", source: "Bench measurement 2026-09-28", revision: 2,
			opening_convention: "beam", samples: [{ raw: 0, physical: 44 }, { raw: 127, physical: 30 }, { raw: 255, physical: 8 }] });
		fireEvent.click(screen.getByRole("button", { name: "Use linear mapping" }));
		expect(readFunction().physical_mapping).toMatchObject({ quality: "measured", revision: 2, samples: [] });
	});

	it("disables adding a sample when the remaining raw gap has no representable interior physical value", () => {
		const adjacentFloat = 1.0000001192092896;
		render(<Editor functions={[{
			id: "zoom", name: "Zoom", attribute: "zoom", dmx_from: 0, dmx_to: 255, priority: 0,
			behavior: { type: "continuous", physical_min: 0, physical_max: adjacentFloat, unit: "degrees" },
			physical_mapping: { quality: "unknown", revision: 0, samples: [
				{ raw: 0, physical: 0 }, { raw: 1, physical: 1 }, { raw: 255, physical: adjacentFloat },
			] },
		}]} />);
		fireEvent.click(screen.getByRole("button", { name: "Details for Zoom" }));
		expect(screen.queryByRole("alert")).not.toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Add intermediate sample" })).toBeDisabled();
		expect(screen.getByText(/No additional sample fits/)).toHaveTextContent("stored physical precision is exhausted");
		expect(readFunction().physical_mapping?.samples).toHaveLength(3);
	});

	it("shows endpoint/nonmonotone errors, retains data, and repairs endpoints only on explicit action", () => {
		render(<Editor />); fireEvent.click(screen.getByRole("button", { name: "Details for Zoom" }));
		fireEvent.click(screen.getByRole("button", { name: "Use sampled mapping" }));
		fireEvent.click(screen.getByRole("button", { name: "Add intermediate sample" }));
		fireEvent.change(screen.getByLabelText("Sample 2 physical"), { target: { value: "50" } });
		expect(screen.getByRole("alert")).toHaveTextContent("strictly monotonic");
		expect(screen.queryByRole("img", { name: "Raw DMX to physical value curve" })).not.toBeInTheDocument();
		expect(readFunction().physical_mapping?.samples[1].physical).toBe(50);
		fireEvent.change(screen.getByLabelText("Sample 2 physical"), { target: { value: "30" } });
		fireEvent.change(screen.getByLabelText("Function physical minimum"), { target: { value: "48" } });
		expect(screen.getByRole("alert")).toHaveTextContent("match the function");
		expect(readFunction().physical_mapping?.samples[0].physical).toBe(44);
		fireEvent.click(screen.getByRole("button", { name: "Use function endpoints" }));
		expect(readFunction().physical_mapping?.samples.map((sample) => sample.physical)).toEqual([48, 30, 8]);
		expect(screen.queryByRole("alert")).not.toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Clear mapping calibration" }));
		expect(readFunction().physical_mapping).toBeNull();
	});
});
