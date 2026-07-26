import {
	cleanup,
	fireEvent,
	render,
	screen,
	within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ModalProvider } from "../modals/ModalStack";
import { ModalNumberEditor } from "./ModalNumberEditor";

afterEach(cleanup);

function renderEditor(
	props: Partial<Parameters<typeof ModalNumberEditor>[0]> = {},
) {
	const onChange = vi.fn();
	const onSubmit = vi.fn();
	const onClose = vi.fn();
	render(
		<ModalProvider>
			<ModalNumberEditor
				ariaLabel="Dimmer value"
				title="Dimmer"
				value="42"
				onChange={onChange}
				onSubmit={onSubmit}
				onClose={onClose}
				{...props}
			/>
		</ModalProvider>,
	);
	return { onChange, onSubmit, onClose };
}

describe("ModalNumberEditor", () => {
	it("adds a two-key-wide vertical fader beside the complete value editor", () => {
		const faderChange = vi.fn();
		const { onChange } = renderEditor({
			fader: {
				label: "Dimmer fader",
				maximum: 100,
				onChange: faderChange,
			},
		});
		const dialog = screen.getByRole("dialog", { name: "Dimmer value" });
		expect(dialog).toHaveClass("with-value-fader");
		expect(dialog.querySelector(".modal-number-editor-content")).toHaveClass(
			"has-fader",
		);
		expect(dialog.querySelector(".modal-number-editor-fader")).toContainElement(
			dialog.querySelector(".vertical-touch-fader"),
		);
		const slider = screen.getByRole("slider", { name: "Dimmer fader" });
		vi.spyOn(slider, "getBoundingClientRect").mockReturnValue({
			bottom: 400,
			height: 300,
			left: 0,
			right: 136,
			top: 100,
			width: 136,
			x: 0,
			y: 100,
			toJSON: () => undefined,
		});
		fireEvent.pointerDown(slider, {
			clientX: 50,
			clientY: 106,
			pointerId: 1,
		});
		expect(onChange).toHaveBeenLastCalledWith("100");
		expect(faderChange).toHaveBeenLastCalledWith(100);
		fireEvent.pointerDown(slider, {
			clientX: 50,
			clientY: 394,
			pointerId: 2,
		});
		expect(onChange).toHaveBeenLastCalledWith("0");
		expect(faderChange).toHaveBeenLastCalledWith(0);
	});

	it("asks before closing a changed value and restores the opening fader value on discard", () => {
		const faderChange = vi.fn();
		const { onChange, onClose } = renderEditor({
			fader: {
				label: "Dimmer fader",
				maximum: 100,
				onChange: faderChange,
			},
		});
		const slider = screen.getByRole("slider", { name: "Dimmer fader" });
		vi.spyOn(slider, "getBoundingClientRect").mockReturnValue({
			bottom: 400,
			height: 300,
			left: 0,
			right: 136,
			top: 100,
			width: 136,
			x: 0,
			y: 100,
			toJSON: () => undefined,
		});
		fireEvent.pointerDown(slider, {
			clientX: 50,
			clientY: 106,
			pointerId: 1,
		});
		fireEvent.click(
			screen.getByRole("button", { name: "Close Dimmer value" }),
		);
		let confirmation = screen.getByRole("dialog", {
			name: "Unsaved Dimmer value changes",
		});
		fireEvent.click(
			within(confirmation).getByText("Stay in modal", { exact: true }),
		);
		expect(
			screen.queryByRole("dialog", {
				name: "Unsaved Dimmer value changes",
			}),
		).not.toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", { name: "Close Dimmer value" }),
		);
		confirmation = screen.getByRole("dialog", {
			name: "Unsaved Dimmer value changes",
		});
		fireEvent.click(
			within(confirmation).getByRole("button", {
				name: "Discard changes",
			}),
		);
		expect(onChange).toHaveBeenLastCalledWith("42");
		expect(faderChange).toHaveBeenLastCalledWith(42);
		expect(onClose).toHaveBeenCalledOnce();
	});

	it("toggles from value input to grouped presets and submits the chosen value", () => {
		const { onChange, onSubmit } = renderEditor({
			presets: {
				selectedValue: "42",
				groups: [
					{
						label: "Intensity",
						options: [
							{ value: "25", label: "Quarter" },
							{ value: "50", label: "Half", description: "Working level" },
						],
					},
				],
			},
		});
		const toggle = screen.getByRole("button", { name: "Show presets" });
		expect(toggle.querySelector('[data-active="true"]')).toHaveTextContent(
			"Value",
		);
		fireEvent.click(toggle);
		expect(
			screen.getByRole("button", { name: "Show value input" }),
		).toHaveAttribute("aria-pressed", "true");
		expect(screen.getByText("Intensity")).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: /Half/ }));
		expect(onChange).toHaveBeenLastCalledWith("50");
		expect(onSubmit).toHaveBeenLastCalledWith("50");
	});

	it("places Release in the title bar", () => {
		const release = vi.fn();
		renderEditor({ onRelease: release, releaseLabel: "Release Dimmer" });
		const releaseButton = screen.getByRole("button", {
			name: "Release Dimmer",
		});
		expect(releaseButton.closest(".ui-modal-title-actions")).not.toBeNull();
		fireEvent.click(releaseButton);
		expect(release).toHaveBeenCalledOnce();
	});
});
