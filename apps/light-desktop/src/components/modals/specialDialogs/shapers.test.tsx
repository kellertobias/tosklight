import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ShapersDialog, shaperBladeGeometry } from "./shapers";

afterEach(cleanup);

function installApertureBounds() {
	const aperture = screen.getByRole("img", { name: "Shaper aperture" });
	vi.spyOn(aperture, "getBoundingClientRect").mockReturnValue({
		x: 0,
		y: 0,
		top: 0,
		left: 0,
		right: 320,
		bottom: 320,
		width: 320,
		height: 320,
		toJSON: () => ({}),
	});
}

describe("ShapersDialog", () => {
	it("renders only advertised blades and module rotation", () => {
		render(
			<ShapersDialog
				attributes={[
					"shaper.blade.1.position",
					"shaper.blade.1.angle",
					"shaper.blade.3.position",
					"shaper.rotation",
				]}
				values={{}}
				disabled={false}
				apply={vi.fn(async (_attribute: string, _value: number) => undefined)}
			/>,
		);

		expect(
			screen.getByRole("slider", { name: "Blade 1 insertion and rotation" }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("slider", { name: "Blade 3 insertion and rotation" }),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("slider", { name: "Blade 2 insertion and rotation" }),
		).not.toBeInTheDocument();
		expect(
			screen.getByRole("slider", { name: "Shaper module rotation" }),
		).toBeInTheDocument();
		expect(screen.getByTestId("shaper-blade-plate-1").tagName).toBe("rect");
		expect(screen.getByTestId("shaper-blade-plate-3").tagName).toBe("rect");
		expect(screen.queryByTestId("shaper-blade-plate-2")).toBeNull();
	});

	it("builds the resulting beam shape from four rectangular masking blades", () => {
		render(
			<ShapersDialog
				attributes={[
					"shaper.blade.1.position",
					"shaper.blade.1.angle",
					"shaper.blade.2.position",
					"shaper.blade.2.angle",
					"shaper.blade.3.position",
					"shaper.blade.3.angle",
					"shaper.blade.4.position",
					"shaper.blade.4.angle",
				]}
				values={{
					"shaper.blade.1.position": { value: 0.5, mixed: false },
					"shaper.blade.2.angle": { value: 1, mixed: false },
				}}
				disabled={false}
				apply={vi.fn(async (_attribute: string, _value: number) => undefined)}
			/>,
		);

		const result = screen.getByTestId("shaper-result-shape");
		expect(result.querySelectorAll("rect.shaper-blade-plate")).toHaveLength(4);
		expect(screen.getByTestId("shaper-blade-plate-1")).toHaveAttribute(
			"data-inner-edge",
			"160",
		);
		expect(screen.getByTestId("shaper-blade-plate-2")).toHaveAttribute(
			"transform",
			"rotate(135 160 160)",
		);
	});

	it("places rectangular blade edges and handles on the same geometry", () => {
		expect(shaperBladeGeometry(1, 0, 0.5)).toEqual({
			innerEdge: 52,
			rotation: 0,
			handle: { x: 160, y: 52 },
		});
		expect(shaperBladeGeometry(4, 1, 0.5)).toEqual({
			innerEdge: 268,
			rotation: 270,
			handle: { x: 268, y: 160 },
		});
	});

	it("maps radial and sideways blade movement to independent attributes", async () => {
		const apply = vi.fn(
			async (_attribute: string, _value: number) => undefined,
		);
		render(
			<ShapersDialog
				attributes={["shaper.blade.1.position", "shaper.blade.1.angle"]}
				values={{
					"shaper.blade.1.position": { value: 0, mixed: false },
					"shaper.blade.1.angle": { value: 0.5, mixed: false },
				}}
				disabled={false}
				apply={apply}
			/>,
		);
		installApertureBounds();
		const blade = screen.getByRole("slider", {
			name: "Blade 1 insertion and rotation",
		});

		fireEvent.pointerDown(blade, { pointerId: 7, clientX: 160, clientY: 56 });
		fireEvent.pointerMove(blade, { pointerId: 7, clientX: 190, clientY: 86 });
		fireEvent.pointerUp(blade, { pointerId: 7 });

		await waitFor(() => {
			expect(apply).toHaveBeenCalledWith(
				"shaper.blade.1.position",
				expect.any(Number),
			);
			expect(apply).toHaveBeenCalledWith(
				"shaper.blade.1.angle",
				expect.any(Number),
			);
		});
		const position = apply.mock.calls.find(
			([attribute]) => attribute === "shaper.blade.1.position",
		)?.[1];
		const angle = apply.mock.calls.find(
			([attribute]) => attribute === "shaper.blade.1.angle",
		)?.[1];
		expect(position).toBeGreaterThan(0);
		expect(angle).toBeGreaterThan(0.5);
	});

	it("uses the outer ring for module rotation only", async () => {
		const apply = vi.fn(
			async (_attribute: string, _value: number) => undefined,
		);
		render(
			<ShapersDialog
				attributes={["shaper.blade.1.position", "shaper.rotation"]}
				values={{}}
				disabled={false}
				apply={apply}
			/>,
		);
		installApertureBounds();
		const ring = screen.getByRole("slider", {
			name: "Shaper module rotation",
		});

		fireEvent.pointerDown(ring, { pointerId: 11, clientX: 160, clientY: 18 });
		expect(apply).not.toHaveBeenCalled();
		fireEvent.pointerMove(ring, { pointerId: 11, clientX: 302, clientY: 160 });
		await waitFor(() =>
			expect(apply).toHaveBeenLastCalledWith("shaper.rotation", 0.75),
		);
		fireEvent.pointerMove(ring, { pointerId: 11, clientX: 160, clientY: 302 });
		fireEvent.pointerUp(ring, { pointerId: 11 });

		await waitFor(() => expect(apply).toHaveBeenCalled());
		expect(
			apply.mock.calls.every(([attribute]) => attribute === "shaper.rotation"),
		).toBe(true);
	});
});
