import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { OutputRoute, VersionedObject } from "../../api/types";
import { OutputRoutesSetup } from "./OutputRoutesSetup";

afterEach(cleanup);

const route: VersionedObject<OutputRoute> = {
  kind: "route",
  id: "front-artnet",
  revision: 4,
  updated_at: "2026-07-16T12:00:00Z",
  body: {
    protocol: "art_net",
    logical_universe: 1,
    destination_universe: 11,
    destination: "10.0.0.20:6454",
    enabled: true,
    minimum_slots: 128,
  },
};

describe("OutputRoutesSetup", () => {
  it("edits an existing versioned route without writing before Save", async () => {
    const save = vi.fn().mockResolvedValue(true);
    render(<OutputRoutesSetup routes={[route]} onSave={save} onDelete={vi.fn().mockResolvedValue(true)}/>);

    expect(screen.getByText("Logical 1 → Art-Net 11")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Edit route" }));
    fireEvent.change(screen.getByLabelText("Logical universe"), { target: { value: "2" } });
    fireEvent.change(screen.getByLabelText("Minimum universe size"), { target: { value: "256" } });
    expect(save).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Save route" }));

    await waitFor(() => expect(save).toHaveBeenCalledWith("front-artnet", {
      ...route.body,
      logical_universe: 2,
      minimum_slots: 256,
    }, 4));
    expect(screen.queryByRole("dialog", { name: "Output route editor" })).not.toBeInTheDocument();
  });

  it("validates an Art-Net destination before creating a route", async () => {
    const save = vi.fn().mockResolvedValue(true);
    render(<OutputRoutesSetup routes={[]} onSave={save} onDelete={vi.fn().mockResolvedValue(true)}/>);

    fireEvent.click(screen.getByRole("button", { name: "Add route" }));
    fireEvent.click(screen.getByRole("button", { name: "Save route" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Art-Net routes require a destination");
    expect(save).not.toHaveBeenCalled();

    fireEvent.change(screen.getByLabelText("Destination"), { target: { value: "127.0.0.1:6454" } });
    fireEvent.click(screen.getByRole("button", { name: "Save route" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0][0]).toMatch(/^route-/);
    expect(save.mock.calls[0][1]).toMatchObject({ destination: "127.0.0.1:6454", enabled: true, minimum_slots: 128 });
    expect(save.mock.calls[0][2]).toBe(0);
  });

  it("requires an explicit confirmation before removing a route", async () => {
    const remove = vi.fn().mockResolvedValue(true);
    render(<OutputRoutesSetup routes={[route]} onSave={vi.fn().mockResolvedValue(true)} onDelete={remove}/>);

    fireEvent.click(screen.getByRole("button", { name: "Edit route" }));
    fireEvent.click(screen.getByRole("button", { name: "Remove route" }));
    expect(remove).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Confirm remove" }));
    await waitFor(() => expect(remove).toHaveBeenCalledWith("front-artnet", 4));
  });
});
