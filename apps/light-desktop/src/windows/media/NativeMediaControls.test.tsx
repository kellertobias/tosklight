import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useNativeMediaEffects } from "./NativeMediaControls";

const blur = {
	index: 0,
	effectType: "blur",
	label: "Blur",
	enabled: true,
	mix: 0.5,
	supported: true,
	capabilityDetail: null,
	parameters: [
		{
			id: "blur-amount",
			label: "Blur amount",
			value: 0.8,
			defaultValue: 0.5,
		},
	],
};

describe("native Media effect controls", () => {
	it("loads only the selected layer and does not reload when callback identities change", async () => {
		const load = vi.fn().mockResolvedValue({ effectLayers: [[blur], []] });
		const update = vi.fn();
		const { result, rerender } = renderHook(
			({ loadAction }) =>
				useNativeMediaEffects({
					active: true,
					fixtureId: "fixture-1",
					layer: 0,
					load: loadAction,
					update,
				}),
			{ initialProps: { loadAction: load } },
		);
		await waitFor(() => expect(result.current.slots).toEqual([blur]));
		rerender({ loadAction: vi.fn().mockResolvedValue({ effectLayers: [[]] }) });
		expect(load).toHaveBeenCalledTimes(1);
		expect(result.current.slots).toEqual([blur]);
	});

	it("updates the selected layer without clearing the visible controls", async () => {
		const updated = { ...blur, mix: 0.75 };
		const update = vi.fn().mockResolvedValue([updated]);
		const { result } = renderHook(() =>
			useNativeMediaEffects({
				active: true,
				fixtureId: "fixture-1",
				layer: 0,
				load: vi.fn().mockResolvedValue({ effectLayers: [[blur]] }),
				update,
			}),
		);
		await waitFor(() => expect(result.current.slots).toEqual([blur]));
		act(() => result.current.change("effect-0-blur-amount", 0.75));
		expect(result.current.slots).toEqual([blur]);
		await waitFor(() => expect(result.current.slots).toEqual([updated]));
		expect(update).toHaveBeenCalledWith(
			"fixture-1",
			0,
			"effect-0-blur-amount",
			0.75,
		);
	});

	it("ignores an older response after a newer native effect change", async () => {
		let resolveFirst: ((slots: (typeof blur)[]) => void) | undefined;
		let resolveSecond: ((slots: (typeof blur)[]) => void) | undefined;
		const update = vi
			.fn()
			.mockImplementationOnce(
				() =>
					new Promise<(typeof blur)[]>((resolve) => {
						resolveFirst = resolve;
					}),
			)
			.mockImplementationOnce(
				() =>
					new Promise<(typeof blur)[]>((resolve) => {
						resolveSecond = resolve;
					}),
			);
		const { result } = renderHook(() =>
			useNativeMediaEffects({
				active: true,
				fixtureId: "fixture-1",
				layer: 0,
				load: vi.fn().mockResolvedValue({ effectLayers: [[blur]] }),
				update,
			}),
		);
		await waitFor(() => expect(result.current.slots).toEqual([blur]));
		act(() => {
			result.current.change("effect-0-blur-amount", 0.6);
			result.current.change("effect-0-blur-amount", 0.9);
		});
		const newest = { ...blur, mix: 0.9 };
		await act(async () => resolveSecond?.([newest]));
		expect(result.current.slots).toEqual([newest]);
		await act(async () => resolveFirst?.([{ ...blur, mix: 0.6 }]));
		expect(result.current.slots).toEqual([newest]);
	});
});
