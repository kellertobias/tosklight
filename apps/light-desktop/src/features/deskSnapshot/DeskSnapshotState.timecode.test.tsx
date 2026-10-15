import { cleanup, render, screen } from "@testing-library/react";
import { act } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	BootstrapSnapshot,
	RuntimeCapabilityEvent,
	SessionResponse,
} from "../../api/types";
import { createServerEventRouter } from "../server/serverEventRouter";
import type { ServerState } from "../server/useServerState";
import {
	DeskSnapshotStateProvider,
	useActiveTimecode,
	useOutputHealth,
	useTimecodeClockDetail,
} from "./DeskSnapshotState";
import { DeskSnapshotStore } from "./store";

afterEach(cleanup);

describe("live authoritative clock indication", () => {
	it("adopts the production event without replacing bootstrap or notifying unrelated readers", () => {
		const store = new DeskSnapshotStore();
		const bootstrap = {
			active_timecode: null,
			output_health: { frame_hz: 44 },
		} as BootstrapSnapshot;
		const session = {
			session_id: "session-a",
			desk: { id: "desk-a" },
		} as SessionResponse;
		store.install(bootstrap, session);
		const state = {
			session,
			deskSnapshotStore: store,
			mediaServers: [],
			setBootstrap: vi.fn(),
		} as unknown as ServerState;
		const route = createServerEventRouter(() => state, session, vi.fn());
		let healthRenders = 0;
		function Clock() {
			const current = useActiveTimecode();
			const detail = useTimecodeClockDetail();
			return (
				<output data-testid="clock" title={detail ?? undefined}>
					{current ?? "No Timecode"}
				</output>
			);
		}
		function Health() {
			healthRenders++;
			return <span>{useOutputHealth()?.frame_hz}</span>;
		}
		render(
			<DeskSnapshotStateProvider store={store}>
				<Clock />
				<Health />
			</DeskSnapshotStateProvider>,
		);
		expect(screen.getByTestId("clock")).toHaveTextContent("No Timecode");
		const before = healthRenders;
		act(() =>
			route({
				type: "timecode_clock_changed",
				snapshot: {
					revision: 1,
					selected_source: "artnet:127.0.0.1:2",
					active_source: "artnet:127.0.0.1:2",
					state: "locked",
					current: "00:00:02:22",
					source_rate: "25",
					desk_rate: "44",
					rate_warning: "Timecode converted from 25 to 44 fps",
				},
			} as unknown as RuntimeCapabilityEvent),
		);
		expect(screen.getByTestId("clock")).toHaveTextContent("00:00:02:22");
		expect(screen.getByTestId("clock")).toHaveAttribute(
			"title",
			expect.stringContaining("artnet:127.0.0.1:2"),
		);
		expect(screen.getByTestId("clock")).toHaveAttribute(
			"title",
			expect.stringContaining("25 to 44"),
		);
		act(() =>
			route({
				type: "timecode_clock_changed",
				snapshot: {
					revision: 2,
					selected_source: "artnet:127.0.0.1:2",
					active_source: null,
					state: "lost",
					current: null,
					source_rate: "25",
					desk_rate: "44",
					rate_warning: "Timecode converted from 25 to 44 fps",
				},
			} as unknown as RuntimeCapabilityEvent),
		);
		expect(screen.getByTestId("clock")).toHaveTextContent("No Timecode");
		expect(screen.getByTestId("clock")).toHaveAttribute(
			"title",
			expect.stringContaining("source lost"),
		);
		act(() =>
			route({
				type: "timecode_clock_changed",
				snapshot: {
					revision: 3,
					selected_source: "artnet:127.0.0.1:2",
					active_source: "artnet:127.0.0.1:2",
					state: "locked",
					current: "00:00:03:00",
					source_rate: "44",
					desk_rate: "44",
					rate_warning: null,
				},
			} as unknown as RuntimeCapabilityEvent),
		);
		expect(screen.getByTestId("clock")).toHaveTextContent("00:00:03:00");
		act(() =>
			route({
				type: "timecode_clock_changed",
				snapshot: { revision: 4, current: 123 },
			} as unknown as RuntimeCapabilityEvent),
		);
		expect(screen.getByTestId("clock")).toHaveTextContent("No Timecode");
		expect(screen.getByTestId("clock")).toHaveAttribute(
			"title",
			"Timecode status unavailable",
		);
		expect(healthRenders).toBe(before);
		expect(store.getSnapshot().bootstrap).toBe(bootstrap);
		expect(state.setBootstrap).not.toHaveBeenCalled();
	});
	it("retires late connection snapshots and cannot rewind a newer stream revision", () => {
		const store = new DeskSnapshotStore();
		const sample = {
			revision: 4,
			selected_source: "artnet:127.0.0.1:2",
			active_source: "artnet:127.0.0.1:2",
			state: "locked",
			current: "00:00:02:22",
			source_rate: "25",
			desk_rate: "44",
			rate_warning: null,
		};
		const first = store.resetClock();
		expect(store.installClock(sample, first)).toBe(true);
		expect(
			store.installClock(
				{ ...sample, revision: 3, current: "00:00:01:00" },
				first,
			),
		).toBe(false);
		expect(store.getSnapshot().clock?.current).toBe("00:00:02:22");
		const second = store.resetClock();
		expect(store.installClock({ ...sample, revision: 100 }, first)).toBe(false);
		expect(store.getSnapshot().clock).toBeNull();
		expect(store.installClock({ ...sample, revision: 1 }, second)).toBe(true);
	});
});
