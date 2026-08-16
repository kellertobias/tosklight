import { cleanup, render, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SoundInputSettings } from "./SoundInputSettings";

const sound = vi.hoisted(() => ({
	deviceId: "default",
	devices: [] as Array<{ deviceId: string; label: string }>,
	permission: "granted",
	refreshInputs: vi.fn(async () => undefined),
	setDeskDevice: vi.fn(),
}));

vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({
	useSessionSnapshot: () => ({ desk: { id: "desk-1" } }),
}));

vi.mock("../control/useSoundDeviceSelection", () => ({
	useSoundDeviceSelection: () => sound,
}));

afterEach(cleanup);

describe("Sound input settings layout", () => {
	it("groups the microphone and refresh actions with explicit spacing", () => {
		const { container } = render(<SoundInputSettings />);
		const actions = container.querySelector(".sound-input-actions");
		expect(actions).not.toBeNull();
		expect(
			within(actions as HTMLElement)
				.getAllByRole("button")
				.map((button) => button.textContent),
		).toEqual(["Request microphone access", "Refresh inputs"]);
	});
});
