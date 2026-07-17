import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  FileManagerRootsSetup,
  fileManagerRootsValidationError,
  looksLikeAbsoluteServerPath,
  type FileManagerRootConfiguration,
} from "./FileManagerRootsSetup";

afterEach(cleanup);

const root: FileManagerRootConfiguration = {
  id: "tour-notes",
  label: "Tour Notes",
  path: "/srv/tour/notes",
  icon: "network",
};

describe("FileManagerRootsSetup", () => {
  it("shows the backward-compatible Shows root until a custom root is added", () => {
    const onChange = vi.fn();
    render(<FileManagerRootsSetup roots={[]} systemPickerFallback={false} onChange={onChange} onSystemPickerFallbackChange={vi.fn()} onOpen={vi.fn()} />);

    expect(screen.getByText("Built-in default · Desk Shows directory · ID: shows")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Add configured root" }));
    expect(onChange).toHaveBeenCalledWith([{
      id: "location-1",
      label: "New location",
      path: "",
      icon: "folder",
    }]);
  });

  it("edits labels, absolute paths, icons, and removes roots without changing their stable IDs", () => {
    const onChange = vi.fn();
    const onSystemPickerFallbackChange = vi.fn();
    const onOpen = vi.fn();
    render(<FileManagerRootsSetup roots={[root]} systemPickerFallback={false} onChange={onChange} onSystemPickerFallbackChange={onSystemPickerFallbackChange} onOpen={onOpen} />);
    const card = screen.getByRole("article", { name: "Configured root 1" });

    fireEvent.change(within(card).getByLabelText(/^Label/), { target: { value: "Updated Notes" } });
    expect(onChange).toHaveBeenLastCalledWith([{ ...root, label: "Updated Notes" }]);
    fireEvent.change(within(card).getByLabelText(/^Absolute server path/), { target: { value: "D:\\Show Files" } });
    expect(onChange).toHaveBeenLastCalledWith([{ ...root, path: "D:\\Show Files" }]);
    fireEvent.click(within(card).getByRole("button", { name: /Network$/ }));
    fireEvent.click(screen.getByRole("option", { name: "Archive" }));
    expect(onChange).toHaveBeenLastCalledWith([{ ...root, icon: "archive" }]);

    fireEvent.click(within(card).getByRole("button", { name: "Remove" }));
    expect(onChange).toHaveBeenLastCalledWith([]);
    fireEvent.click(screen.getByRole("button", { name: "Open File Manager Workspace" }));
    expect(onOpen).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("switch", { name: "Allow Open system file picker fallback" }));
    expect(onSystemPickerFallbackChange).toHaveBeenCalledWith(true);
  });

  it("validates server paths and unique stable IDs before Setup can save", () => {
    expect(looksLikeAbsoluteServerPath("/srv/shows")).toBe(true);
    expect(looksLikeAbsoluteServerPath("C:\\Shows")).toBe(true);
    expect(looksLikeAbsoluteServerPath("\\\\server\\share")).toBe(true);
    expect(looksLikeAbsoluteServerPath("relative/shows")).toBe(false);
    expect(fileManagerRootsValidationError([root])).toBeNull();
    expect(fileManagerRootsValidationError([{ ...root, path: "relative" }])).toContain("absolute path");
    expect(fileManagerRootsValidationError([root, { ...root, label: "Duplicate" }])).toContain("duplicated");
  });
});
