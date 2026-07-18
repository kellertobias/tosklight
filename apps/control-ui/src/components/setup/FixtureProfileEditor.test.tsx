import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AttributeDescriptor, FixtureProfile, HeadColorSystem } from "../../api/types";
import { FixtureProfileEditor, applyCanonicalChannelAttribute, replaceFunctionBehavior, replaceHeadColorSystem } from "./FixtureProfileEditor";
import { blankChannel, blankFixtureProfile, blankFunction, blankHead } from "./fixtureProfileModel";

vi.mock("../files/RootConfinedFilePickerButton", () => ({
  RootConfinedFilePickerButton: ({ label }: { label: string }) => <span>{label}</span>,
}));

const elementFromPointDescriptor = Object.getOwnPropertyDescriptor(document, "elementFromPoint");

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  if (elementFromPointDescriptor) Object.defineProperty(document, "elementFromPoint", elementFromPointDescriptor);
  else Reflect.deleteProperty(document, "elementFromPoint");
});

function validProfile(revision = 0): FixtureProfile {
  const profile = blankFixtureProfile();
  profile.manufacturer = "Acme";
  profile.name = "Orbit";
  profile.revision = revision;
  return profile;
}

function choose(label: string, option: string) {
  const trigger = screen.getByText(label, { selector: "label", exact: true }).closest(".ui-form-field")!.querySelector(".ui-select-trigger") as HTMLButtonElement;
  fireEvent.click(trigger);
  fireEvent.click(screen.getByRole("option", { name: option }));
}

function openModeEditor(tab: "Heads" | "Channels" | "Color" | "Geometry" = "Channels") {
  fireEvent.click(screen.getByRole("button", { name: "Edit channels for Default" }));
  if (tab !== "Channels") fireEvent.click(screen.getByRole("tab", { name: tab }));
  return screen.getByRole("dialog", { name: "Edit Default mode" });
}

function touchDrag(source: HTMLElement, target: HTMLElement, pointerId: number, pointerType = "touch") {
  Object.defineProperties(source, {
    setPointerCapture: { configurable: true, value: vi.fn() },
    hasPointerCapture: { configurable: true, value: vi.fn(() => true) },
    releasePointerCapture: { configurable: true, value: vi.fn() },
  });
  Object.defineProperty(document, "elementFromPoint", { configurable: true, value: vi.fn(() => target) });
  fireEvent.pointerDown(source, { pointerId, pointerType });
  fireEvent.pointerMove(source, { pointerId, pointerType, clientX: 40, clientY: 80 });
  fireEvent.pointerUp(source, { pointerId, pointerType });
}

