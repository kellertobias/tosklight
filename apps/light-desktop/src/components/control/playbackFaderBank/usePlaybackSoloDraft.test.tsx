import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type {
	PlaybackSoloCapability,
	PlaybackSoloRegion,
} from "../../../features/playbackSolo/contracts";
import { usePlaybackSoloDraft } from "./usePlaybackSoloDraft";

const region: PlaybackSoloRegion = {
	id: "existing",
	name: "Across pages",
	members: [
		{ page: 2, position: 101 },
		{ page: 4, position: 102 },
	],
};
function capability() {
	return {
		authorityId: "show",
		authorityGeneration: 1,
		available: true,
		error: null,
		getRegions: () => [region],
		isSaving: () => false,
		subscribe: () => () => {},
		activate: () => () => {},
		load: async () => [region],
		clearError: () => {},
		save: vi.fn<PlaybackSoloCapability["save"]>(async (regions) => regions),
	} satisfies PlaybackSoloCapability;
}

describe("Solo cell draft", () => {
	it("creates only on Save and cancels without writing", async () => {
		const cap = capability();
		const { result } = renderHook(() => usePlaybackSoloDraft(cap, [region]));
		act(() => result.current.beginEdit());
		expect(result.current.selecting).toBe(true);
		await act(async () => result.current.create());
		expect(result.current.error).toBe("Select at least two Playback addresses");
		expect(cap.save).not.toHaveBeenCalled();
		act(() => result.current.cancel());
		expect(result.current.selecting).toBe(false);
		expect(cap.save).not.toHaveBeenCalled();
		act(() => result.current.beginEdit());
		act(() => result.current.setSelected([...region.members]));
		await act(async () => result.current.create());
		expect(cap.save).toHaveBeenCalledTimes(1);
		const saved = cap.save.mock.calls[0][0];
		expect(saved).toHaveLength(2);
		expect(saved[0]).toEqual(region);
		expect(saved[1].members).toEqual(region.members);
		expect(saved[1].id).not.toBe(region.id);
		expect(result.current.selecting).toBe(false);
	});
	it("edits the same ID, retaining other pages and unrelated regions", async () => {
		const cap = capability();
		const peer = { ...region, id: "peer", name: "Peer" };
		const { result } = renderHook(() =>
			usePlaybackSoloDraft(cap, [region, peer]),
		);
		act(() => result.current.beginEdit(region));
		act(() =>
			result.current.setSelected((members) => [
				...members.filter((address) => address.page !== 2),
				{ page: 2, position: 201 },
			]),
		);
		act(() => result.current.setName("Renamed"));
		await act(async () => result.current.create());
		expect(cap.save.mock.calls[0][0]).toEqual([
			{
				id: "existing",
				name: "Renamed",
				members: [
					{ page: 4, position: 102 },
					{ page: 2, position: 201 },
				],
			},
			peer,
		]);
	});
	it("keeps a refused draft visible and discards it on authority replacement", async () => {
		const cap = capability();
		cap.save.mockResolvedValueOnce(null);
		const { result, rerender } = renderHook(
			({ generation }) =>
				usePlaybackSoloDraft({ ...cap, authorityGeneration: generation }, [
					region,
				]),
			{ initialProps: { generation: 1 } },
		);
		act(() => result.current.beginEdit(region));
		await act(async () => result.current.create());
		expect(result.current.selecting).toBe(true);
		rerender({ generation: 2 });
		expect(result.current.selecting).toBe(false);
		expect(result.current.selected).toEqual([]);
	});
	it("does not let an old accepted save close a replacement authority’s draft", async () => {
		const cap = capability();
		let finish: (regions: readonly PlaybackSoloRegion[]) => void = () => {};
		cap.save.mockImplementationOnce(
			() =>
				new Promise((resolve) => {
					finish = resolve;
				}),
		);
		const { result, rerender } = renderHook(
			({ generation }) =>
				usePlaybackSoloDraft({ ...cap, authorityGeneration: generation }, [
					region,
				]),
			{ initialProps: { generation: 1 } },
		);
		act(() => result.current.beginEdit(region));
		let pending: Promise<void>;
		act(() => {
			pending = result.current.create();
		});
		rerender({ generation: 2 });
		act(() => result.current.beginEdit());
		await act(async () => {
			finish([region]);
			await pending;
		});
		expect(result.current.selecting).toBe(true);
		expect(result.current.selected).toEqual([]);
		expect(result.current.creating).toBe(false);
	});
});

it("retains the latest region color when saving a membership and name draft", async () => {
	const cap = capability();
	const colored = { ...region, color: "#12aBcF" };
	const peer = { ...region, id: "peer", color: "#ff00aa" };
	const { result, rerender } = renderHook(
		({ regions }) => usePlaybackSoloDraft(cap, regions),
		{ initialProps: { regions: [colored, peer] } },
	);
	act(() => result.current.beginEdit(colored));
	act(() => result.current.setName("Renamed"));
	act(() =>
		result.current.setSelected([...region.members, { page: 3, position: 7 }]),
	);
	// A concurrent authoritative presentation change must survive this membership-only edit.
	rerender({ regions: [{ ...colored, color: "#345678" }, peer] });
	await act(async () => result.current.create());
	expect(cap.save).toHaveBeenCalledExactlyOnceWith([
		{
			...colored,
			name: "Renamed",
			color: "#345678",
			members: [...region.members, { page: 3, position: 7 }],
		},
		peer,
	]);
});
