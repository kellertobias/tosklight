import type { SetStateAction } from "react";
import { describe, expect, it } from "vitest";
import { createFeatureErrorGroup } from "./featureErrorReporting";

describe("createFeatureErrorGroup", () => {
	it("does not let success in one lane clear an error in another", () => {
		let visible: string | null = null;
		const errors = createFeatureErrorGroup((next: SetStateAction<string | null>) => {
			visible = typeof next === "function" ? next(visible) : next;
		});

		errors.reportSession(new Error("stream disconnected"));
		errors.reportMutation(null);

		expect(visible).toContain("stream disconnected");
	});

	it("restores an older active lane when the latest error clears", () => {
		let visible: string | null = null;
		const errors = createFeatureErrorGroup((next: SetStateAction<string | null>) => {
			visible = typeof next === "function" ? next(visible) : next;
		});

		errors.reportSession(new Error("stream disconnected"));
		errors.reportMutation(new Error("write rejected"));
		expect(visible).toContain("write rejected");

		errors.reportMutation(null);
		expect(visible).toContain("stream disconnected");
		errors.reportSession(null);
		expect(visible).toBeNull();
	});

	it("does not clear an unrelated global error", () => {
		let visible: string | null = null;
		const errors = createFeatureErrorGroup((next: SetStateAction<string | null>) => {
			visible = typeof next === "function" ? next(visible) : next;
		});

		errors.reportSession(new Error("stream disconnected"));
		visible = "show load failed";
		errors.reportSession(null);

		expect(visible).toBe("show load failed");
	});
});
