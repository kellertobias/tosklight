import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ParameterController } from "../useParameterController";
import { FamilyEncoderSlotSurface } from "./FamilyEncoderSlotSurface";
import { PAN, POINT } from "./familyEncoderTestSupport";

const app = vi.hoisted(() => ({ dispatch: vi.fn() }));
vi.mock("../../../../state/AppContext", () => ({
	useOptionalApp: () => app,
}));

/**
 * TL-544 G4: the Point slot (Position page 2) on the software touch encoder steps the ordered
 * Point choices like a hardware detent and offers them in the value pad; the hardware-connected
 * layout keeps reading it and leaves editing to the detents. TL-544 G12: a software drag
 * release finishes the open encoder gesture.
 */

afterEach(cleanup);

const CHOICES = [
	{ value: "origin", label: "Origin", reference: { kind: "origin" } },
	{
		value: "point:p1",
		label: "900 · Singer",
		reference: { kind: "point", point_id: "p1" },
	},
];

function controller(
	hardwareConnected: boolean,
	slot = POINT,
	{ choices = CHOICES, text = "Origin" } = {},
) {
	const familyEncoders = {
		componentSlot: () => ({ ...slot, label: slot === POINT ? "Point" : "Pan" }),
		display: () => ({ value: null, text, source: "requested" as const }),
		step: vi.fn(),
		set: vi.fn(),
		setRange: vi.fn(),
		pointChoices: choices,
		choosePoint: vi.fn(),
		finishGestures: vi.fn(),
	};
	const value = {
		hardwareConnected,
		canWriteValues: true,
		hasProgrammerValue: () => false,
		releaseParameter: async () => undefined,
		familyEncoders,
	} as unknown as ParameterController;
	return { value, familyEncoders };
}

describe("Point slot on the software encoder", () => {
	it("steps to the next and previous Point with the chevrons", () => {
		const { value, familyEncoders } = controller(false);
		render(<FamilyEncoderSlotSurface controller={value} index={0} />);
		fireEvent.click(screen.getByRole("button", { name: "Next Enc 1 · Point value" }));
		fireEvent.click(screen.getByRole("button", { name: "Previous Enc 1 · Point value" }));
		expect(familyEncoders.step.mock.calls).toEqual([
			[0, POINT.descriptor.step],
			[0, -POINT.descriptor.step],
		]);
		expect(familyEncoders.set).not.toHaveBeenCalled();
	});

	it("picks a named Point from the value pad", () => {
		const { value, familyEncoders } = controller(false);
		render(<FamilyEncoderSlotSurface controller={value} index={0} />);
		fireEvent.click(screen.getByRole("button", { name: "Set Enc 1 · Point value" }));
		fireEvent.click(screen.getByRole("button", { name: /900 · Singer/ }));
		expect(familyEncoders.choosePoint).toHaveBeenCalledExactlyOnceWith(0, "point:p1");
		expect(familyEncoders.set).not.toHaveBeenCalled();
	});

	it("opens the Point picker from the hardware-connected display, without a numeric editor", () => {
		const { value, familyEncoders } = controller(true);
		render(<FamilyEncoderSlotSurface controller={value} index={0} />);
		fireEvent.click(screen.getByRole("button", { name: "Encoder 1: Point, Origin" }));
		expect(screen.queryByRole("button", { name: "Enter" })).toBeNull();
		fireEvent.click(screen.getByRole("button", { name: /900 · Singer/ }));
		expect(familyEncoders.choosePoint).toHaveBeenCalledExactlyOnceWith(0, "point:p1");
		expect(familyEncoders.set).not.toHaveBeenCalled();
	});
});

describe("Creating Points from the Point slot (TL-651)", () => {
	const ORIGIN_ONLY = [CHOICES[0]];

	it("reads No Points instead of an em dash while the show has no 3D Point", () => {
		const { value } = controller(false, POINT, { choices: ORIGIN_ONLY, text: "—" });
		render(<FamilyEncoderSlotSurface controller={value} index={0} />);
		expect(screen.getByRole("group", { name: "Enc 1 · Point" })).toHaveTextContent(
			"No Points",
		);
	});

	it("keeps the em dash once the show holds a Point", () => {
		const { value } = controller(false, POINT, { text: "—" });
		render(<FamilyEncoderSlotSurface controller={value} index={0} />);
		expect(screen.getByRole("group", { name: "Enc 1 · Point" })).not.toHaveTextContent(
			"No Points",
		);
	});

	for (const hardware of [false, true])
		it(`offers Create Point and Manage Points in the picker (${hardware ? "hardware-connected" : "software"})`, () => {
			app.dispatch.mockClear();
			const { value } = controller(hardware, POINT, { choices: ORIGIN_ONLY, text: "—" });
			render(<FamilyEncoderSlotSurface controller={value} index={0} />);
			const open = () =>
				fireEvent.click(
					hardware
						? screen.getByRole("button", { name: "Encoder 1: Point, No Points" })
						: screen.getByRole("button", { name: "Set Enc 1 · Point value" }),
				);
			open();
			expect(screen.getByText(/This show has no Points yet/)).toBeInTheDocument();
			fireEvent.click(screen.getByRole("button", { name: "Create Point" }));
			expect(app.dispatch).toHaveBeenLastCalledWith({
				type: "OPEN_BUILTIN",
				kind: "patch",
				patchView: "points",
				patchRequest: "create_point",
			});
			expect(screen.queryByRole("button", { name: "Create Point" })).toBeNull();
			open();
			fireEvent.click(screen.getByRole("button", { name: "Manage Points" }));
			expect(app.dispatch).toHaveBeenLastCalledWith({
				type: "OPEN_BUILTIN",
				kind: "patch",
				patchView: "points",
			});
		});
});

describe("software drag release (TL-544 G12)", () => {
	it("finishes the open encoder gesture when a stepping drag is released", () => {
		vi.useFakeTimers();
		try {
			const { value, familyEncoders } = controller(false, PAN);
			render(<FamilyEncoderSlotSurface controller={value} index={0} />);
			const encoder = screen.getByRole("group", { name: "Enc 1 · Pan" });
			fireEvent.pointerDown(encoder, { pointerId: 1, button: 0, clientY: 200 });
			fireEvent.pointerMove(encoder, { pointerId: 1, clientY: 150 });
			expect(familyEncoders.step).toHaveBeenCalled();
			fireEvent.pointerUp(encoder, { pointerId: 1, clientY: 150 });
			expect(familyEncoders.finishGestures).toHaveBeenCalledOnce();
		} finally {
			vi.useRealTimers();
		}
	});
});
