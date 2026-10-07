// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { inlineDialogTabs, ParameterControlView } from "./ParameterControlView";
import { encoderAreaStore } from "./useEncoderArea";
import type { ParameterController } from "./useParameterController";

const tabs = vi.hoisted(() => ({ last: null as null | { controller: ParameterController; specialDialog: unknown } }));

vi.mock("./EncoderSurfaces", () => ({
	EncoderSurfaces: () => <div data-testid="encoders" />,
}));
vi.mock("./ParameterFamilyTabs", () => ({
	ParameterFamilyTabs: (props: { controller: ParameterController; specialDialog?: unknown }) => {
		tabs.last = { controller: props.controller, specialDialog: props.specialDialog };
		return <div data-testid="families">{props.specialDialog as never}</div>;
	},
}));

function controller(overrides: Partial<ParameterController> = {}) {
	return {
		visibleEncoderCount: 4,
		family: "Color",
		encoderPage: 2,
		dispatch: vi.fn(),
		selectEncoderGroup: vi.fn(),
		...overrides,
	} as unknown as ParameterController;
}

afterEach(() => {
	cleanup();
	encoderAreaStore.reset();
});

describe("ParameterControlView as the measured Color placement budget", () => {
	it("publishes its actual lower container, not the viewport", () => {
		render(<ParameterControlView controller={controller()} />);
		const surfaces = document.querySelector(".parameter-surfaces");
		expect(encoderAreaStore.get().element).toBe(surfaces);
	});

	it("steps the encoders aside while the compact Color dialog claims the area", () => {
		render(<ParameterControlView controller={controller()} />);
		expect(screen.getByTestId("encoders")).toBeInTheDocument();
		act(() => encoderAreaStore.claim("color"));
		expect(screen.queryByTestId("encoders")).toBeNull();
		act(() => encoderAreaStore.release("color"));
		expect(screen.getByTestId("encoders")).toBeInTheDocument();
	});

	it("routes Special Dialog presses to the inline dialog's page cycle", () => {
		render(<ParameterControlView controller={controller()} />);
		act(() => encoderAreaStore.claim("color"));
		const before = encoderAreaStore.get().cycle;
		fireEvent.click(screen.getByRole("button", { name: "Special Dialog" }));
		expect(encoderAreaStore.get().cycle).toBe(before + 1);
	});
});

describe("family tabs while the compact Color dialog is open", () => {
	it("returns to the encoders on the active Color tab without paging", () => {
		const base = controller();
		inlineDialogTabs(base).selectEncoderGroup("Color", 3);
		expect(base.dispatch).toHaveBeenCalledWith({
			type: "SET_MODAL",
			modal: "specialDialogsOpen",
			value: false,
		});
		expect(base.selectEncoderGroup).not.toHaveBeenCalled();
	});

	it("closes it and switches when another family is chosen", () => {
		const base = controller();
		inlineDialogTabs(base).selectEncoderGroup("Position", 1);
		expect(base.dispatch).toHaveBeenCalledTimes(1);
		expect(base.selectEncoderGroup).toHaveBeenCalledWith("Position", 1);
	});

	it("is only installed while the area is claimed", () => {
		const base = controller();
		render(<ParameterControlView controller={base} />);
		expect(tabs.last?.controller).toBe(base);
		act(() => encoderAreaStore.claim("color"));
		expect(tabs.last?.controller).not.toBe(base);
	});
});
