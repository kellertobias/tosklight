import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, it, expect, vi } from "vitest";
import type {
	PositionCalibrationContext,
	InstalledPositionCalibration,
} from "@tosklight/patch";
import { PositionCalibrationEditor } from "./PositionCalibration";
afterEach(cleanup);
const context = (): PositionCalibrationContext => ({
	identity: {
		profile_id: crypto.randomUUID(),
		mode_id: crypto.randomUUID(),
		geometry_digest: "a".repeat(64),
	},
	axes: [{ node_id: crypto.randomUUID(), name: "Left tilt", role: "tilt" }],
});
it("authors a complete per-axis override without stacking family calibration", async () => {
	const ctx = context(),
		save = vi.fn().mockResolvedValue(true);
	const initial: InstalledPositionCalibration = {
		revision: 0,
		quality: "estimated",
		source: null,
		pan_zero_degrees: 900,
		tilt_zero_degrees: 15,
	};
	render(
		<PositionCalibrationEditor
			identity="17"
			initial={initial}
			context={ctx}
			invertTilt
			onSave={save}
			onClose={() => {}}
		/>,
	);
	fireEvent.click(screen.getByRole("button", { name: "Override Left tilt" }));
	expect(screen.getByLabelText("Left tilt zero offset (°)")).toHaveValue("15");
	fireEvent.change(screen.getByLabelText("Left tilt zero offset (°)"), {
		target: { value: "-735" },
	});
	fireEvent.click(screen.getByRole("button", { name: "Save" }));
	await waitFor(() =>
		expect(save).toHaveBeenCalledWith({
			...initial,
			axis_overrides: {
				version: 1,
				source_identity: ctx.identity,
				axes: [
					{ node_id: ctx.axes[0].node_id, zero_degrees: -735, invert: true },
				],
			},
		}),
	);
	expect(initial.axis_overrides).toBeUndefined();
});
it("retains stale overrides until explicitly cleared when the source changes", async () => {
	const ctx = context(),
		save = vi.fn().mockResolvedValue(true),
		initial: InstalledPositionCalibration = {
			revision: 0,
			quality: "unknown",
			pan_zero_degrees: 5,
			tilt_zero_degrees: 0,
			axis_overrides: {
				version: 1,
				source_identity: ctx.identity,
				axes: [
					{ node_id: ctx.axes[0].node_id, zero_degrees: 30, invert: false },
				],
			},
		};
	const view = render(
		<PositionCalibrationEditor
			identity="Copy"
			initial={initial}
			context={ctx}
			onSave={save}
			onClose={() => {}}
		/>,
	);
	view.rerender(
		<PositionCalibrationEditor
			identity="Copy"
			initial={initial}
			context={{
				...ctx,
				identity: { ...ctx.identity, profile_id: crypto.randomUUID() },
			}}
			onSave={save}
			onClose={() => {}}
		/>,
	);
	expect(screen.getByRole("status")).toHaveTextContent("inactive");
	expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
	fireEvent.click(screen.getByRole("button", { name: "Clear axis overrides" }));
	fireEvent.click(screen.getByRole("button", { name: "Save" }));
	await waitFor(() =>
		expect(save).toHaveBeenCalledWith({ ...initial, axis_overrides: null }),
	);
	expect(initial.axis_overrides?.axes[0].zero_degrees).toBe(30);
});
