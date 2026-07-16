import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { FileManagerPickerHost, openFileManagerPicker } from "./FileManagerPickerHost";

vi.mock("./FileManagerWindow", () => ({
  FileManager: ({ picker }: { picker: { target?: string; multiple?: boolean; allowedExtensions?: string[]; initialRootId?: string; initialDirectory?: string; onSelect: (selection: unknown[]) => void; onCancel: () => void } }) => <section aria-label="Mock picker">
    <output>{JSON.stringify({ target: picker.target, multiple: picker.multiple, allowedExtensions: picker.allowedExtensions, initialRootId: picker.initialRootId, initialDirectory: picker.initialDirectory })}</output>
    <button onClick={() => picker.onSelect([{ rootId: "shows", entry: { path: "notes.txt" } }])}>Select mock</button>
    <button onClick={picker.onCancel}>Cancel mock</button>
  </section>,
}));

afterEach(cleanup);

describe("FileManagerPickerHost", () => {
  it("hosts the reusable picker configuration and resolves only after explicit selection", async () => {
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
    expect(screen.getByText(/"target":"files"/)).toHaveTextContent('"multiple":true');
    fireEvent.click(screen.getByRole("button", { name: "Select mock" }));
    await expect(result).resolves.toEqual([{ rootId: "shows", entry: { path: "notes.txt" } }]);
    expect(screen.queryByRole("dialog", { name: "Choose files or folders" })).not.toBeInTheDocument();
  });

  it("resolves cancellation as null", async () => {
    render(<FileManagerPickerHost />);
    let result!: Promise<unknown>;
    act(() => { result = openFileManagerPicker({ target: "folders" }); });
    fireEvent.click(screen.getByRole("button", { name: "Cancel mock" }));
    await expect(result).resolves.toBeNull();
  });
});
