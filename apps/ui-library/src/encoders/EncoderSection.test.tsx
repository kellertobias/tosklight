import {
	cleanup,
	fireEvent,
	render as rtlRender,
	screen,
} from "@testing-library/react";
import type { ReactElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ModalProvider } from "../modals/ModalStack";
import { EncoderSection, type EncoderSectionModel } from "./EncoderSection";

const model: EncoderSectionModel = {
	id: "position",
	label: "Position",
	description: "Pan and tilt",
	encoders: [
		{
			id: "pan",
			slot: 1,
			target: { label: "Pan", display: "20°", role: "Turn" },
			secondary: { label: "Tilt", display: "30°", role: "Push-turn" },
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
		expect(relative).toHaveBeenCalledWith("pan", 0.001, undefined);
		expect(screen.queryByRole("slider")).not.toBeInTheDocument();
	});

	it("passes internal-domain scaling and custom increments through the family model", () => {
		const relative = vi.fn();
		const absolute = vi.fn();
		render(
			<EncoderSection
				model={{
					id: "scaled",
					label: "Scaled",
					encoders: [
						{
							id: "dimmer",
							slot: 1,
							target: { label: "Dimmer", display: "52.0%" },
							value: 520,
							minimum: 0,
							maximum: 1000,
							inputScale: 0.1,
							slowStep: 1,
							fastStep: 10,
						},
					],
				}}
				surface="touch"
				callbacks={{
					onRelativeChange: relative,
					onAbsoluteChange: absolute,
				}}
			/>,
		);
		fireEvent.click(document.querySelector(".touch-encoder-tap-positive")!);
		expect(relative).toHaveBeenCalledWith("dimmer", 1, undefined);
		fireEvent.click(
			screen.getByRole("button", { name: "Set Enc 1 · Dimmer value" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "7" }));
		fireEvent.click(screen.getByRole("button", { name: "5" }));
		fireEvent.click(screen.getByRole("button", { name: "ENTER" }));
		expect(absolute).toHaveBeenCalledWith("dimmer", 750);
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
