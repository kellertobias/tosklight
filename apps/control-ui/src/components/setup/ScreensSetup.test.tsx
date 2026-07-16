import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ScreenConfiguration } from "../../api/types";
import { ScreenSettingsCard } from "./ScreensSetup";

const configuredScreen: ScreenConfiguration = {
  id: "screen-1",
  name: "Screen 1",
  layout: { desks: [], activeDeskId: "main" },
  show_dock: true,
  show_playbacks: true,
  playback_count: 8,
  playback_rows: 1,
  first_playback_slot: 1,
  page_mode: "follow_main",
  show_page_controls: true,
  desired_open: true,
  display_id: null,
  bounds: null,
  fullscreen: false,
};

describe("additional screen settings", () => {
  it("updates fields immediately and serializes the saved configurations", async () => {
    const saved: ScreenConfiguration[] = [];
    const save = vi.fn(async (value: ScreenConfiguration) => { saved.push(value); });
    render(<ScreenSettingsCard screen={configuredScreen} displays={[]} save={save} remove={vi.fn()}/>);

    expect(screen.getByRole("heading", { name: "Layout" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Placement" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Playbacks" })).toBeInTheDocument();
    expect(screen.getByLabelText("First Playback Number")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Follow Main" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove Screen" })).toBeInTheDocument();
    const name = screen.getByLabelText("Screen name");
    fireEvent.change(name, { target: { value: "Stage manager" } });
    expect(name).toHaveValue("Stage manager");
    fireEvent.click(screen.getByRole("button", { name: "Close Screen" }));

    await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
    expect(saved[0].name).toBe("Stage manager");
    expect(saved[1]).toMatchObject({ name: "Stage manager", desired_open: false });
  });
});
