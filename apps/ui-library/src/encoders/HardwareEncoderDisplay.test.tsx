import {
	act,
	cleanup,
	fireEvent,
	render as rtlRender,
	screen,
} from "@testing-library/react";
import { createRef, type ReactElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ModalProvider } from "../modals/ModalStack";
import {
	type HardwareEncoderDisplayHandle,
	HardwareEncoderDisplayView,
} from "./HardwareEncoderDisplay";

afterEach(cleanup);
const render = (ui: ReactElement) => rtlRender(ui, { wrapper: ModalProvider });

describe("HardwareEncoderDisplayView", () => {
	it("renders unassigned, single-target, and dual-target states", () => {
		const { rerender } = render(<HardwareEncoderDisplayView slot={1} />);
		expect(screen.getByLabelText("Encoder 1 unassigned")).toBeInTheDocument();
		rerender(
			<HardwareEncoderDisplayView
				slot={1}
				target={{ label: "Dimmer", value: "52%" }}
			/>,
		);
		expect(screen.getByLabelText("Encoder 1: Dimmer, 52%")).toHaveClass(
			"single-target",
		);
		rerender(
			<HardwareEncoderDisplayView
				slot={1}
				target={{ label: "Pan", value: "80° ... 100°" }}
				secondary={{ label: "Tilt", value: "30°", role: "Push-turn" }}
			/>,
		);
		const dual = screen.getByLabelText("Encoder 1: Pan, 80° ... 100°");
		expect(dual).toHaveClass("dual-target");
		expect(
			dual.querySelector(".hardware-encoder-primary-labels"),
		).toHaveTextContent("PanEnc 1");
		expect(
			dual.querySelector(".hardware-encoder-target.hardware-encoder-primary"),
		).toHaveTextContent("80° ... 100°");
		expect(dual.querySelector(".hardware-encoder-divider")).toBeInTheDocument();
		expect(
			dual.querySelector(".hardware-encoder-target.hardware-encoder-secondary"),
		).toHaveTextContent("30°");
		expect(
			dual.querySelector(".hardware-encoder-secondary-labels"),
		).toHaveTextContent("TiltPush-turn");
	});

	it("provides a generic imperative activation path for application hardware adapters", () => {
		const ref = createRef<HardwareEncoderDisplayHandle>();
		const edit = vi.fn();
		render(
			<HardwareEncoderDisplayView
				ref={ref}
				slot={2}
				target={{ label: "Zoom", value: "42%" }}
				editValue={42}
				onEdit={edit}
			/>,
		);
		act(() => ref.current?.activate());
		expect(
			screen.getByRole("dialog", { name: "Encoder 2 value" }),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "5" }));
		fireEvent.click(screen.getByRole("button", { name: "ENTER" }));
		expect(edit).toHaveBeenCalledWith(5);
	});

	it("opens the touched half directly and hardware activation with both target choices", () => {
		const ref = createRef<HardwareEncoderDisplayHandle>();
		const edit = vi.fn();
		const editSecondary = vi.fn();
		render(
			<HardwareEncoderDisplayView
				ref={ref}
				slot={1}
				target={{ label: "Pan", value: "80° ... 100°" }}
				secondary={{ label: "Tilt", value: "30°", role: "Push-turn" }}
				editValue={90}
				secondaryEditValue={30}
				onEdit={edit}
				onSecondaryEdit={editSecondary}
			/>,
		);
		const encoder = screen.getByRole("button", {
			name: "Encoder 1: Pan, 80° ... 100°",
		});
		vi.spyOn(encoder, "getBoundingClientRect").mockReturnValue({
			top: 0,
			bottom: 100,
			left: 0,
			right: 100,
			x: 0,
			y: 0,
			width: 100,
			height: 100,
			toJSON: () => ({}),
		});

		fireEvent.click(encoder, { detail: 1, clientY: 25 });
		expect(
			screen.getByRole("dialog", { name: "Encoder 1 value" }),
		).toHaveTextContent("Pan");
		fireEvent.click(screen.getByRole("button", { name: "5" }));
		fireEvent.click(screen.getByRole("button", { name: "ENTER" }));
		expect(edit).toHaveBeenCalledWith(5);
		expect(editSecondary).not.toHaveBeenCalled();

		fireEvent.click(encoder, { detail: 1, clientY: 75 });
		expect(
			screen.getByRole("dialog", { name: "Encoder 1 value" }),
		).toHaveTextContent("Tilt");
		fireEvent.click(screen.getByRole("button", { name: "7" }));
		fireEvent.click(screen.getByRole("button", { name: "ENTER" }));
		expect(editSecondary).toHaveBeenCalledWith(7);

		act(() => ref.current?.activate());
		const selector = document.querySelector(
			".hardware-encoder-target-selector",
		);
		expect(selector).toHaveTextContent("PanTilt");
		expect(screen.getByRole("button", { name: "Pan" })).toHaveAttribute(
			"aria-pressed",
			"true",
		);
		fireEvent.click(screen.getByRole("button", { name: "Tilt" }));
		expect(screen.getByRole("button", { name: "Tilt" })).toHaveAttribute(
			"aria-pressed",
			"true",
		);
		expect(
			screen.getByRole("dialog", { name: "Encoder 1 value" }),
		).toHaveTextContent("Tilt");
	});

	it("offers Release only with ownership and a callback, then closes without editing", () => {
		const ref = createRef<HardwareEncoderDisplayHandle>();
		const edit = vi.fn();
		const release = vi.fn();
		const rendered = render(
			<HardwareEncoderDisplayView
				ref={ref}
				slot={1}
				target={{ label: "Dimmer", value: "52%" }}
				editValue={52}
				canRelease
				onEdit={edit}
			/>,
		);
		act(() => ref.current?.activate());
		expect(
			screen.queryByRole("button", { name: "Release Dimmer" }),
		).not.toBeInTheDocument();

		rendered.rerender(
			<HardwareEncoderDisplayView
				ref={ref}
				slot={1}
				target={{ label: "Dimmer", value: "52%" }}
				editValue={52}
				canRelease
				onEdit={edit}
				onRelease={release}
			/>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Release Dimmer" }));
		expect(release).toHaveBeenCalledOnce();
		expect(edit).not.toHaveBeenCalled();
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	});
});
