import { afterEach, describe, expect, it, vi } from "vitest";
import {
	consumeObjectEditorRequest,
	currentObjectEditorRequest,
	publishObjectEditorRequest,
	resetObjectEditorRequestsForTests,
	subscribeObjectEditorRequest,
} from "./objectEditorRequest";

describe("object editor requests", () => {
	afterEach(resetObjectEditorRequestsForTests);

	it("retains a cross-surface request until the exact editor consumes it", () => {
		const listener = vi.fn();
		const unsubscribe = subscribeObjectEditorRequest(listener);
		const request = { kind: "timecode" as const, objectId: "timecode-7" };

		publishObjectEditorRequest(request);

		expect(listener).toHaveBeenCalledWith(request);
		expect(currentObjectEditorRequest()).toBe(request);
		consumeObjectEditorRequest(request);
		expect(currentObjectEditorRequest()).toBeNull();
		unsubscribe();
	});

	it("keeps the request available while the editor subscriber reconnects", () => {
		const request = { kind: "macro" as const, objectId: "macro-12" };
		publishObjectEditorRequest(request);

		const listener = vi.fn();
		const unsubscribe = subscribeObjectEditorRequest(listener);
		expect(listener).not.toHaveBeenCalled();
		expect(currentObjectEditorRequest()).toBe(request);

		consumeObjectEditorRequest(request);
		expect(currentObjectEditorRequest()).toBeNull();
		unsubscribe();
	});
});
