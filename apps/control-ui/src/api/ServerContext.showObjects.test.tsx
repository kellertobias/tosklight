import { act, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
	usePortableGroups,
	usePresets,
	useShowObjectMutationState,
	useShowObjectsStore,
} from "../features/showObjects/ShowObjectsState";
import type { ShowObjectsStore } from "../features/showObjects/store";
import { ServerProvider, useServer } from "./ServerContext";

vi.mock("../features/server/useServerPolling", () => ({
	useServerPolling: vi.fn(),
}));
vi.mock("../features/server/useServerConnection", () => ({
	useServerConnection: vi.fn(),
}));
vi.mock("../features/server/useShowData", () => ({
	useShowObjects: () => vi.fn().mockResolvedValue(undefined),
	useServerRefresh: () => vi.fn().mockResolvedValue(undefined),
}));

let unrelatedServerRenders = 0;
let featureRenders = 0;
let disabledGroupRenders = 0;
let presetRenders = 0;

function UnrelatedServerConsumer() {
	useServer();
	unrelatedServerRenders += 1;
	return null;
}

function GroupConsumer() {
	const groups = usePortableGroups();
	featureRenders += 1;
	return <span>{groups[0]?.body.name ?? "No Group"}</span>;
}

function DisabledGroupConsumer() {
	const groups = usePortableGroups(false);
	disabledGroupRenders += 1;
	return <span>{groups[0]?.body.name ?? "No disabled Group"}</span>;
}

function PresetConsumer() {
	const presets = usePresets();
	presetRenders += 1;
	return <span>{presets[0]?.body.name ?? "No Preset"}</span>;
}

function MutationStatusConsumer() {
	const state = useShowObjectMutationState("group", "1");
	return (
		<span data-testid="mutation-status">
			{state.pending ? "pending" : "settled"}:{state.status}:
			{state.error?.message ?? ""}
		</span>
	);
}

function StoreCapture({
	onStore,
}: {
	onStore: (store: ShowObjectsStore) => void;
}) {
	onStore(useShowObjectsStore());
	return null;
}

describe("ServerProvider show-object ownership", () => {
	it("does not rerender an unrelated ServerContext consumer for a Group update", () => {
		unrelatedServerRenders = 0;
		featureRenders = 0;
		disabledGroupRenders = 0;
		presetRenders = 0;
		let store!: ShowObjectsStore;
		render(
			<ServerProvider>
				<UnrelatedServerConsumer />
				<GroupConsumer />
				<DisabledGroupConsumer />
				<PresetConsumer />
				<MutationStatusConsumer />
				<StoreCapture onStore={(value) => (store = value)} />
			</ServerProvider>,
		);
		expect(screen.getByText("No Group")).toBeTruthy();
		expect(unrelatedServerRenders).toBe(1);
		expect(featureRenders).toBe(1);
		expect(disabledGroupRenders).toBe(1);
		expect(presetRenders).toBe(1);

		act(() => {
			store.reset("show-a");
			store.setCollection("show-a", "group", [
				{
					kind: "group",
					id: "1",
					revision: 1,
					updated_at: "",
					body: { name: "Front", fixtures: [] },
				},
			]);
		});

		expect(screen.getByText("Front")).toBeTruthy();
		expect(screen.getByText("No disabled Group")).toBeTruthy();
		expect(disabledGroupRenders).toBe(1);
		expect(presetRenders).toBe(1);
		expect(screen.getByTestId("mutation-status").textContent).toBe(
			"settled:ready:",
		);
		let token = "";
		act(() => {
			token = store.beginOptimistic("show-a", "group", "1", {
				name: "Front Wash",
				fixtures: [],
			});
		});
		expect(screen.getByTestId("mutation-status").textContent).toBe(
			"pending:ready:",
		);
		act(() => store.rollback(token, new Error("revision conflict")));
		expect(screen.getByTestId("mutation-status").textContent).toBe(
			"settled:error:revision conflict",
		);
		const groupRendersBeforePreset = featureRenders;
		act(() => {
			store.setCollection("show-a", "preset", [
				{
					kind: "preset",
					id: "Color.1",
					revision: 1,
					updated_at: "",
					body: {
						name: "Blue",
						number: 1,
						family: "Color",
						values: {},
					},
				},
			]);
		});
		expect(screen.getByText("Blue")).toBeTruthy();
		expect(featureRenders).toBe(groupRendersBeforePreset);
		expect(presetRenders).toBeGreaterThan(1);
		expect(featureRenders).toBeGreaterThan(1);
		expect(unrelatedServerRenders).toBe(1);
	});
});
