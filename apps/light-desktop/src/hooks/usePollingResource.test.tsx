import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { usePollingResource } from "./usePollingResource";

afterEach(() => {
	vi.useRealTimers();
});

describe("usePollingResource", () => {
	it("does no work until its consumer becomes active", async () => {
		vi.useFakeTimers();
		const load = vi.fn().mockResolvedValue("frame");
		const onValue = vi.fn();
		const { rerender } = renderHook(
			({ enabled }) =>
				usePollingResource({
					enabled,
					intervalMillis: 250,
					load,
					onValue,
				}),
			{ initialProps: { enabled: false } },
		);

		await act(() => vi.advanceTimersByTimeAsync(1_000));
		expect(load).not.toHaveBeenCalled();

		rerender({ enabled: true });
		await act(async () => undefined);
		expect(load).toHaveBeenCalledOnce();
		expect(onValue).toHaveBeenCalledWith("frame");
	});

	it("does not overlap a slow load and stops applying values after deactivation", async () => {
		vi.useFakeTimers();
		let resolve!: (value: string) => void;
		const load = vi.fn(
			() => new Promise<string>((complete) => (resolve = complete)),
		);
		const onValue = vi.fn();
		const { rerender } = renderHook(
			({ enabled }) =>
				usePollingResource({
					enabled,
					intervalMillis: 100,
					load,
					onValue,
				}),
			{ initialProps: { enabled: true } },
		);

		await act(() => vi.advanceTimersByTimeAsync(500));
		expect(load).toHaveBeenCalledOnce();
		rerender({ enabled: false });
		await act(async () => resolve("late"));
		expect(onValue).not.toHaveBeenCalled();
	});

	it("refreshes immediately when the projection scope changes", async () => {
		vi.useFakeTimers();
		const load = vi.fn().mockResolvedValue("frame");
		const { rerender } = renderHook(
			({ scope }) =>
				usePollingResource({
					enabled: true,
					intervalMillis: 1_000,
					load,
					onValue: vi.fn(),
					refreshKey: scope,
				}),
			{ initialProps: { scope: "live" } },
		);
		await act(async () => undefined);

		rerender({ scope: "preload" });
		await act(async () => undefined);
		expect(load).toHaveBeenCalledTimes(2);
	});
});
