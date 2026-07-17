import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Button } from "../components/common/controls";
import { FileManagerPickerHost, openFileManagerPicker } from "./FileManagerPickerHost";

const mocks = vi.hoisted(() => ({ configuration: { file_manager_system_picker_fallback: false } }));

vi.mock("../api/ServerContext", () => ({ useServer: () => ({ configuration: mocks.configuration }) }));

vi.mock("./FileManagerWindow", () => ({
  extension: (name: string) => name.includes(".") ? name.split(".").pop()?.toLowerCase() ?? "" : "",
  FileManager: ({ picker }: { picker: { target?: string; multiple?: boolean; allowedExtensions?: string[]; initialRootId?: string; initialDirectory?: string; onSelect: (selection: unknown[]) => void; onCancel: () => void } }) => <section aria-label="Mock picker">
    <output>{JSON.stringify({ target: picker.target, multiple: picker.multiple, allowedExtensions: picker.allowedExtensions, initialRootId: picker.initialRootId, initialDirectory: picker.initialDirectory })}</output>
    <Button onClick={() => picker.onSelect([{ rootId: "shows", entry: { path: "notes.txt" } }])}>Select mock</Button>
    <Button onClick={picker.onCancel}>Cancel mock</Button>
  </section>,
}));

afterEach(cleanup);

describe("FileManagerPickerHost", () => {
  it("hosts the reusable picker configuration and resolves only after explicit selection", async () => {
    mocks.configuration.file_manager_system_picker_fallback = false;
    render(<FileManagerPickerHost />);
    let result!: Promise<unknown>;
    act(() => {
      result = openFileManagerPicker({
        target: "files",
        multiple: true,
        allowedExtensions: ["txt", "md"],
        initialRootId: "shows",
        initialDirectory: "run",
      });
    });

    expect(screen.getByRole("dialog", { name: "Choose files or folders" })).toBeVisible();
    expect(screen.queryByRole("button", { name: "Open system file picker" })).not.toBeInTheDocument();
    expect(screen.getByText(/"target":"files"/)).toHaveTextContent('"multiple":true');
    fireEvent.click(screen.getByRole("button", { name: "Select mock" }));
    await expect(result).resolves.toEqual([{ rootId: "shows", entry: { path: "notes.txt" } }]);
    expect(screen.queryByRole("dialog", { name: "Choose files or folders" })).not.toBeInTheDocument();
  });

  it("resolves cancellation as null", async () => {
    mocks.configuration.file_manager_system_picker_fallback = false;
    render(<FileManagerPickerHost />);
    let result!: Promise<unknown>;
    act(() => { result = openFileManagerPicker({ target: "folders" }); });
    fireEvent.click(screen.getByRole("button", { name: "Cancel mock" }));
    await expect(result).resolves.toBeNull();
  });

  it("keeps the system picker constrained when the disabled-by-default fallback is enabled", async () => {
    mocks.configuration.file_manager_system_picker_fallback = true;
    const view = render(<FileManagerPickerHost />);
    let result!: Promise<unknown>;
    act(() => { result = openFileManagerPicker({ target: "files", multiple: true, allowedExtensions: [".gdtf"] }); });

    expect(screen.getByRole("button", { name: "Open system file picker" })).toBeVisible();
    const input = view.container.querySelector<HTMLInputElement>('input[type="file"]')!;
    expect(input).toHaveAttribute("accept", ".gdtf");
    expect(input).toHaveAttribute("multiple");
    fireEvent.change(input, { target: { files: [new File(["fixture"], "tour.gdtf", { type: "application/zip" })] } });
    await expect(result).resolves.toEqual(expect.objectContaining({
      source: "system",
      target: "files",
      files: [expect.objectContaining({ name: "tour.gdtf" })],
    }));
  });

  it("rejects a system-picked file outside the calling form's extension filter", async () => {
    mocks.configuration.file_manager_system_picker_fallback = true;
    const view = render(<FileManagerPickerHost />);
    let result!: Promise<unknown>;
    act(() => { result = openFileManagerPicker({ target: "files", allowedExtensions: ["gdtf"] }); });

    const input = view.container.querySelector<HTMLInputElement>('input[type="file"]')!;
    fireEvent.change(input, { target: { files: [new File(["image"], "wrong.png")] } });
    expect(screen.getByRole("alert")).toHaveTextContent("Choose only .gdtf files");
    fireEvent.click(screen.getByRole("button", { name: "Cancel mock" }));
    await expect(result).resolves.toBeNull();
  });

  it("configures the system fallback as a directory chooser for folder targets", () => {
    mocks.configuration.file_manager_system_picker_fallback = true;
    const view = render(<FileManagerPickerHost />);
    act(() => { void openFileManagerPicker({ target: "folders", multiple: false }); });

    const input = view.container.querySelector<HTMLInputElement>('input[type="file"]')!;
    expect(input).toHaveAttribute("webkitdirectory");
    expect(input).toHaveAttribute("multiple");
    expect(input).not.toHaveAttribute("accept");
  });
});