describe("FixtureProfileEditor", () => {
  it("authors the focused Generic physical metadata while preserving hidden legacy facts", async () => {
    const profile = validProfile();
    profile.physical.connectors = "powerCON TRUE1 TOP; 5-pin XLR in/out";
    profile.physical.light_source = "600 W LED engine";
    profile.physical.color_rendering_index = 92;
    profile.physical.lens = "Fresnel zoom";
    const save = vi.fn(async (draft: FixtureProfile) => draft);
    render(<FixtureProfileEditor initialProfile={profile} manufacturers={[]} onSave={save} onClose={vi.fn()}/>);

    expect(screen.queryByLabelText("Connectors")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Light source")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Color rendering index (CRI)")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Lens")).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Width (mm)"), { target: { value: "420" } });
    fireEvent.change(screen.getByLabelText("Height (mm)"), { target: { value: "680" } });
    fireEvent.change(screen.getByLabelText("Depth (mm)"), { target: { value: "310" } });
    fireEvent.change(screen.getByLabelText("Weight (kg)"), { target: { value: "24.5" } });
    fireEvent.change(screen.getByLabelText("Power consumption (W)"), { target: { value: "720" } });
    fireEvent.change(screen.getByLabelText("Color temperature (K)"), { target: { value: "6500" } });
    fireEvent.change(screen.getByLabelText("Luminous output (lm)"), { target: { value: "18500" } });
    fireEvent.change(screen.getByLabelText("Beam angle (degrees)"), { target: { value: "36" } });
    fireEvent.click(screen.getByRole("button", { name: "Save fixture" }));

    await waitFor(() => expect(save).toHaveBeenCalledOnce());
    expect(save.mock.calls[0][0].physical).toEqual({
      width_millimetres: 420,
      height_millimetres: 680,
      depth_millimetres: 310,
      weight_kilograms: 24.5,
      power_watts: 720,
      connectors: "powerCON TRUE1 TOP; 5-pin XLR in/out",
      light_source: "600 W LED engine",
      color_temperature_kelvin: 6500,
      color_rendering_index: 92,
      luminous_output_lumens: 18500,
      lens: "Fresnel zoom",
      beam_angle_degrees: 36,
    });
  });

  it("uses the server canonical attribute registry including IDs, families, value types, and default units", () => {
    const registry: AttributeDescriptor[] = [
      { id: "color", label: "Color", family: "color", value_type: "color", default_unit: null },
      { id: "zoom", label: "Zoom", family: "beam", value_type: "continuous", default_unit: "deg" },
      { id: "gobo.1", label: "Gobo 1", family: "beam", value_type: "indexed", default_unit: null },
    ];
    const profile = validProfile();
    const channel = blankChannel(profile.modes[0]);
    expect(applyCanonicalChannelAttribute(channel, "zoom", registry)).toMatchObject({
      attribute: "zoom",
      unit: "deg",
    });
    expect(applyCanonicalChannelAttribute(channel, "gobo.1", registry)).toMatchObject({
      attribute: "gobo.1",
      unit: null,
    });
    expect(applyCanonicalChannelAttribute(channel, "color.cyan", registry)).toMatchObject({
      attribute: "color.cyan",
      highlight_raw: 0,
    });
    expect(applyCanonicalChannelAttribute({ ...channel, highlight_raw: 73 }, "color.cyan", registry)).toMatchObject({
      attribute: "color.cyan",
      highlight_raw: 73,
    });

    const { container } = render(<FixtureProfileEditor initialProfile={profile} manufacturers={[]} attributeRegistry={registry} onSave={vi.fn()} onClose={vi.fn()}/>);
    const options = [...container.querySelectorAll<HTMLOptionElement>("#fixture-attribute-registry option")];
    expect(options.map((option) => option.value)).toEqual(["color", "zoom", "gobo.1"]);
    expect(options[1]).toHaveTextContent("beam · Zoom");
    expect(options[1]).toHaveAttribute("data-value-type", "continuous");
    expect(options[1]).toHaveAttribute("data-default-unit", "deg");
    expect(container.querySelector('option[value="beam.zoom"]')).not.toBeInTheDocument();
  });

  it("preserves a head's color correction matrix while changing its color system", () => {
    const correctionMatrix: HeadColorSystem["correction_matrix"] = [
      [1.1, 0.1, -0.1],
      [0.02, 0.97, 0.01],
      [0, 0.04, 0.96],
    ];
    const existing: HeadColorSystem[] = [{
      head_id: "head-1",
      correction_matrix: correctionMatrix,
      system: { type: "additive", emitters: [] },
    }];

    expect(replaceHeadColorSystem(existing, "head-1", {
      type: "discrete_wheel",
      channel_id: "color-wheel",
      slots: [],
    })).toEqual([{
      head_id: "head-1",
      correction_matrix: correctionMatrix,
      system: { type: "discrete_wheel", channel_id: "color-wheel", slots: [] },
    }]);
    expect(replaceHeadColorSystem([], "head-2", { type: "additive", emitters: [] })[0].correction_matrix).toEqual([
      [1, 0, 0],
      [0, 1, 0],
      [0, 0, 1],
    ]);
  });

  it("edits the per-head XYZ correction matrix without replacing its color system", async () => {
    const profile = validProfile();
    const head = profile.modes[0].heads[0];
    profile.modes[0].color_systems = [{
      head_id: head.id,
      correction_matrix: [[1, .1, 0], [0, 1, 0], [0, 0, 1]],
      system: { type: "additive", emitters: [] },
    }];
    const save = vi.fn(async (_draft: FixtureProfile, _expectedRevision: number) => profile);
    render(<FixtureProfileEditor initialProfile={profile} manufacturers={[]} onSave={save} onClose={vi.fn()}/>);
    fireEvent.click(screen.getByRole("tab", { name: "Modes" }));
    openModeEditor("Color");
    const field = screen.getByLabelText("Main correction row 1 column 2");
    expect(field).toHaveValue("0.1");
    fireEvent.change(field, { target: { value: "0.25" } });
    fireEvent.click(screen.getByRole("button", { name: "Save fixture" }));
    await waitFor(() => expect(save).toHaveBeenCalled());
    expect(save.mock.calls[0][0].modes[0].color_systems[0]).toMatchObject({
      correction_matrix: [[1, .25, 0], [0, 1, 0], [0, 0, 1]],
      system: { type: "additive" },
    });
  });

  it("applies the documented default priority when a function behavior changes", () => {
    const channel = blankChannel(validProfile().modes[0]);
    const fn = { ...blankFunction(channel), priority: 37 };

    expect(replaceFunctionBehavior(fn, "fixed", channel).priority).toBe(100);
    expect(replaceFunctionBehavior(fn, "indexed", channel).priority).toBe(100);
    expect(replaceFunctionBehavior(fn, "control", channel).priority).toBe(200);
    expect(replaceFunctionBehavior(fn, "continuous", channel).priority).toBe(0);
  });

  it("shows fixed, indexed, control, and continuous default priorities after behavior changes", () => {
    const profile = validProfile();
    profile.modes[0].channels = [blankChannel(profile.modes[0])];
    render(<FixtureProfileEditor initialProfile={profile} manufacturers={[]} onSave={vi.fn()} onClose={vi.fn()}/>);
    fireEvent.click(screen.getByRole("tab", { name: "Modes" }));
    openModeEditor();
    fireEvent.click(screen.getByRole("button", { name: "Edit intensity channel" }));
    fireEvent.click(screen.getByRole("button", { name: "Channel functions (0)" }));
    expect(screen.getByText("No functions are configured for this channel.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Add function" }));
    expect(screen.getByLabelText("Priority")).toHaveValue("0");

    choose("Function behavior", "Named fixed value");
    expect(screen.getByLabelText("Priority")).toHaveValue("100");
    fireEvent.change(screen.getByLabelText("Priority"), { target: { value: "137" } });
    expect(screen.getByLabelText("Priority")).toHaveValue("137");
    choose("Function behavior", "Indexed color or gobo");
    expect(screen.getByLabelText("Priority")).toHaveValue("100");
    choose("Function behavior", "Control action");
    expect(screen.getByLabelText("Priority")).toHaveValue("200");
    choose("Function behavior", "Continuous mapping");
    expect(screen.getByLabelText("Priority")).toHaveValue("0");
  });

  it("uses Generic and Modes title tabs with contextual title actions and no footer Cancel", () => {
    const { container } = render(<FixtureProfileEditor initialProfile={validProfile()} manufacturers={[]} onSave={vi.fn()} onClose={vi.fn()}/>);
    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Generic", "Modes"]);
    expect(screen.getByRole("button", { name: "Save fixture" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Add mode" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: "Modes" }));
    expect(container.querySelector(".ui-modal-title-actions")).toContainElement(screen.getByRole("button", { name: "Add mode" }));
    const modeRow = container.querySelector<HTMLElement>(".fixture-mode-list > article")!;
    expect(within(modeRow).getByLabelText(/^Mode name/)).toHaveValue("Default");
    expect(within(modeRow).getByLabelText("Mode notes")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Cancel" })).not.toBeInTheDocument();
  });

  it("closes unchanged immediately but offers Stay or Discard for a changed title-close and Escape", () => {
    const close = vi.fn();
    const { rerender } = render(<FixtureProfileEditor initialProfile={validProfile()} manufacturers={[]} onSave={vi.fn()} onClose={close}/>);
    fireEvent.click(screen.getByRole("button", { name: "Close fixture editor" }));
    expect(close).toHaveBeenCalledOnce();
    close.mockClear();
    rerender(<FixtureProfileEditor initialProfile={validProfile()} manufacturers={[]} onSave={vi.fn()} onClose={close}/>);
    fireEvent.change(screen.getByLabelText(/^Fixture name/), { target: { value: "Changed" } });
    fireEvent.click(screen.getByRole("button", { name: "Close fixture editor" }));
    expect(screen.getByRole("button", { name: "Stay" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Stay" }));
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.getByRole("button", { name: "Discard changes" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    expect(close).toHaveBeenCalledOnce();
  });

  it("guards a dirty backdrop close", () => {
    const close = vi.fn();
    const { container } = render(<FixtureProfileEditor initialProfile={validProfile()} manufacturers={[]} onSave={vi.fn()} onClose={close}/>);
    fireEvent.change(screen.getByLabelText(/^Fixture name/), { target: { value: "Changed" } });
    fireEvent.pointerDown(container.querySelector(".fixture-profile-editor-layer")!);
    expect(screen.getByRole("alertdialog", { name: "Discard fixture changes?" })).toBeInTheDocument();
    expect(close).not.toHaveBeenCalled();
  });

  it("shows the exact asynchronous server save error and keeps the editor open", async () => {
    const close = vi.fn();
    let rejectSave!: (reason: Error) => void;
    const save = vi.fn(() => new Promise<FixtureProfile>((_resolve, reject) => { rejectSave = reject; }));
    render(<FixtureProfileEditor initialProfile={validProfile()} manufacturers={[]} onSave={save} onClose={close}/>);
    fireEvent.click(screen.getByRole("button", { name: "Save fixture" }));
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ revision: 0 }), 0));
    expect(screen.getByRole("button", { name: "Loading" })).toBeDisabled();
    rejectSave(new Error("revision conflict: expected 0, current 2"));
    expect(screen.queryByRole("alertdialog", { name: "Create a new fixture revision?" })).not.toBeInTheDocument();
    await waitFor(() => expect(screen.getByText("revision conflict: expected 0, current 2")).toBeInTheDocument());
    expect(screen.getByRole("dialog", { name: "Create fixture profile" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save fixture" })).toBeEnabled();
    expect(close).not.toHaveBeenCalled();
  });

  it("asks before saving an edit as a new atomic revision", async () => {
    const save = vi.fn(async (profile: FixtureProfile) => ({ ...profile, revision: 4 }));
    render(<FixtureProfileEditor initialProfile={validProfile(3)} manufacturers={[]} onSave={save} onClose={vi.fn()}/>);
    fireEvent.click(screen.getByRole("button", { name: "Save fixture" }));
    expect(save).not.toHaveBeenCalled();
    expect(screen.getByRole("alertdialog", { name: "Create a new fixture revision?" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Save and create revision" }));
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ revision: 3 }), 3));
  });

  it("provides case-insensitive manufacturer lookup with the shared full-text keyboard", () => {
    render(<FixtureProfileEditor initialProfile={validProfile()} manufacturers={["Robe", "robe", "ETC", "Clay Paky"]} onSave={vi.fn()} onClose={vi.fn()}/>);
    fireEvent.click(screen.getByRole("button", { name: "Look up manufacturer" }));
		const lookup = screen.getByRole("dialog", { name: "Manufacturer lookup" });
		fireEvent.click(within(lookup).getByRole("button", { name: "Open keyboard" }));
    expect(screen.getByLabelText("Full text keyboard")).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Close input" }));
    expect(screen.getAllByRole("option", { name: /Robe/i })).toHaveLength(1);
    fireEvent.change(within(lookup).getByRole("textbox", { name: "Search manufacturers" }), { target: { value: "clay" } });
    fireEvent.click(screen.getByRole("option", { name: "Clay Paky" }));
    expect(screen.getByLabelText(/^Manufacturer/)).toHaveValue("Clay Paky");
    expect(screen.getByRole("dialog", { name: "Create fixture profile" })).toBeInTheDocument();
  });

  it("protects the final mode and keeps exactly one multi-split channel accordion open", () => {
    const profile = validProfile();
    const mode = profile.modes[0];
    mode.splits = [{ number: 1, footprint: 4 }, { number: 2, footprint: 8 }];
    mode.heads.push({ ...blankHead(1), master_shared: false });
    render(<FixtureProfileEditor initialProfile={profile} manufacturers={[]} onSave={vi.fn()} onClose={vi.fn()}/>);
    fireEvent.click(screen.getByRole("tab", { name: "Modes" }));
    expect(screen.getByRole("button", { name: "Remove Default" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Edit channels for Default" })).toBeInTheDocument();
    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Generic", "Modes"]);
    const modeEditor = openModeEditor();
    expect(within(modeEditor).getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Heads", "Channels", "Color", "Geometry"]);
    fireEvent.click(within(modeEditor).getByRole("button", { name: "Close mode editor" }));
    expect(screen.queryByRole("dialog", { name: "Edit Default mode" })).not.toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: "Create fixture profile" })).toBeInTheDocument();
    openModeEditor();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "Edit Default mode" })).not.toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: "Create fixture profile" })).toBeInTheDocument();
    openModeEditor();
    const accordionButtons = screen.getAllByRole("button", { name: /Split [12]/ });
    expect(accordionButtons.filter((button) => button.getAttribute("aria-expanded") === "true")).toHaveLength(1);
    fireEvent.click(accordionButtons[1]);
    expect(accordionButtons[0]).toHaveAttribute("aria-expanded", "false");
    expect(accordionButtons[1]).toHaveAttribute("aria-expanded", "true");
    expect(within(screen.getByText("No logical channels are assigned to split 2.").parentElement!).getByRole("button", { name: "Add channel" })).toBeInTheDocument();
  });

  it("assigns channels to splits independently while retaining one logical head", () => {
    const profile = validProfile();
    const headId = profile.modes[0].heads[0].id;
    profile.modes[0].splits = [{ number: 1, footprint: 1 }, { number: 2, footprint: 1 }];
    profile.modes[0].channels = [{ ...blankChannel(profile.modes[0], 1), head_id: headId }];
    const save = vi.fn(async (draft: FixtureProfile) => draft);
    render(<FixtureProfileEditor initialProfile={profile} manufacturers={[]} onSave={save} onClose={vi.fn()}/>);
    fireEvent.click(screen.getByRole("tab", { name: "Modes" }));
    openModeEditor();

    fireEvent.click(screen.getByRole("button", { name: "Edit intensity channel" }));
    choose("Address split", "Split 2");
    fireEvent.click(screen.getByRole("button", { name: "Close channel editor" }));

    expect(screen.getByRole("button", { name: /Split 2/ })).toHaveAttribute("aria-expanded", "true");
    expect(document.querySelectorAll(".fixture-channel-row")).toHaveLength(1);
    expect(screen.queryAllByLabelText("Head name")).toHaveLength(0);
    fireEvent.click(screen.getByRole("button", { name: "Save fixture" }));
    expect(save.mock.calls[0][0].modes[0].heads).toHaveLength(1);
    expect(save.mock.calls[0][0].modes[0].channels[0]).toMatchObject({ head_id: headId, split: 2 });
  });

  it("reorders modes through the drag handle for touch and mouse without move buttons", () => {
    render(<FixtureProfileEditor initialProfile={validProfile()} manufacturers={[]} onSave={vi.fn()} onClose={vi.fn()}/>);
    fireEvent.click(screen.getByRole("tab", { name: "Modes" }));
    fireEvent.click(screen.getByRole("button", { name: "Add mode" }));
    const articles = document.querySelectorAll<HTMLElement>(".fixture-mode-list > article");
    const source = articles[0].querySelector<HTMLElement>(".touch-drag-handle")!;
    const target = articles[1];
    Object.defineProperties(source, {
      setPointerCapture: { configurable: true, value: vi.fn() },
      hasPointerCapture: { configurable: true, value: vi.fn(() => true) },
      releasePointerCapture: { configurable: true, value: vi.fn() },
    });
    Object.defineProperty(document, "elementFromPoint", { configurable: true, value: vi.fn(() => target) });

    fireEvent.pointerDown(source, { pointerId: 4, pointerType: "touch" });
    fireEvent.pointerMove(source, { pointerId: 4, pointerType: "touch", clientX: 40, clientY: 80 });

    const names = [...document.querySelectorAll<HTMLElement>(".fixture-mode-list > article")].map((row) => row.querySelector<HTMLInputElement>("input")?.value);
    expect(names[0]).toMatch(/^Mode 2/);
    expect(names[1]).toMatch(/^Default/);
    expect(screen.queryByRole("button", { name: /Move Mode 2 (?:up|down)/ })).not.toBeInTheDocument();

    const reordered = document.querySelectorAll<HTMLElement>(".fixture-mode-list > article");
    touchDrag(reordered[0].querySelector<HTMLElement>(".touch-drag-handle")!, reordered[1], 8, "mouse");
    expect([...document.querySelectorAll<HTMLElement>(".fixture-mode-list > article")].map((row) => row.querySelector<HTMLInputElement>("input")?.value)).toEqual(["Default", "Mode 2"]);
  });

  it("uses the two-row mode layout with a trash action and multiline notes keyboard", () => {
    render(<FixtureProfileEditor initialProfile={validProfile()} manufacturers={[]} onSave={vi.fn()} onClose={vi.fn()}/>);
    fireEvent.click(screen.getByRole("tab", { name: "Modes" }));
    const mode = document.querySelector<HTMLElement>(".fixture-mode-list > article")!;
    expect(within(mode).getByRole("button", { name: "Edit channels for Default" })).toHaveTextContent("Edit");
    expect(within(mode).getByRole("button", { name: "Remove Default" }).querySelector("svg")).toBeInTheDocument();
    expect(within(mode).getByLabelText("Mode notes").tagName).toBe("TEXTAREA");
    fireEvent.click(mode.querySelector<HTMLButtonElement>(".ui-large-text-keyboard")!);
    expect(screen.getByRole("dialog", { name: "Mode notes" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Enter · New line" }));
    fireEvent.click(screen.getByRole("button", { name: "Done · Confirm" }));
    expect(within(mode).getByLabelText("Mode notes")).toHaveValue("\n");
  });

  it("adds, reorders, and removes heads and channels through the editor controls", () => {
    const registry: AttributeDescriptor[] = [{ id: "intensity", label: "Intensity", family: "intensity", value_type: "continuous", default_unit: "percent" }, { id: "pan", label: "Pan", family: "position", value_type: "continuous", default_unit: "degrees" }];
    const { container } = render(<FixtureProfileEditor initialProfile={validProfile()} manufacturers={[]} attributeRegistry={registry} onSave={vi.fn()} onClose={vi.fn()}/>);
    fireEvent.click(screen.getByRole("tab", { name: "Modes" }));
    openModeEditor("Heads");

    fireEvent.click(screen.getByRole("button", { name: "Add head" }));
    const headNames = screen.getAllByLabelText("Head name");
    expect(headNames).toHaveLength(2);
    fireEvent.change(headNames[1], { target: { value: "Spot" } });
    let headRows = document.querySelectorAll<HTMLElement>(".fixture-head-list > article");
    touchDrag(headRows[0].querySelector<HTMLElement>(".touch-drag-handle")!, headRows[1], 5);
    expect(screen.getAllByLabelText("Head name").map((input) => (input as HTMLInputElement).value)).toEqual(["Spot", "Main"]);
    fireEvent.click(screen.getByRole("button", { name: "Move Spot down" }));
    fireEvent.click(screen.getByRole("button", { name: "Move Spot up" }));
    expect(screen.getAllByLabelText("Head name").map((input) => (input as HTMLInputElement).value)).toEqual(["Spot", "Main"]);
    fireEvent.click(screen.getByRole("button", { name: "Remove Main" }));
    expect(screen.getAllByLabelText("Head name")).toHaveLength(1);
    expect(screen.getByLabelText("Head name")).toHaveValue("Spot");

    fireEvent.click(screen.getByRole("tab", { name: "Channels" }));
    fireEvent.click(screen.getByRole("button", { name: "Add channel" }));
    fireEvent.click(screen.getByRole("button", { name: "Add channel" }));
    let channelRows = document.querySelectorAll<HTMLElement>(".fixture-channel-row");
    fireEvent.click(within(channelRows[1]).getByRole("button", { name: "Edit intensity channel" }));
    choose("Channel role", "position · Pan");
    fireEvent.click(screen.getByRole("button", { name: "Close channel editor" }));
    channelRows = document.querySelectorAll<HTMLElement>(".fixture-channel-row");
    touchDrag(channelRows[0].querySelector<HTMLElement>(".touch-drag-handle")!, channelRows[1], 6);
    expect([...container.querySelectorAll<HTMLElement>(".fixture-channel-row")].map((row) => row.textContent)).toEqual([expect.stringContaining("Pan"), expect.stringContaining("Intensity")]);
    fireEvent.click(screen.getByRole("button", { name: "Move pan down" }));
    fireEvent.click(screen.getByRole("button", { name: "Move pan up" }));
    expect([...container.querySelectorAll<HTMLElement>(".fixture-channel-row")].map((row) => row.textContent)).toEqual([expect.stringContaining("Pan"), expect.stringContaining("Intensity")]);
    fireEvent.click(screen.getByRole("button", { name: "Remove intensity" }));
    expect(container.querySelectorAll(".fixture-channel-row")).toHaveLength(1);
    expect(container.querySelector(".fixture-channel-row")).toHaveTextContent("Pan");
  });

  it("shows a single split's channel table directly without an accordion", () => {
    render(<FixtureProfileEditor initialProfile={validProfile()} manufacturers={[]} onSave={vi.fn()} onClose={vi.fn()}/>);
    fireEvent.click(screen.getByRole("tab", { name: "Modes" }));
    openModeEditor();
    expect(screen.getByRole("table")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Split 1/ })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add channel" })).toBeInTheDocument();
  });

  it("edits optional wheel calibration and source-layout parameters", () => {
    const profile = validProfile();
    profile.modes[0].channels.push({ ...blankChannel(profile.modes[0]), attribute: "color.wheel.1" });
    render(<FixtureProfileEditor initialProfile={profile} manufacturers={[]} onSave={vi.fn()} onClose={vi.fn()}/>);
    fireEvent.click(screen.getByRole("tab", { name: "Modes" }));
    openModeEditor("Color");
    choose("Color system", "Discrete color wheel");
    fireEvent.click(screen.getByRole("button", { name: "Add color slot" }));
    fireEvent.click(screen.getByLabelText("Measured XYZ available"));
    expect(screen.getAllByLabelText(/Measured XYZ [XYZ]/)).toHaveLength(3);

    fireEvent.click(screen.getByRole("tab", { name: "Geometry" }));
    fireEvent.click(screen.getByRole("treeitem", { name: /Beam/ }));
    choose("Source layout", "Matrix");
    expect(screen.getByLabelText("Matrix columns")).toHaveValue("4");
    expect(screen.getByLabelText("Matrix rows")).toHaveValue("4");
    expect(screen.getByText("Matrix spacing (mm)")).toBeInTheDocument();
  });

  it("applies every geometry template through the nested mode editor", () => {
    const profile = validProfile();
    profile.modes[0].heads.push({ ...blankHead(1), master_shared: false });
    render(<FixtureProfileEditor initialProfile={profile} manufacturers={[]} onSave={vi.fn()} onClose={vi.fn()}/>);
    fireEvent.click(screen.getByRole("tab", { name: "Modes" }));
    openModeEditor("Geometry");

    const sourceLayout = () => screen.getByText("Source layout", { selector: "label", exact: true }).closest(".ui-form-field")!.querySelector(".ui-select-trigger")!;
    fireEvent.click(screen.getByRole("button", { name: "Fixed fixture" }));
    expect(screen.getByText("1 parts · 2 emitters. Preview uses the Stage renderer's hierarchy, transforms, source layouts, and beam angles.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("treeitem", { name: /Beam 1/ }));
    expect(sourceLayout()).toHaveTextContent("Point");
    expect(screen.getByLabelText("Projects a directional beam")).toBeChecked();

    fireEvent.click(screen.getByRole("button", { name: "Moving head" }));
    expect(screen.getByText("4 parts · 2 emitters. Preview uses the Stage renderer's hierarchy, transforms, source layouts, and beam angles.")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Bar" }));
    fireEvent.click(screen.getByRole("treeitem", { name: /Beam 1/ }));
    expect(sourceLayout()).toHaveTextContent("Strip");
    expect(screen.getByLabelText("Projects a directional beam")).not.toBeChecked();

    fireEvent.click(screen.getByRole("button", { name: "Matrix" }));
    fireEvent.click(screen.getByRole("treeitem", { name: /Beam 1/ }));
    expect(sourceLayout()).toHaveTextContent("Matrix");
    expect(screen.getByLabelText("Projects a directional beam")).not.toBeChecked();

    fireEvent.click(screen.getByRole("button", { name: "Shared-pan multi-head" }));
    expect(screen.getByText("4 parts · 2 emitters. Preview uses the Stage renderer's hierarchy, transforms, source layouts, and beam angles.")).toBeInTheDocument();
  });
});
