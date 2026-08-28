import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CadDepthMenu } from "./CadDepthMenu";
import type { CadEntity } from "./types";

function entity(id: string, x: number, y: number): CadEntity {
	return {
		id,
		kind: "fixture",
		name: id,
		positionMillimetres: [x, y, 0],
		rotationDegrees: [0, 0, 0],
		sizeMillimetres: [100, 100, 100],
	} as unknown as CadEntity;
}

const entities = [entity("a", 0, 0), entity("b", 10_000, 2_000)];

describe("the range menu", () => {
	it("names the axis it cuts and the picture it shows it from", () => {
		render(
			<CadDepthMenu
				view="left_to_right"
				entities={entities}
				cutPlanes={undefined}
				onChange={vi.fn()}
				onClose={vi.fn()}
			/>,
		);
		// Looking along the rig cuts depth, and the companion picture is the plan.
		expect(screen.getByLabelText("Depth range")).toBeInTheDocument();
		expect(screen.getByText("Plan")).toBeInTheDocument();
	});

	it("calls the height it cuts by its own name", () => {
		render(
			<CadDepthMenu
				view="top_down"
				entities={entities}
				cutPlanes={undefined}
				onChange={vi.fn()}
				onClose={vi.fn()}
			/>,
		);
		expect(screen.getByLabelText("Height range")).toBeInTheDocument();
		expect(screen.getByText("Elevation")).toBeInTheDocument();
	});

	it("writes a dragged line back as a depth in millimetres", () => {
		const onChange = vi.fn();
		render(
			<CadDepthMenu
				view="left_to_right"
				entities={entities}
				cutPlanes={undefined}
				onChange={onChange}
				onClose={vi.fn()}
			/>,
		);
		const preview = document.querySelector(".cad-depth-preview");
		if (!preview) throw new Error("the preview was not rendered");
		// jsdom lays nothing out, so the surface is given a width to measure against.
		preview.getBoundingClientRect = () =>
			({ left: 0, width: 200, top: 0, height: 100 }) as DOMRect;

		const near = screen.getByRole("slider", { name: "Depth from" });
		fireEvent.pointerDown(near, { clientX: 100, button: 0 });
		expect(onChange).toHaveBeenCalledTimes(1);
		const planes = onChange.mock.calls[0][0];
		// Halfway across the preview is halfway through the padded span, not through the rig.
		expect(planes.nearMillimetres).toBeGreaterThan(0);
		expect(planes.nearMillimetres).toBeLessThan(10_000);
		expect(planes.farMillimetres).toBeNull();
	});

	it("keeps the far cut from crossing in front of the near one", () => {
		const onChange = vi.fn();
		render(
			<CadDepthMenu
				view="left_to_right"
				entities={entities}
				cutPlanes={{ nearMillimetres: 6_000, farMillimetres: 9_000 }}
				onChange={onChange}
				onClose={vi.fn()}
			/>,
		);
		const preview = document.querySelector(".cad-depth-preview");
		if (!preview) throw new Error("the preview was not rendered");
		preview.getBoundingClientRect = () =>
			({ left: 0, width: 200, top: 0, height: 100 }) as DOMRect;

		// Dragging the far cut to the very start would put it behind the near one.
		const far = screen.getByRole("slider", { name: "Depth to" });
		fireEvent.pointerDown(far, { clientX: 0, button: 0 });
		expect(onChange.mock.calls[0][0].farMillimetres).toBe(6_000);
	});

	it("closes from its own control and clears the cut from Show all", () => {
		const onClose = vi.fn();
		const onChange = vi.fn();
		render(
			<CadDepthMenu
				view="left_to_right"
				entities={entities}
				cutPlanes={{ nearMillimetres: 1_000, farMillimetres: 2_000 }}
				onChange={onChange}
				onClose={onClose}
			/>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Close range settings" }));
		expect(onClose).toHaveBeenCalledOnce();
		fireEvent.click(screen.getByRole("button", { name: "Show all" }));
		expect(onChange).toHaveBeenCalledWith({
			nearMillimetres: null,
			farMillimetres: null,
		});
	});
});
