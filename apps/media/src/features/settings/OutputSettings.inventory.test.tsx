import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../../app/ToastContext";
import { anOutputConfiguration, stubServer } from "../../testing/server";
import { OutputSettings } from "./OutputSettings";

afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); });

function setup(state: "loading" | "stalled" | "failed", mode: "sound" | "dmx") {
 const output = anOutputConfiguration("inventory-output", "Main");
 output.soundOutputKind = "device";
 output.soundOutputName = "Saved desk output";
 output.soundOutputInventory = { state, hasSuccessfulSnapshot: false, error: state === "failed" ? "OS enumeration failed" : null };
 const server = stubServer({ outputConfigurations: { [output.id]: output } });
 const baseFetch = globalThis.fetch;
 vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
  if (String(input).endsWith(`/outputs/${output.id}/configuration/update`)) {
   const edit = JSON.parse(String(init?.body ?? "{}"));
   Object.assign(output, edit);
   return new Response(JSON.stringify(output), { headers: { "content-type": "application/json" } });
  }
  return baseFetch(input, init);
 }));
 render(<ToastProvider><OutputSettings outputId={output.id} outputName="Main" mode={mode} direct /></ToastProvider>);
 return server;
}

describe("optional output-device inventory", () => {
 it.each(["loading", "stalled", "failed"] as const)("keeps the DMX editor writable while discovery is %s", async (state) => {
  const server = setup(state, "dmx");
  const universe = await screen.findByLabelText("Universe");
  await userEvent.clear(universe);
  await userEvent.type(universe, "51");
  await userEvent.tab();
  await waitFor(() => expect(server.outputConfigurations["inventory-output"].universe).toBe(51));
  expect(server.outputConfigurations["inventory-output"].soundOutputName).toBe("Saved desk output");
  expect(screen.queryByText(/Reading Main output settings/)).not.toBeInTheDocument();
 });
 it.each(["loading", "stalled", "failed"] as const)("retains a selected device without claiming it unavailable while discovery is %s", async (state) => {
  setup(state, "sound");
  expect(await screen.findByText("Saved desk output · saved selection")).toBeInTheDocument();
  expect(screen.queryByText(/Saved desk output · unavailable/)).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Refresh audio devices" })).toBeEnabled();
  const notice = state === "loading" ? /Discovering audio output/ : state === "stalled" ? /taking longer/ : /could not be discovered/;
  expect(screen.getByText(notice)).toBeVisible();
 });
});
