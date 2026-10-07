import { describe, expect, it, vi } from "vitest";
import type { OutputReadoutSnapshot } from "../../api/familyEncoderModels";
import type { VisualizationRuntimeScope } from "./contracts";
import { MAX_READOUT_CLAIM, VisualizationRuntimeSession } from "./session";
import { VisualizationRuntimeStore } from "./store";
import type {
	VisualizationRuntimeStream,
	VisualizationRuntimeStreamObserver,
	VisualizationRuntimeTransport,
} from "./transport";

const scope: VisualizationRuntimeScope = {
	showId: "11111111-1111-4111-8111-111111111111",
	sessionId: "22222222-2222-4222-8222-222222222222",
	authorityKey: "server-a",
};

function harness(streaming = true) {
	const store = new VisualizationRuntimeStore();
	store.reset(scope);
	let observer: VisualizationRuntimeStreamObserver | null = null;
	const stream = {
		updateClaims: vi.fn(),
		updateReadoutClaim: vi.fn(),
		close: vi.fn(),
	} satisfies VisualizationRuntimeStream;
	const transport: VisualizationRuntimeTransport = {
		loadSnapshot: vi.fn(async () => {
			throw new Error("not under test");
		}),
		...(streaming
			? {
					openStream: (_scope, next) => {
						observer = next;
						return stream;
					},
				}
			: {}),
	};
	const session = new VisualizationRuntimeSession({ scope, store, transport });
	return {
		session,
		stream,
		deliver: (snapshot: OutputReadoutSnapshot) => observer?.readouts?.(snapshot, 1),
	};
}

const lastClaim = (stream: ReturnType<typeof harness>["stream"]) =>
	stream.updateReadoutClaim.mock.calls.at(-1)?.[0];

describe("visualization session readout claims", () => {
	it("merges consumers in claim order, bounds the claim and releases it", () => {
		const { session, stream, deliver } = harness();
		const first = vi.fn();
		const second = vi.fn();
		const releaseFirst = session.claimReadouts(["a", "b"], first, "encoders");
		const releaseSecond = session.claimReadouts(["b", "c"], second, "modal");

		expect(lastClaim(stream)).toEqual(["a", "b", "c"]);
		expect(stream.updateClaims).toHaveBeenLastCalledWith(["normal"], 3, false, false);

		const snapshot = { lane: "normal", owners: [] } as unknown as OutputReadoutSnapshot;
		deliver(snapshot);
		expect(first).toHaveBeenCalledWith(snapshot);
		expect(second).toHaveBeenCalledWith(snapshot);

		releaseFirst?.();
		expect(lastClaim(stream)).toEqual(["b", "c"]);
		releaseSecond?.();
		expect(lastClaim(stream)).toBeNull();
		expect(stream.close).toHaveBeenCalled();

		const many = Array.from({ length: MAX_READOUT_CLAIM + 20 }, (_, index) => `f${index}`);
		session.claimReadouts(many, vi.fn());
		expect(lastClaim(stream)).toHaveLength(MAX_READOUT_CLAIM);
	});

	it("reports no claim without a stream so the caller reads over HTTP", () => {
		const { session } = harness(false);
		expect(session.claimReadouts(["a"], vi.fn())).toBeNull();
	});
});
