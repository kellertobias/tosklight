import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FixtureProfile } from "../../api/types";
import { FixtureLibrarySetup } from "./FixtureLibrarySetup";
import { blankFixtureProfile } from "./fixtureProfileModel";

const server = vi.hoisted(() => ({
  fixtureProfiles: [] as unknown[],
  fixtureLibrary: [] as unknown[],
  fixtureProfileWarnings: [] as string[],
  bootstrap: { attribute_registry: [] },
  error: null as string | null,
  fixtureProfileRevisions: vi.fn(),
  saveFixtureProfile: vi.fn(),
  saveFixtureProfileSourceGdtf: vi.fn(),
  deleteFixtureProfile: vi.fn(),
}));

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../files/RootConfinedFilePickerButton", () => ({
  RootConfinedFilePickerButton: ({ label }: { label: string }) => <span>{label}</span>,
}));

beforeEach(() => {
  const toolbar = document.createElement("div");
  toolbar.id = "setup-section-actions";
  document.body.append(toolbar);
  server.fixtureProfiles = [];
  server.fixtureLibrary = [];
  server.fixtureProfileWarnings = [];
  server.fixtureProfileRevisions.mockReset();
  server.saveFixtureProfile.mockReset();
  server.deleteFixtureProfile.mockReset();
});

afterEach(() => {
  cleanup();
  document.getElementById("setup-section-actions")?.remove();
});

describe("fixture profile revision editing", () => {
  it("filters the fixture library immediately while typing and restores it with Clear search", async () => {
    const dimmer = blankFixtureProfile();
    dimmer.manufacturer = "Generic";
    dimmer.name = "Dimmer";
    dimmer.short_name = "Dimmer";
    const orbit = blankFixtureProfile();
    orbit.id = "orbit-profile";
    orbit.manufacturer = "Acme";
    orbit.name = "Orbit Wash";
    orbit.short_name = "Orbit";
    server.fixtureProfiles = [dimmer, orbit];

    render(<FixtureLibrarySetup/>);
    const search = await screen.findByRole("textbox", { name: "Search" });
    fireEvent.change(search, { target: { value: "orbit" } });

    expect(screen.getByRole("button", { name: /Orbit Wash/ })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Dimmer/ })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
    expect(screen.getByRole("button", { name: /^Dimmer/ })).toBeInTheDocument();
  });

  it("uses a retained revision as the draft but the latest revision as the optimistic save check", async () => {
    const latest = blankFixtureProfile();
    latest.manufacturer = "Acme";
    latest.name = "Orbit";
    latest.short_name = "Orbit";
    latest.revision = 5;
    latest.notes = "Latest revision";
    const retained = structuredClone(latest);
    retained.revision = 2;
    retained.notes = "Retained revision draft";
    server.fixtureProfiles = [latest];
    server.fixtureProfileRevisions.mockResolvedValue([retained, latest]);
    server.saveFixtureProfile.mockImplementation(async (profile: FixtureProfile, expectedRevision: number) => ({
      ...profile,
      revision: expectedRevision + 1,
    }));

    render(<FixtureLibrarySetup/>);
    fireEvent.click(screen.getByRole("button", { name: "Revision history" }));
    await waitFor(() => expect(server.fixtureProfileRevisions).toHaveBeenCalledWith(latest.id));
    const retainedArticle = screen.getByText("Revision 2").closest("article")!;
    fireEvent.click(within(retainedArticle).getByRole("button", { name: "Edit as new revision" }));
    fireEvent.click(screen.getByRole("button", { name: "Save fixture" }));
    fireEvent.click(screen.getByRole("button", { name: "Save and create revision" }));

    await waitFor(() => expect(server.saveFixtureProfile).toHaveBeenCalledOnce());
    expect(server.saveFixtureProfile.mock.calls[0][0]).toMatchObject({
      id: latest.id,
      revision: 2,
      notes: "Retained revision draft",
    });
    expect(server.saveFixtureProfile.mock.calls[0][1]).toBe(5);
  });
});
