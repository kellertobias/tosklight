import {
	cleanup,
	fireEvent,
	render as rtlRender,
	screen,
	within,
} from "@testing-library/react";
import { useState, type ReactElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ModalProvider } from "../modals/ModalStack";
import {
	Button,
	CheckboxField,
	ColorPickerField,
	FileDropField,
	FormField,
	FormLayout,
	GroupedSelectionField,
	ICON_CATALOG_GROUPS,
	IconPickerField,
	LargeTextField,
	MultiValueToggleField,
	NumberField,
	SelectField,
	SwitchField,
	TextAreaField,
	TextField,
	validateDroppedFiles,
} from "./controls";
import { HorizontalFaderField } from "./FaderControls";
afterEach(cleanup);
const render = (ui: ReactElement) => rtlRender(ui, { wrapper: ModalProvider });

describe("shared controls", () => {
	it("defaults buttons to a safe type and exposes semantic states", () => {
		render(
			<Button variant="danger" loading>
				Delete
			</Button>,
		);
		const button = screen.getByRole("button");
		expect(button).toHaveAttribute("type", "button");
		expect(button).toBeDisabled();
		expect(button).toHaveClass("ui-danger");
		expect(button).toHaveAttribute("aria-busy", "true");
	});
	it("associates field labels and errors", () => {
		render(<TextField label="Name" error="Required" required />);
		const input = screen.getByLabelText(/Name/);
		expect(input).toHaveAttribute("aria-invalid", "true");
		expect(screen.getByRole("alert")).toHaveTextContent("Required");
	});
	it("renders number controls in minus, field, plus, keyboard order", () => {
		const { container } = render(<NumberField label="Rows" min={1} max={3} />);
		const control = container.querySelector(".ui-number-control")!;
		expect(
			[...control.children].map((child) => child.getAttribute("aria-label")),
		).toEqual(["Decrease value", null, "Increase value", "Open number pad"]);
		expect(control.querySelector(".ui-input-keyboard")).toHaveTextContent("⌨");
		expect(screen.getByLabelText("Rows")).toHaveAttribute(
			"inputmode",
			"numeric",
		);
	});
	it("accepts integer edits when decimal input is disabled", () => {
		const change = vi.fn();
		render(
			<NumberField
				label="Rows"
				value={3}
				min={1}
				max={4}
				onValueChange={change}
			/>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Decrease value" }));
		expect(change).toHaveBeenCalledWith("2");
	});
	it("opens a number-pad modal and commits its draft", () => {
		const change = vi.fn(),
			keyboardCommit = vi.fn();
		render(
			<NumberField
				label="Rows"
				min={1}
				max={3}
				onChange={change}
				onKeyboardCommit={keyboardCommit}
			/>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Open number pad" }));
		expect(screen.getByRole("dialog", { name: "Rows" })).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "3" }));
		fireEvent.click(screen.getByRole("button", { name: "ENTER" }));
		expect(change).toHaveBeenCalled();
		expect(keyboardCommit).toHaveBeenCalledWith("3");
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	});
	it("configures decimal input and hidden steppers", () => {
		render(<NumberField label="Scale" allowDecimal showStepButtons={false} />);
		expect(screen.getByLabelText("Scale")).toHaveAttribute(
			"inputmode",
			"decimal",
		);
		expect(
			screen.queryByRole("button", { name: "Decrease value" }),
		).not.toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Open number pad" }));
		expect(screen.getByRole("button", { name: "." })).toBeInTheDocument();
	});
	it("renders clear before keyboard, clears text, and opens the shared keyboard", () => {
		const change = vi.fn();
		const { container } = render(
			<TextField label="Name" value="Wash" clearable onChange={change} />,
		);
		const control = container.querySelector(".ui-text-control")!;
		expect(
			[...control.children].map((child) => child.getAttribute("aria-label")),
		).toEqual([null, "Clear input", "Open keyboard"]);
		fireEvent.click(screen.getByRole("button", { name: "Clear input" }));
		expect(change).toHaveBeenCalled();
		fireEvent.click(screen.getByRole("button", { name: "Open keyboard" }));
		expect(screen.getByRole("dialog", { name: "Name" })).toBeInTheDocument();
	});
	it("scrolls the multiline textbox with explicit up and down buttons", () => {
		render(<TextAreaField label="Notes" defaultValue={"one\ntwo\nthree"} />);
		const notes = screen.getByLabelText("Notes") as HTMLTextAreaElement;
		notes.scrollBy = vi.fn();
		notes.setSelectionRange(5, 5);
		fireEvent.click(screen.getByRole("button", { name: "Scroll text up" }));
		fireEvent.click(screen.getByRole("button", { name: "Scroll text down" }));
		expect(notes.selectionStart).toBe(5);
		expect(notes.scrollBy).toHaveBeenNthCalledWith(1, {
			top: -66,
			behavior: "smooth",
		});
		expect(notes.scrollBy).toHaveBeenNthCalledWith(2, {
			top: 66,
			behavior: "smooth",
		});
	});
	it("opens the multiline keyboard with a native caret textbox and title-bar Done action", () => {
		render(<TextAreaField label="Notes" defaultValue={"Line one\nLine two"} />);
		fireEvent.click(screen.getByRole("button", { name: "Open keyboard" }));
		const dialog = screen.getByRole("dialog", { name: "Notes" });
		const editor = within(dialog).getByRole("textbox", {
			name: "Notes value",
		}) as HTMLTextAreaElement;
		expect(editor.tagName).toBe("TEXTAREA");
		expect(editor).toHaveValue("Line one\nLine two");
		editor.setSelectionRange(5, 5);
		fireEvent.keyUp(editor, { key: "ArrowLeft" });
		fireEvent.click(within(dialog).getByRole("button", { name: "X" }));
		expect(editor).toHaveValue("Line xone\nLine two");
		expect(editor.selectionStart).toBe(6);
		expect(
			within(dialog).getByLabelText("Full text keyboard"),
		).toBeInTheDocument();
		expect(
			within(dialog).getByRole("button", { name: "Done" }),
		).toBeInTheDocument();
		expect(
			within(dialog).queryByRole("button", { name: "Done · Confirm" }),
		).not.toBeInTheDocument();
	});
	it("operates checkbox and switch controls with unified field labels", () => {
		const change = vi.fn();
		const { container } = render(
			<FormLayout labelPlacement="side">
				<CheckboxField
					label="Dock"
					aria-label="Dock"
					checked
					onChange={change}
				/>
				<SwitchField
					label="Fullscreen"
					aria-label="Fullscreen"
					checked
					onChange={change}
				/>
			</FormLayout>,
		);
		expect(screen.getByText("Checked")).toBeInTheDocument();
		expect(screen.getByText("On")).toBeInTheDocument();
		fireEvent.click(screen.getByRole("checkbox", { name: "Dock" }));
		fireEvent.click(screen.getByRole("switch", { name: "Fullscreen" }));
		expect(change).toHaveBeenCalledTimes(2);
		expect(
			container.querySelectorAll(".ui-form-field.labels-side"),
		).toHaveLength(2);
		expect(container.querySelector(".ui-switch-control")).toBeInTheDocument();
	});
	it("selects one value from a shared multi-value toggle", () => {
		const change = vi.fn();
		render(
			<MultiValueToggleField
				label="Stage view"
				value="2d"
				options={[
					{ value: "2d", label: "2D" },
					{ value: "3d", label: "3D" },
				]}
				onChange={change}
			/>,
		);
		expect(screen.getByRole("radio", { name: "2D" })).toHaveAttribute(
			"aria-checked",
			"true",
		);
		fireEvent.click(screen.getByRole("radio", { name: "3D" }));
		expect(change).toHaveBeenCalledWith("3d");
	});
	it("supports arbitrary horizontal fader ranges", () => {
		const change = vi.fn();
		render(
			<HorizontalFaderField
				label="Environment brightness"
				value={1}
				minimum={0}
				maximum={2}
				step={0.05}
				display="100%"
				onChange={change}
			/>,
		);
		const fader = screen.getByRole("slider", {
			name: "Environment brightness",
		});
		expect(fader).toHaveAttribute("max", "2");
		fireEvent.input(fader, { target: { value: "1.5" } });
		expect(change).toHaveBeenCalledWith(1.5);
	});
	it("never lets the mouse wheel control a range fader", () => {
		const change = vi.fn();
		render(
			<HorizontalFaderField
				label="Environment brightness"
				value={1}
				onChange={change}
			/>,
		);
		const fader = screen.getByRole("slider", {
			name: "Environment brightness",
		});
		fader.focus();
		const wheel = new WheelEvent("wheel", {
			bubbles: true,
			cancelable: true,
			deltaY: 10,
		});
		fader.dispatchEvent(wheel);
		expect(wheel.defaultPrevented).toBe(true);
		expect(fader).not.toHaveFocus();
		expect(change).not.toHaveBeenCalled();
	});
	it("shows an immutable unit in the number input modal", () => {
		render(<NumberField label="Distance" value="2.5" unit="m" />);
		fireEvent.click(screen.getByRole("button", { name: "Open number pad" }));
		expect(screen.getByLabelText("Unit")).toHaveTextContent("m");
		expect(screen.getByLabelText("Distance value")).toHaveTextContent("2.5");
		fireEvent.click(screen.getByRole("button", { name: "Move cursor left" }));
		expect(screen.getByLabelText("Unit")).toHaveTextContent("m");
	});
	it("opens a full-width picker, distinguishes selection from hover, and restores focus", () => {
		const change = vi.fn();
		render(
			<SelectField
				label="Mode"
				value="main"
				options={[
					{ value: "main", label: "Main" },
					{ value: "own", label: "Own" },
				]}
				onChange={change}
			/>,
		);
		const trigger = screen.getByRole("button", { name: /Main/ });
		expect(trigger.querySelector(".ui-select-chevron svg")).toBeInTheDocument();
		expect(trigger).toHaveAttribute("aria-haspopup", "listbox");
		expect(trigger).toHaveAttribute("aria-expanded", "false");
		Object.defineProperties(trigger, {
			offsetWidth: { value: 360 },
			offsetHeight: { value: 50 },
		});
		vi.spyOn(trigger, "getBoundingClientRect").mockReturnValue({
			left: 104.5,
			top: 100,
			right: 455.5,
			bottom: 148.75,
			width: 351,
			height: 48.75,
			x: 104.5,
			y: 100,
			toJSON: () => ({}),
		});
		fireEvent.click(trigger);
		expect(trigger).toHaveAttribute("aria-expanded", "true");
		const listbox = screen.getByRole("listbox");
		expect(trigger).toHaveAttribute("aria-controls", listbox.id);
		expect(trigger).toHaveAttribute(
			"aria-activedescendant",
			`${listbox.id}-option-0`,
		);
		expect(trigger.querySelector(".ui-select-chevron")).toHaveClass("is-open");
		expect(listbox).toHaveStyle({ width: "360px" });
		const selected = screen.getByRole("option", { name: "Main" }),
			hovered = screen.getByRole("option", { name: "Own" });
		expect(selected).toHaveClass("is-active");
		fireEvent.pointerMove(hovered);
		expect(hovered).toHaveClass("is-highlighted");
		expect(hovered).not.toHaveClass("is-active");
		fireEvent.click(hovered);
		expect(change).toHaveBeenCalledWith("own");
		expect(trigger).toHaveFocus();
	});
	it("dismisses the touch picker with escape", () => {
		render(
			<SelectField
				label="Mode"
				value="main"
				options={[{ value: "main", label: "Main" }]}
				onChange={() => undefined}
			/>,
		);
		const trigger = screen.getByRole("button", { name: /Main/ });
		fireEvent.click(trigger);
		fireEvent.keyDown(window, { key: "Escape" });
		expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
		expect(trigger).toHaveFocus();
	});
	it("supports side labels, multiple columns, and arbitrary field content", () => {
		const { container } = render(
			<FormLayout labelPlacement="side" columns={2}>
				<FormField label="Action">
					<Button>Run</Button>
				</FormField>
			</FormLayout>,
		);
		expect(container.querySelector(".ui-form-layout")).toHaveClass(
			"labels-side",
		);
		expect(container.querySelector(".ui-form-layout")).toHaveStyle({
			"--form-columns": "2",
		});
		expect(screen.getByRole("button", { name: "Run" })).toBeInTheDocument();
	});
	it("selects catalog icons and uses the playback color palette for forms", () => {
		const icon = vi.fn(),
			color = vi.fn();
		render(
			<>
				<IconPickerField label="Icon" value="◇" onChange={icon} />
				<ColorPickerField label="Color" value="#f97316" onChange={color} />
			</>,
		);
		fireEvent.click(screen.getByRole("button", { name: /Choose icon/ }));
		fireEvent.click(screen.getByRole("button", { name: "Use ★" }));
		expect(icon).toHaveBeenCalledWith("★");
		fireEvent.click(screen.getByRole("button", { name: /#F97316/ }));
		expect(screen.getByRole("region", { name: "Color" }).parentElement).toBe(
			document.body.lastElementChild,
		);
		expect(screen.getAllByRole("option", { name: /Use color #/ })).toHaveLength(
			16,
		);
		fireEvent.click(screen.getByRole("option", { name: "Use color #06b6d4" }));
		expect(color).toHaveBeenCalledWith("#06b6d4");
	});

	it("uses every repository icon once, supports default groups, and has no custom icon entry", () => {
		const values = ICON_CATALOG_GROUPS.flatMap((group) =>
			group.icons.map((icon) => icon.value),
		);
		expect(new Set(values).size).toBe(values.length);
		expect(ICON_CATALOG_GROUPS.map((group) => group.label)).toEqual([
			"Built-in / General",
			"Beam size",
			"Fixture base",
			"Fixture type",
			"Flash",
			"Functionality",
			"Gobo",
			"Laser shape",
			"Misc",
			"Position",
			"Position beam",
			"Prism",
		]);
		render(
			<IconPickerField
				label="Gobo"
				value="◇"
				defaultGroup="gobo"
				onChange={() => undefined}
			/>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Choose icon" }));
		expect(screen.getByRole("combobox", { name: "Icon group" })).toHaveValue(
			"gobo",
		);
		expect(
			document.querySelector('[data-icon-group="gobo"]'),
		).toBeInTheDocument();
		expect(screen.queryByLabelText("Custom")).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Use custom icon" }),
		).not.toBeInTheDocument();
	});

	it("renders an inset color swatch and square palette choices", () => {
		render(
			<ColorPickerField
				label="Color"
				value="#f97316"
				onChange={() => undefined}
			/>,
		);
		const trigger = screen.getByRole("button", { name: "#F97316" });
		expect(
			trigger.querySelector(".ui-color-trigger-swatch"),
		).toBeInTheDocument();
		fireEvent.click(trigger);
		for (const choice of screen.getAllByRole("option", { name: /Use color/ }))
			expect(choice).toHaveClass("ui-button");
	});

	it("selects grouped options and renders an explicit clear action only when configured", () => {
		function Harness() {
			const [value, setValue] = useState("go");
			return (
				<>
					<GroupedSelectionField
						label="Top button"
						value={value}
						onChange={setValue}
						clearAction={{ label: "Empty Button", value: "none" }}
						groups={[
							{
								label: "Step Control",
								options: [
									{ value: "go", label: "GO", icon: "▶", description: "Advance." },
									{ value: "back", label: "GO MINUS", description: "Return." },
								],
							},
						]}
					/>
					<GroupedSelectionField
						label="Fader"
						value="master"
						onChange={() => undefined}
						groups={[
							{
								label: "Level Control",
								options: [
									{ value: "master", label: "Master", description: "Level." },
								],
							},
						]}
					/>
				</>
			);
		}
		render(<Harness />);
		const iconTrigger = screen.getByRole("button", { name: /GO/ });
		expect(iconTrigger.querySelector(".ui-grouped-selection-icon")).toHaveTextContent("▶");
		expect(iconTrigger.querySelector(".ui-grouped-selection-value")).toHaveClass("has-icon");
		const plainTrigger = screen.getByRole("button", { name: /Master/ });
		expect(plainTrigger.querySelector(".ui-grouped-selection-icon")).not.toBeInTheDocument();
		expect(plainTrigger.querySelector(".ui-grouped-selection-value")).toHaveClass("has-no-icon");
		fireEvent.click(iconTrigger);
		const dialog = screen.getByRole("dialog", { name: "Choose Top button" });
		expect(within(dialog).getByText("Advance.")).toBeVisible();
		fireEvent.click(within(dialog).getByRole("button", { name: /GO MINUS/ }));
		expect(screen.getByRole("button", { name: /GO MINUS/ })).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: /GO MINUS/ }));
		fireEvent.click(screen.getByRole("button", { name: "Empty Button" }));
		expect(screen.getByRole("button", { name: /Empty Button/ })).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: /Master/ }));
		expect(
			within(screen.getByRole("dialog", { name: "Choose Fader" })).queryByRole(
				"button",
				{ name: "Empty Button" },
			),
		).not.toBeInTheDocument();
	});

	it("validates file constraints and delivers an accepted drop exactly once", () => {
		const accepted = new File(["profile"], "fixture.gdtf", {
			type: "application/octet-stream",
		});
		const rejected = new File(["notes"], "notes.txt", { type: "text/plain" });
		expect(
			validateDroppedFiles([accepted], { extensions: [".gdtf"] }),
		).toBeNull();
		expect(validateDroppedFiles([rejected], { extensions: [".gdtf"] })).toMatch(
			/not an accepted/u,
		);
		expect(
			validateDroppedFiles([accepted, accepted], { extensions: [".gdtf"] }),
		).toBe("Choose one file only.");
		const files = vi.fn(),
			reject = vi.fn(),
			picker = vi.fn();
		render(
			<FileDropField
				label="Fixture profile"
				constraints={{ extensions: [".gdtf"] }}
				onFiles={files}
				onRejected={reject}
				onOpenPicker={picker}
			/>,
		);
		const field = screen.getByRole("button", { name: /Choose file/ });
		const transfer = { files: [accepted], dropEffect: "none" };
		fireEvent.dragEnter(field, { dataTransfer: transfer });
		expect(field).toHaveClass("drag-accepted");
		fireEvent.dragEnter(field.querySelector(".ui-file-drop-copy")!, {
			dataTransfer: transfer,
		});
		fireEvent.dragLeave(field.querySelector(".ui-file-drop-copy")!, {
			dataTransfer: transfer,
		});
		expect(field).toHaveClass("drag-accepted");
		fireEvent.drop(field, { dataTransfer: transfer });
		expect(files).toHaveBeenCalledOnce();
		fireEvent.click(field);
		expect(picker).toHaveBeenCalledOnce();
		fireEvent.drop(field, {
			dataTransfer: { files: [rejected], dropEffect: "none" },
		});
		expect(files).toHaveBeenCalledOnce();
		expect(reject).toHaveBeenCalledOnce();
	});
});
