import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { PlaybackDefinition, PlaybackRuntimeProjection } from "../../../api/types";
import { playbackHasRuntimeError } from "../playbackRuntimeError";
import type { PlaybackBankController } from "./controller";
import { dynamicPlaybackSummary, playbackFaderDisplay, playbackFaderReadout } from "./feedback";
import { HardwarePlaybackCard } from "./HardwarePlaybackCard";
import { TouchPlaybackCard } from "./TouchPlaybackCard";
import type { PlaybackSlotProjection } from "./types";

afterEach(cleanup);
const playback = {
	address: { page: 3, position: 1 }, name: "Beam Show PWM",
	target: { type: "dynamic", assignment: { last_known_pool_number: 1, fader_mode: "size_and_master" } },
	has_fader: true, buttons: ["off", "on", "flash"], button_count: 3,
	fader: "master", go_activates: true, auto_off: true, xfade_millis: 0,
} as unknown as PlaybackDefinition;
const baseRuntime = {
	state: "active", enabled: true, controller_status: "winning", size: .4, master: .8,
	fader_value: .6, effective_speed_multiplier: 1, speed_source: "speed_group",
	target_count: 2, compatible_target_count: 2, lane_count: 2, supported_address_count: 4,
	missing_target_count: 0, unpatched_target_count: 0, warning: null,
	instance_id: "84f05b72-instance", controller_id: "e380abdf-controller",
};
const projection = (changes: Record<string, unknown> = {}) =>
	({ target: "dynamic", runtime: { ...baseRuntime, ...changes } }) as Extract<PlaybackRuntimeProjection, { target: "dynamic" }>;

it("keeps the complete feedback contract while displaying the actual configured fader position", () => {
	const p = projection();
	expect(playbackFaderReadout(playback, undefined, 60, p)).toBe("60%");
	expect(playbackFaderDisplay(playback, undefined, 60, p)).toBe(
		"ACTIVE · WINNING · Size 40% · Master 80% · 1.00× speed group · 2/2 compatible targets · 4/4 target/lane addresses · 0 missing · 0 unpatched · I 84f05b72 · C e380abdf",
	);
	expect(playbackFaderReadout(null, undefined, 0)).toBe("Empty");
	expect(playbackFaderReadout(playback, undefined, 0)).toBe("Unavailable");
});

it("keeps warnings visible and diagnostic identifiers secondary", () => {
	const summary = dynamicPlaybackSummary(projection({ missing_target_count: 1, compatible_target_count: 1, supported_address_count: 2, warning: "2 of 4 target/lane addresses run; 2 skipped (1 missing targets, 0 unpatched targets) · Aliasing: shortest segment" }));
	expect(summary).toEqual({ label: "ACTIVE", detail: "1/2 targets", warning: "1 missing · 2/4 addr · Aliasing" });
	expect(dynamicPlaybackSummary(projection({ state: "failed", warning: "No compatible targets" })).warning).toBe("No compatible targets");
	expect(dynamicPlaybackSummary({ target: "dynamic", runtime: null } as any)).toEqual({ label: "OFF" });
});

it.each([HardwarePlaybackCard, TouchPlaybackCard])("updates both mounted aliases without diagnostic text in the fader or false error state", (Card) => {
	const actions = ["OFF", "ON", "FLASH"].map(label => ({ id: label.toLowerCase(), label, onClick: vi.fn() }));
	const controller = { activePageNumber: 3, hardware: Card === HardwarePlaybackCard, rowCount: 2, state: {}, runtimeActions: {} } as PlaybackBankController;
	const alias = (p: ReturnType<typeof projection>, id: string) => <div data-testid={id}><Card
		controller={controller} slotData={{ playback, slot: 1, rowIndex: 0, row: { presentation: "fader", button_count: 3 } } as PlaybackSlotProjection}
		active={undefined} runtimeProjection={p} selected={false} kind="dynamic"
		summary={dynamicPlaybackSummary(p)} color="#cf4aa8" hasFader value={60}
		actions={actions} touchActions={actions} className={playbackHasRuntimeError(playback, p) ? "playback-runtime-error" : ""}
		cardStyle={undefined} commandTarget={null} interceptPointer={() => {}} interceptClick={() => {}}
	/></div>;
	const aliases = (p: ReturnType<typeof projection>) => <>{alias(p, "main")}{alias(p, "pane")}</>;
	const { rerender } = render(aliases(projection()));
	for (const state of ["active", "off", "pending", "failed", "off"]) {
		const p = projection({ state, enabled: state === "active", controller_status: state === "off" || state === "pending" ? "missing" : "winning", warning: state === "failed" ? "No compatible targets" : null });
		rerender(aliases(p));
		for (const id of ["main", "pane"]) {
			const scope = screen.getByTestId(id);
			expect(within(scope).getByText(state.toUpperCase(), { selector: ".playback-summary > span" })).toBeInTheDocument();
			expect(scope.textContent).not.toContain("84f05b72");
			expect(scope.textContent).not.toContain("target/lane addresses");
			expect(scope.querySelector("article")?.classList.contains("playback-runtime-error")).toBe(state === "failed");
			expect(scope.querySelector("article")?.getAttribute("aria-description")).toContain("Size 40% · Master 80%");
			if (state === "failed") expect(within(scope).getByText("No compatible targets")).toBeVisible();
		}
	}
});
