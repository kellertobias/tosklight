import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MatterBridgeStatus } from "../../api/types";
import { MatterBridgeSettings } from "../setup/MatterBridgeSettings";

vi.mock("../../features/configuration/ConfigurationState", async (importOriginal) => ({
	...(await importOriginal<Record<string, unknown>>()),
	useDeskConfiguration: () => mocks.server.configuration,
	useMatterEnabled: () => mocks.server.configuration.matter_enabled,
}));

const mocks = vi.hoisted(() => ({
  clipboardWriteText: vi.fn(),
  saveConfiguration: vi.fn(),
  server: {} as {
    configuration: Record<string, unknown> & { matter_enabled: boolean };
    matter: MatterBridgeStatus | null;
    saveConfiguration: ReturnType<typeof vi.fn>;
  },
}));

vi.mock("../../api/ServerContext", () => ({ useServer: () => mocks.server }));

function matterStatus(overrides: Partial<MatterBridgeStatus> = {}): MatterBridgeStatus {
  return {
    enabled: true,
    transport: "running",
    commissionable: false,
    network_running: true,
    commissioned: false,
    commissioning_window_open: false,
    revision: 1,
    lights: [],
    ...overrides,
  };
}

beforeEach(() => {
  mocks.saveConfiguration.mockReset();
  mocks.clipboardWriteText.mockReset();
  mocks.clipboardWriteText.mockResolvedValue(undefined);
  mocks.server.configuration = { matter_enabled: false, retained_setting: "unchanged" };
  mocks.server.matter = null;
  mocks.server.saveConfiguration = mocks.saveConfiguration;
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: mocks.clipboardWriteText },
  });
});

afterEach(cleanup);

describe("physical desk Matter playback bridge settings", () => {
  it("shows disabled state and persists the toggle without discarding other desk settings", () => {
    render(<MatterBridgeSettings />);

    const toggle = screen.getByRole("switch", {
      name: "Matter server disabled",
    });
    expect(toggle).not.toBeChecked();
    expect(screen.getByText("Disabled. No Matter lights are advertised.")).toBeInTheDocument();

    fireEvent.click(toggle);
    expect(mocks.saveConfiguration).toHaveBeenCalledWith({
      matter_enabled: true,
      retained_setting: "unchanged",
    });
  });

  it("reports a running bridge with zero lights when every playback slot is empty", () => {
    mocks.server.configuration.matter_enabled = true;
    mocks.server.matter = matterStatus();

    render(<MatterBridgeSettings />);

    expect(screen.getByRole("switch", { name: "Matter server enabled" })).toBeChecked();
    expect(screen.getByText("0 assigned playbacks exposed as dimmable lights.")).toBeInTheDocument();
    expect(screen.getByText(/including button-only controls; empty slots are not advertised/i)).toBeInTheDocument();
    expect(screen.queryByText("Ready to commission")).not.toBeInTheDocument();
  });

  it("shows pairing data and counts a button-only assignment as a dimmable light", () => {
    mocks.server.configuration.matter_enabled = true;
    mocks.server.matter = matterStatus({
      commissionable: true,
      commissioning_window_open: true,
      pairing: {
        qr_code: "MT:TEST-PAIRING-PAYLOAD",
        manual_code: "3497-0112-233",
        discriminator: 1234,
      },
      lights: [
        {
          endpoint_id: 128,
          page: 2,
          playback: 1,
          playback_number: 42,
          name: "Page 2 Playback 1: Button only house",
          on: true,
          level: 127,
        },
      ],
    });

    render(<MatterBridgeSettings />);

    expect(screen.getByText("1 assigned playback exposed as dimmable lights.")).toBeInTheDocument();
    expect(screen.getByText("Ready to commission")).toBeInTheDocument();
    expect(screen.getByText("3497-0112-233")).toBeInTheDocument();
    expect(screen.getByText("MT:TEST-PAIRING-PAYLOAD")).toBeInTheDocument();
    expect(screen.getByText(/including button-only controls; empty slots are not advertised/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Copy pairing code" }));
    expect(mocks.clipboardWriteText).toHaveBeenCalledWith("3497-0112-233");
  });

  it("surfaces a failed network status instead of presenting pairing as ready", () => {
    mocks.server.configuration.matter_enabled = true;
    mocks.server.matter = matterStatus({
      transport: "failed",
      network_running: false,
      limitation: "Matter UDP bind failed: address already in use",
    });

    render(<MatterBridgeSettings />);

    expect(
      screen.getByText("Matter UDP bind failed: address already in use"),
    ).toBeInTheDocument();
    expect(screen.queryByText("Ready to commission")).not.toBeInTheDocument();
  });
});
