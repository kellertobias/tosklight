import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ColorDialogLayout, type ColorDialogLayoutProps, type ColorDialogPage } from "./ColorDialogLayout";

afterEach(cleanup);

const slots = {
	compactPicker: <div role="application" aria-label="Color picker" tabIndex={0} data-testid="slot-picker" />,
	whiteBlend: <input type="range" aria-label="White Blend" data-testid="slot-blend" />,
	whiteBalance: <div data-testid="slot-balance"><input type="range" aria-label="Temperature" /><input type="range" aria-label="Duv" /></div>,
	expandedControls: <div data-testid="slot-expanded"><input type="range" aria-label="Hue" /></div>,
	approximation: <div data-testid="slot-approximation">Fixture 101 approximated</div>,
};

function props(overrides: Partial<ColorDialogLayoutProps> = {}): ColorDialogLayoutProps {
	return { fits: true, page: "mix", expanded: false, onPage: vi.fn(), onExpand: vi.fn(), onClose: vi.fn(), ...slots, ...overrides };
}
const compactDialog = () => screen.getByRole("dialog", { name: "Color Special Dialog" });
const precedes = (first: Element, second: Element) => Boolean(first.compareDocumentPosition(second) & Node.DOCUMENT_POSITION_FOLLOWING);

describe("ColorDialogLayout compact pages", () => {
	it("orders page one as 2D picker, then White Blend above White balance / Expand", () => {
		render(<ColorDialogLayout {...props()} />);
		const dialog = compactDialog();
		expect(dialog).not.toHaveAttribute("aria-modal");
		expect(dialog.closest(".ui-modal-stack-layer")).toBeNull();
		const page = within(dialog).getByTestId("editor-page");
		expect(page).toHaveAttribute("data-page", "mix");
		const [picker, actions] = [...page.children];
		expect(picker).toBe(screen.getByTestId("slot-picker"));
		expect(actions).toHaveClass("color-dialog-actions");
		const blend = screen.getByTestId("slot-blend");
		const buttons = within(dialog).getAllByRole("button");
		expect(buttons.map(button => button.getAttribute("aria-label") ?? button.textContent)).toEqual(["Switch to White balance", "Expand"]);
		expect(buttons[0]).toHaveTextContent("White balance");
		expect(actions.firstElementChild).toBe(blend);
		for (const button of buttons) expect(precedes(blend, button)).toBe(true);
		expect(screen.queryByTestId("slot-balance")).toBeNull();
		expect(screen.queryByTestId("slot-expanded")).toBeNull();
		expect(screen.queryByTestId("slot-approximation")).toBeNull();
	});

	it("shows only the supplied Temperature/Duv content on page two, without header, footer, status or Encoders", () => {
		render(<ColorDialogLayout {...props({ page: "white" })} />);
		const dialog = compactDialog();
		expect(within(dialog).getByTestId("editor-page")).toHaveAttribute("data-page", "white");
		expect(within(dialog).getByTestId("slot-balance")).toBeInTheDocument();
		expect(screen.queryByTestId("slot-picker")).toBeNull();
		expect(screen.queryByTestId("slot-blend")).toBeNull();
		expect(within(dialog).getAllByRole("button").map(button => button.textContent)).toEqual(["Color", "Expand"]);
		expect(within(dialog).queryByRole("button", { name: /Encoders/ })).toBeNull();
		expect(dialog.querySelector("header, footer, h1, h2, h3, .ui-modal-titlebar, [role=status], [role=alert], [aria-live]")).toBeNull();
		expect(screen.queryByTestId("slot-approximation")).toBeNull();
	});

	it("reports page and expand requests without authoring any Color value", () => {
		const onPage = vi.fn(), onExpand = vi.fn(), onClose = vi.fn();
		const { rerender } = render(<ColorDialogLayout {...props({ onPage, onExpand, onClose })} />);
		fireEvent.click(screen.getByRole("button", { name: "Switch to White balance" }));
		expect(onPage).toHaveBeenLastCalledWith("white");
		fireEvent.click(screen.getByRole("button", { name: "Expand" }));
		expect(onExpand).toHaveBeenCalledTimes(1);
		rerender(<ColorDialogLayout {...props({ page: "white", onPage, onExpand, onClose })} />);
		fireEvent.click(screen.getByRole("button", { name: "Switch to Color" }));
		expect(onPage).toHaveBeenLastCalledWith("mix");
		expect(onClose).not.toHaveBeenCalled();
		const callbacks = Object.entries(props()).filter(([, value]) => typeof value === "function").map(([key]) => key).sort();
		expect(callbacks, "Only navigation callbacks; no authored value callback").toEqual(["onClose", "onExpand", "onPage"]);
	});

	it("focuses the first compact control on open and on page changes, and closes on Escape", () => {
		const onClose = vi.fn();
		const { rerender } = render(<ColorDialogLayout {...props({ onClose })} />);
		expect(screen.getByTestId("slot-picker")).toHaveFocus();
		rerender(<ColorDialogLayout {...props({ page: "white", onClose })} />);
		expect(screen.getByLabelText("Temperature")).toHaveFocus();
		fireEvent.keyDown(document, { key: "Escape" });
		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it("leaves Escape to a stacked modal above the compact surface", () => {
		const onClose = vi.fn();
		render(<ColorDialogLayout {...props({ onClose })} />);
		const layer = document.createElement("div");
		layer.className = "ui-modal-stack-layer"; layer.dataset.modalTop = "true";
		document.body.append(layer);
		fireEvent.keyDown(document, { key: "Escape" });
		layer.remove();
		expect(onClose).not.toHaveBeenCalled();
	});

	it("uses Preview for Media and shows the supplied preview in place of White balance", () => {
		const preview = <div data-testid="slot-media">Processed media</div>;
		const { rerender } = render(<ColorDialogLayout {...props({ mediaPreview: preview })} />);
		expect(screen.getByRole("button", { name: "Switch to Preview" })).toHaveTextContent("Preview");
		rerender(<ColorDialogLayout {...props({ page: "white", mediaPreview: preview })} />);
		expect(screen.getByTestId("editor-page")).toHaveAttribute("data-page", "preview");
		expect(screen.getByTestId("slot-media")).toBeInTheDocument();
		expect(screen.queryByTestId("slot-balance")).toBeNull();
	});
});

const details = (dialog: HTMLElement, name = "Details") =>
	fireEvent.click(within(dialog).getByRole("tab", { name }));

describe("ColorDialogLayout full modal", () => {
	it("uses the standard ModalFrame with a Color tab for the selection and a Details tab for the rest", async () => {
		const onClose = vi.fn();
		render(<ColorDialogLayout {...props({ expanded: true, onClose, native: <div data-testid="slot-native" /> })} />);
		const dialog = await screen.findByRole("dialog", { name: "Color Special Dialog" });
		expect(dialog).toHaveAttribute("aria-modal", "true");
		expect(dialog.closest(".ui-modal-stack-layer")).toHaveClass("color-dialog-layer");
		const tabs = within(dialog).getByRole("tablist", { name: "Color dialog tabs" });
		expect(within(tabs).getAllByRole("tab").map(tab => tab.textContent)).toEqual(["Color", "Details"]);
		expect(within(tabs).getByRole("tab", { name: "Color" })).toHaveAttribute("aria-selected", "true");
		// The Color tab is the selection alone.
		const body = within(dialog).getByTestId("editor-page");
		const controls = within(dialog).getByTestId("full-color-editor");
		expect(controls).toContainElement(screen.getByTestId("slot-expanded"));
		expect([...body.children]).toEqual([controls]);
		expect(within(dialog).queryByRole("region", { name: "Color approximation" })).toBeNull();
		// Details holds the approximation and Direct color, in that order, and nothing of the selection.
		details(dialog);
		const approximation = within(dialog).getByRole("region", { name: "Color approximation" });
		const direct = within(dialog).getByRole("region", { name: "Direct color" });
		expect(approximation).toContainElement(screen.getByTestId("slot-approximation"));
		expect(direct).toContainElement(screen.getByTestId("slot-native"));
		expect([...within(dialog).getByTestId("editor-page").children]).toEqual([approximation, direct]);
		expect(screen.queryByTestId("slot-expanded")).toBeNull();
		expect(within(dialog).queryByRole("button", { name: "Expand" })).toBeNull();
		expect(within(dialog).queryByRole("button", { name: /^Switch to/ })).toBeNull();
		expect(screen.queryByTestId("slot-picker")).toBeNull();
		expect(dialog.querySelector("[role=status], [role=alert], [aria-live]")).toBeNull();
		fireEvent.click(within(dialog).getByRole("button", { name: "Close Special Dialog" }));
		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it("opens the same modal when the compact budget does not fit", async () => {
		render(<ColorDialogLayout {...props({ fits: false, page: "white" })} />);
		const dialog = await screen.findByRole("dialog", { name: "Color Special Dialog" });
		expect(dialog).toHaveAttribute("aria-modal", "true");
		expect(within(dialog).getByTestId("slot-expanded")).toBeInTheDocument();
		details(dialog);
		expect(within(dialog).getByTestId("slot-approximation")).toBeInTheDocument();
	});

	it("titles Media color and shows the supplied preview on a Preview tab instead of an approximation", async () => {
		render(<ColorDialogLayout {...props({ expanded: true, mediaPreview: <div data-testid="slot-media" /> })} />);
		const dialog = await screen.findByRole("dialog", { name: "Color Special Dialog" });
		expect(dialog.querySelector(".ui-modal-titlebar")).toHaveTextContent("Media color");
		details(dialog, "Preview");
		expect(within(dialog).getByRole("region", { name: "Media preview" })).toContainElement(screen.getByTestId("slot-media"));
		expect(within(dialog).queryByRole("region", { name: "Color approximation" })).toBeNull();
	});

	it("shows no tabs when there is nothing beside the selection", async () => {
		render(<ColorDialogLayout {...props({ expanded: true, approximation: undefined })} />);
		const dialog = await screen.findByRole("dialog", { name: "Color Special Dialog" });
		expect(within(dialog).queryByRole("tablist")).toBeNull();
		expect(within(dialog).getByTestId("slot-expanded")).toBeInTheDocument();
	});

	it("reopens on the Color tab", async () => {
		const { rerender } = render(<ColorDialogLayout {...props({ expanded: true })} />);
		details(await screen.findByRole("dialog", { name: "Color Special Dialog" }));
		rerender(<ColorDialogLayout {...props({ expanded: false })} />);
		rerender(<ColorDialogLayout {...props({ expanded: true })} />);
		const dialog = await screen.findByRole("dialog", { name: "Color Special Dialog" });
		expect(within(dialog).getByRole("tab", { name: "Color" })).toHaveAttribute("aria-selected", "true");
	});

	it("keeps approximation changes passive: no focus steal, alert or announcement", async () => {
		const { rerender } = render(<ColorDialogLayout {...props({ expanded: true })} />);
		const dialog = await screen.findByRole("dialog", { name: "Color Special Dialog" });
		details(dialog);
		const tab = within(dialog).getByRole("tab", { name: "Details" });
		await act(() => new Promise(resolve => requestAnimationFrame(resolve)));
		tab.focus();
		rerender(<ColorDialogLayout {...props({ expanded: true, approximation: <div data-testid="slot-approximation">Wheel approximates Dark blue</div> })} />);
		expect(tab).toHaveFocus();
		expect(screen.getByTestId("slot-approximation")).toHaveTextContent("Dark blue");
		expect(document.querySelector("[role=alert], [role=status], [aria-live]")).toBeNull();
	});
});

/** Caller-owned state; the layout only toggles page and expansion. */
function Harness({ initialFits = true }: { initialFits?: boolean }) {
	const [page, setPage] = useState<ColorDialogPage>("mix");
	const [expanded, setExpanded] = useState(false);
	const [fits, setFits] = useState(initialFits);
	const [white, setWhite] = useState(37);
	const [temperature, setTemperature] = useState(3200);
	const blend = <input type="range" aria-label="White Blend" min={0} max={100} value={white} onChange={event => setWhite(Number(event.target.value))} />;
	const balance = <input type="range" aria-label="Temperature" min={1000} max={20000} step={100} value={temperature} onChange={event => setTemperature(Number(event.target.value))} />;
	return <>
		<button type="button" onClick={() => setFits(current => !current)}>Resize</button>
		<ColorDialogLayout fits={fits} page={page} expanded={expanded} onPage={setPage} onExpand={() => setExpanded(true)} onClose={() => setExpanded(false)}
			compactPicker={<div role="application" aria-label="Color picker" tabIndex={0} />} whiteBlend={blend} whiteBalance={balance}
			expandedControls={<>{blend}{balance}</>} approximation={<output aria-label="Request">{white}% · {temperature} K</output>} />
	</>;
}

describe("ColorDialogLayout controlled content", () => {
	it("keeps caller values through page changes, expansion and resize", async () => {
		render(<Harness />);
		fireEvent.change(screen.getByLabelText("White Blend"), { target: { value: "99" } });
		fireEvent.click(screen.getByRole("button", { name: "Switch to White balance" }));
		fireEvent.change(screen.getByLabelText("Temperature"), { target: { value: "19900" } });
		fireEvent.click(screen.getByRole("button", { name: "Switch to Color" }));
		expect(screen.getByLabelText("White Blend")).toHaveValue("99");
		fireEvent.click(screen.getByRole("button", { name: "Expand" }));
		const dialog = await screen.findByRole("dialog", { name: "Color Special Dialog" });
		expect(dialog).toHaveAttribute("aria-modal", "true");
		expect(within(dialog).getByLabelText("White Blend")).toHaveValue("99");
		expect(within(dialog).getByLabelText("Temperature")).toHaveValue("19900");
		details(dialog);
		expect(within(dialog).getByLabelText("Request")).toHaveTextContent("99% · 19900 K");
		fireEvent.click(within(dialog).getByRole("button", { name: "Close Special Dialog" }));
		// Compact again: the page the operator left is preserved.
		expect(screen.getByRole("button", { name: "Switch to White balance" })).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Resize" }));
		const narrow = await screen.findByRole("dialog", { name: "Color Special Dialog" });
		expect(narrow).toHaveAttribute("aria-modal", "true");
		expect(within(narrow).getByLabelText("White Blend")).toHaveValue("99");
		expect(within(narrow).getByLabelText("Temperature")).toHaveValue("19900");
	});
});

describe("ColorDialogLayout geometry", () => {
	it("keeps the accepted compact column budgets and button stacking", () => {
		const { rerender } = render(<ColorDialogLayout {...props()} />);
		const page = () => screen.getByTestId("editor-page");
		const buttons = () => compactDialog().querySelector(".color-dialog-buttons")!;
		expect(getComputedStyle(page()).gridTemplateColumns).toBe("minmax(0, 1fr) 230px");
		expect(getComputedStyle(buttons()).flexDirection).not.toBe("column");
		expect(getComputedStyle(compactDialog()).height).toBe("100%");
		rerender(<ColorDialogLayout {...props({ page: "white" })} />);
		expect(getComputedStyle(page()).gridTemplateColumns).toBe("minmax(0, 1fr) 170px");
		expect(getComputedStyle(buttons()).flexDirection).toBe("column");
		rerender(<ColorDialogLayout {...props({ page: "white", mediaPreview: <div /> })} />);
		expect(getComputedStyle(page()).gridTemplateColumns).toBe("minmax(0, 1fr) 140px");
		expect(getComputedStyle(buttons()).flexDirection).toBe("column");
		const button = within(compactDialog()).getByRole("button", { name: "Expand" });
		expect(getComputedStyle(button).minHeight).toBe("40px");
	});

	// jsdom does not resolve min()/calc() widths; the browser geometry run covers modal width.
	it("gives the modal a flush, column-flow standard frame", async () => {
		render(<ColorDialogLayout {...props({ expanded: true })} />);
		const dialog = await screen.findByRole("dialog", { name: "Color Special Dialog" });
		expect(getComputedStyle(dialog).padding).toBe("0px");
		expect(getComputedStyle(dialog).display).toBe("flex");
		expect(getComputedStyle(dialog).flexDirection).toBe("column");
		expect(getComputedStyle(within(dialog).getByTestId("editor-page")).overflow).toBe("auto");
		details(dialog);
		expect(getComputedStyle(within(dialog).getByTestId("editor-page")).overflow).toBe("auto");
	});
});
