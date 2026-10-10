import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	DeskStateDiagnosticsProvider,
	useDeskStateDiagnostics,
} from "./DeskStateDiagnosticsState";

afterEach(() => {
	cleanup();
	vi.useRealTimers();
});

describe("authoritative output inspection recovery", () => {
	it("preserves confirmed loss through a failed inspection then clears on actual send recovery", async () => {
		vi.useFakeTimers();
		const route = {
			protocol: "art_net",
			universe: 50,
			destination: "127.0.0.1:16454",
			enabled: true,
		};
		const status = {
			...route,
			delivery_state: "send_failed" as const,
			current_error: "Permission denied",
		};
		const configured = {
			kind: "output_route",
			id: "test-route",
			revision: 1,
			updated_at: "2026-10-08T16:00:00Z",
			body: {
				protocol: "art_net" as const,
				logical_universe: 1,
				destination_universe: 50,
				destination: route.destination,
				enabled: true,
				delivery_mode: "unicast" as const,
				minimum_slots: 512,
			},
		};
		const readDiagnostics = vi
			.fn()
			.mockResolvedValueOnce({
				outputRoutes: [route],
				outputDeliveryStatus: [status],
			})
			.mockRejectedValueOnce(new Error("inspection unavailable"))
			.mockResolvedValueOnce({
				outputRoutes: [route],
				outputDeliveryStatus: [
					{ ...status, delivery_state: "sending", current_error: null },
				],
			});
		function Probe() {
			return (
				<output>
					{useDeskStateDiagnostics().some(
						(item) => item.capabilityLoss === "dmx_output",
					)
						? "DMX send failed"
						: "No confirmed loss"}
				</output>
			);
		}
		await act(async () => {
			render(
				<DeskStateDiagnosticsProvider
					enabled
					readDiagnostics={readDiagnostics}
					outputRoutes={[configured]}
					pollMilliseconds={1500}
				>
					<Probe />
				</DeskStateDiagnosticsProvider>,
			);
		});
		expect(screen.getByText("DMX send failed")).toBeInTheDocument();
		await act(async () => {
			await vi.advanceTimersByTimeAsync(1500);
		});
		expect(screen.getByText("DMX send failed")).toBeInTheDocument();
		await act(async () => {
			await vi.advanceTimersByTimeAsync(1500);
		});
		expect(screen.getByText("No confirmed loss")).toBeInTheDocument();
		let resolveOlderInspection!: (snapshot: unknown) => void;
		readDiagnostics
			.mockImplementationOnce(
				() =>
					new Promise((resolve) => {
						resolveOlderInspection = resolve;
					}),
			)
			.mockResolvedValueOnce({
				outputRoutes: [route],
				outputDeliveryStatus: [
					{ ...status, delivery_state: "sending", current_error: null },
				],
			});
		await act(async () => {
			await vi.advanceTimersByTimeAsync(3000);
		});
		await act(async () => {
			resolveOlderInspection({
				outputRoutes: [route],
				outputDeliveryStatus: [status],
			});
		});
		expect(screen.getByText("No confirmed loss")).toBeInTheDocument();
	});
});
