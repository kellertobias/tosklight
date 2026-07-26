import { afterEach, describe, expect, it, vi } from "vitest";
import {
	registerControlSurfaceTarget,
	resetControlSurfaceTargetsForTests,
	routeControlSurfaceIntent,
	routeControlSurfaceIntentWithFeedback,
} from "./registry";

afterEach(resetControlSurfaceTargetsForTests);

describe("control-surface interaction ownership", () => {
	it("routes to the highest-priority capable target", () => {
		const background = vi.fn();
		const active = vi.fn();
		registerControlSurfaceTarget({
			id: "background",
			priority: 10,
			accepts: () => true,
			handle: background,
		});
		registerControlSurfaceTarget({
			id: "active",
			priority: 20,
			accepts: ({ type }) => type === "set",
			handle: active,
		});
		expect(
			routeControlSurfaceIntent({ type: "set", source: "keyboard" }),
		).toEqual({ status: "handled", targetId: "active" });
		expect(active).toHaveBeenCalledOnce();
		expect(background).not.toHaveBeenCalled();
	});

	it("rejects ambiguous and missing targets without invoking either candidate", () => {
		const first = vi.fn();
		const second = vi.fn();
		for (const [id, handle] of [
			["first", first],
			["second", second],
		] as const)
			registerControlSurfaceTarget({
				id,
				priority: 10,
				accepts: () => true,
				handle,
			});
		expect(
			routeControlSurfaceIntent({ type: "set", source: "hardware" }),
		).toEqual({
			status: "ambiguous",
			targetIds: ["first", "second"],
		});
		expect(first).not.toHaveBeenCalled();
		expect(second).not.toHaveBeenCalled();
		resetControlSurfaceTargetsForTests();
		expect(routeControlSurfaceIntent({ type: "set", source: "touch" })).toEqual(
			{ status: "missing" },
		);
	});

	it("reports a visible safe error for missing and ambiguous ownership", () => {
		const errors: string[] = [];
		const receive = (event: Event) =>
			errors.push((event as CustomEvent<string>).detail);
		window.addEventListener("light:command-error", receive);
		routeControlSurfaceIntentWithFeedback({
			type: "set",
			source: "hardware",
		});
		for (const id of ["left", "right"])
			registerControlSurfaceTarget({
				id,
				priority: 10,
				accepts: () => true,
				handle: vi.fn(),
			});
		routeControlSurfaceIntentWithFeedback({
			type: "set",
			source: "context_menu",
		});
		window.removeEventListener("light:command-error", receive);
		expect(errors).toEqual([
			"No active control surface can handle SET.",
			"More than one active control surface can handle SET. Choose one surface and try again.",
		]);
	});
});
