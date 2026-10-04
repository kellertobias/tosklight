import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const colorIntentReport = vi.fn();
vi.mock("../attributeConfiguration/AttributeConfigurationActions", () => ({
	useAttributeConfigurationActions: () => ({ colorIntentReport }),
}));

import {
	COLOR_REPORT_MIN_INTERVAL_MILLIS,
	COLOR_REPORT_UNACCEPTED_RETRIES,
	useAcceptedColorReport,
} from "./useAcceptedColorReport";

const notYet = { accepted_frame: { state: "not_yet_output" }, heads: [] };
const accepted = { accepted_frame: { state: "accepted" }, heads: [{ fixture_id: "a" }] };

async function flush() {
	await act(async () => {
		await Promise.resolve();
	});
}

describe("useAcceptedColorReport", () => {
	beforeEach(() => {
		vi.useFakeTimers();
		colorIntentReport.mockReset();
	});
	afterEach(() => vi.useRealTimers());

	it("re-reads an early not-yet-output report until an accepted frame arrives", async () => {
		colorIntentReport.mockResolvedValueOnce(notYet).mockResolvedValueOnce(notYet).mockResolvedValue(accepted);
		const { result } = renderHook(() => useAcceptedColorReport(["a"], { enabled: true }));
		await flush();
		expect(result.current).toBeNull();
		for (let read = 0; read < 2; read += 1) {
			await act(async () => {
				vi.advanceTimersByTime(COLOR_REPORT_MIN_INTERVAL_MILLIS);
			});
			await flush();
		}
		expect(colorIntentReport).toHaveBeenCalledTimes(3);
		expect(result.current).toEqual(accepted);
	});

	it("stops re-reading after the bounded number of retries", async () => {
		colorIntentReport.mockResolvedValue(notYet);
		renderHook(() => useAcceptedColorReport(["a"], { enabled: true }));
		await flush();
		for (let read = 0; read < COLOR_REPORT_UNACCEPTED_RETRIES + 5; read += 1) {
			await act(async () => {
				vi.advanceTimersByTime(COLOR_REPORT_MIN_INTERVAL_MILLIS);
			});
			await flush();
		}
		expect(colorIntentReport).toHaveBeenCalledTimes(COLOR_REPORT_UNACCEPTED_RETRIES + 1);
	});
});
