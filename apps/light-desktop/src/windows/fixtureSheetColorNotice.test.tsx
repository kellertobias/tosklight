import { act, cleanup, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ColorIntentReport } from "../api/client/attributeConfiguration";
import { colorDetailsRequests } from "../features/colorReport/useAcceptedColorReport";
import { fixtureSheetColumns } from "./fixtureSheetColumns";
import type { FixtureSheetRow } from "./fixtureSheetProjection";
import { useFixtureSheetColorStatus } from "./useFixtureSheetColorStatus";

const app = vi.hoisted(() => ({ dispatch: vi.fn() }));
const reads = vi.hoisted(() => ({ calls: [] as string[][], report: null as ColorIntentReport | null }));

vi.mock("../state/AppContext", () => ({ useApp: () => ({ dispatch: app.dispatch }) }));
vi.mock("../features/familyEncoders/FamilyEncodersProvider", () => ({
	useSemanticFamilyEncoders: () => true,
}));
vi.mock("../features/attributeConfiguration/AttributeConfigurationActions", () => ({
	useAttributeConfigurationActions: () => ({
		colorIntentReport: async (ids: string[]) => {
			reads.calls.push(ids);
			return reads.report;
		},
	}),
}));

afterEach(() => {
	cleanup();
	app.dispatch.mockReset();
	reads.calls = [];
	colorDetailsRequests.reset();
});

const present = () => ({ current: false, containedCurrent: false, base: false, containedBase: false });
const row = (fixtureId: string) =>
	({
		id: 1, name: fixtureId, fixtureId, parentFixtureId: fixtureId, childFixtureIds: [], targetKind: "fixture",
		color: "#f00", colorAvailable: true, colorLabel: "Red", preloadColor: null, limitingGroups: [],
		sources: { dimmer: "default", color: "programmer", position: "default", beam: "default", focus: "default" },
	}) as unknown as FixtureSheetRow;

function accepted(): ColorIntentReport {
	return {
		color_model: "intent",
		accepted_frame: { state: "accepted", frame: null },
		heads: [
			{ fixture_id: "uv-less", fixture_number: 2, fixture_name: "PAR", owner_id: "uv-less", head_name: "", has_target: true, quality: "exact", engine: null, delta_uv: null, calibration_revision: null, uv: { status: "unsupported", clipped: false } },
			{ fixture_id: "fine", fixture_number: 3, fixture_name: "Wash", owner_id: "fine", head_name: "", has_target: true, quality: "exact", engine: null, delta_uv: null, calibration_revision: null, uv: { status: "applied", clipped: false } },
		],
	};
}

async function status(ids: string[]) {
	const hook = renderHook(() => useFixtureSheetColorStatus(true, ids, 1));
	await act(async () => undefined);
	return hook;
}

describe("Fixture Sheet quiet Color triangle", () => {
	it("reads one batched accepted-frame report for the rows on screen, never one per row", async () => {
		reads.report = accepted();
		await status(["uv-less", "fine", "other"]);
		expect(reads.calls).toEqual([["uv-less", "fine", "other"]]);
	});

	it("shows a passive triangle only beside an expected limitation", async () => {
		reads.report = accepted();
		const { result } = await status(["uv-less", "fine"]);
		const column = fixtureSheetColumns(false, present, "off", result.current).find((c) => c.id === "color");
		render(<>{column?.render?.(row("uv-less"), 0)}{column?.render?.(row("fine"), 1)}</>);
		const triangles = screen.getAllByTestId("fixture-sheet-color-notice");
		expect(triangles).toHaveLength(1);
		expect(triangles[0]).toHaveAccessibleName(/UV unavailable on this fixture/);
		expect(document.querySelector("[role=alert], [role=status], [aria-live]")).toBeNull();
		expect(document.activeElement).toBe(document.body);
	});

	it("opens the Color details on deliberate activation without a toast or a row selection", async () => {
		reads.report = accepted();
		const notices = vi.fn();
		window.addEventListener("light:desk-notice", notices);
		const { result } = await status(["uv-less"]);
		const column = fixtureSheetColumns(false, present, "off", result.current).find((c) => c.id === "color");
		const rowActivation = vi.fn();
		render(<div onClick={rowActivation} onPointerDown={rowActivation}>{column?.render?.(row("uv-less"), 0)}</div>);
		const triangle = screen.getByTestId("fixture-sheet-color-notice");
		fireEvent.pointerDown(triangle);
		fireEvent.click(triangle);
		window.removeEventListener("light:desk-notice", notices);
		expect(app.dispatch).toHaveBeenCalledWith({ type: "OPEN_SPECIAL_DIALOG", family: "Color" });
		expect(colorDetailsRequests.get()?.fixtureId).toBe("uv-less");
		expect(rowActivation).not.toHaveBeenCalled();
		expect(notices).not.toHaveBeenCalled();
	});

	it("shows nothing for a legacy report or a frame not yet output", async () => {
		reads.report = { ...accepted(), accepted_frame: undefined };
		const legacy = await status(["uv-less"]);
		expect(legacy.result.current).toBeUndefined();
		reads.report = { ...accepted(), accepted_frame: { state: "not_yet_available", frame: null } };
		const pending = await status(["uv-less"]);
		expect(pending.result.current).toBeUndefined();
	});
});
