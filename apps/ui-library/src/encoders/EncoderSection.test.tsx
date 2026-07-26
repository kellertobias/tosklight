import { cleanup, fireEvent, render as rtlRender, screen } from "@testing-library/react";
import type { ReactElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ModalProvider } from "../modals/ModalStack";
import {
	EncoderSection,
	type EncoderSectionModel,
} from "./EncoderSection";

const model: EncoderSectionModel = {
	id: "position",
	label: "Position",
	description: "Pan and tilt",
	encoders: [
		{
			id: "pan",
			slot: 1,
			target: { label: "Pan", display: "20°", role: "Turn" },
			secondary: { label: "Tilt", display: "30°", role: "Press-turn" },
			value: 0.4,
			canRelease: true,
		},
		{
			id: "tilt",
			slot: 2,
			target: { label: "Tilt", display: "Mixed" },
			value: 0.5,
		},
	],
};
const render = (ui: ReactElement) => rtlRender(ui, { wrapper: ModalProvider });

afterEach(cleanup);

describe("EncoderSection", () => {
	it("composes a data-driven touch family and preserves relative callbacks", () => {
		const relative = vi.fn();
		const { container } = render(
			<EncoderSection
				model={model}
				surface="touch"
				callbacks={{ onRelativeChange: relative }}
			/>,
		);

		expect(screen.getByRole("region", { name: "Position" })).toHaveAttribute(
			"data-encoder-family",
			"position",
		);
		expect(
			screen.getByRole("group", { name: "Enc 1 · Pan" }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("group", { name: "Enc 2 · Tilt" }),
		).toBeInTheDocument();
		fireEvent.click(container.querySelector(".touch-encoder-tap-positive")!);
		expect(relative).toHaveBeenCalledWith("pan", 0.01, undefined);
		expect(screen.queryByRole("slider")).not.toBeInTheDocument();
	});

	it("renders the same family as reduced hardware targets", () => {
		const absolute = vi.fn();
		render(
			<div className="hardware-connected">
				<EncoderSection
					model={model}
					surface="hardware"
					callbacks={{ onAbsoluteChange: absolute }}
				/>
			</div>,
		);

		expect(screen.getByLabelText("Encoder 1: Pan, 20°")).toHaveClass(
			"dual-target",
		);
		fireEvent.click(screen.getByLabelText("Encoder 1: Pan, 20°"));
		fireEvent.click(screen.getByRole("button", { name: "5" }));
		fireEvent.click(screen.getByRole("button", { name: "ENTER" }));
		expect(absolute).toHaveBeenCalledWith("pan", 0.05);
	});
});
