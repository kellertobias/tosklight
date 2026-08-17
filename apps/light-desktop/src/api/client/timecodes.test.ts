import { describe, expect, it, vi } from "vitest";
import { TimecodesApiClient } from "./timecodes";
import type { LiveClientTransport } from "./transport";

function transport() {
	return {
		request: vi.fn().mockResolvedValue({}),
		blob: vi.fn(),
		absoluteUrl: vi.fn(),
		currentDeskId: vi.fn().mockReturnValue("desk-a"),
		sendAction: vi.fn().mockResolvedValue({}),
	} satisfies LiveClientTransport;
}

describe("TimecodesApiClient", () => {
	it("sends revision-safe portable mutations with a replay identity", async () => {
		const wire = transport();
		vi.spyOn(crypto, "randomUUID").mockReturnValue(
			"00000000-0000-4000-8000-000000000070",
		);
		const client = new TimecodesApiClient(wire);
		await client.update("show-a", "timecode/a", 4, { name: "Opener" });
		const [path, init] = wire.request.mock.calls[0] ?? [];
		expect(path).toBe("/api/v2/timecodes/actions");
		expect(new Headers(init.headers).get("x-tosk-show")).toBe("show-a");
		expect(JSON.parse(String(init.body))).toEqual({
			request_id: "00000000-0000-4000-8000-000000000070",
			action: {
				type: "update",
				timecode_id: "timecode/a",
				expected_revision: 4,
				patch: { name: "Opener" },
			},
		});
	});

	it("uses snapshots for reads and one ordered live frame for transport", async () => {
		const wire = transport();
		wire.request
			.mockResolvedValueOnce({ show_revision: 0, objects: [] })
			.mockResolvedValueOnce([])
			.mockResolvedValueOnce({
				timecode_id: "timecode/a",
				revision: 1,
				state: "stopped",
				frame: 0,
				duration_frame: 440,
				audio_linked: false,
				cue_list_clips: [
					{
						lane_id: "lane-a",
						cue_list_id: "cue-list-a",
						clip_id: "clip-a",
						state: "unable",
						cue_id: null,
						cue_start_frame: null,
						message: "start Cue does not exist",
					},
				],
			})
			.mockResolvedValueOnce({ peaks: [] });
		wire.sendAction.mockResolvedValueOnce({
			timecode_id: "timecode/a",
			revision: 2,
			state: "paused",
			frame: 220,
			duration_frame: 440,
			audio_linked: false,
			cue_list_clips: [],
		});
		const client = new TimecodesApiClient(wire);
		await client.objects("show-a");
		await client.runtime("show-a");
		const snapshot = await client.snapshot("show-a", "timecode/a");
		await client.waveform("show-a", "timecode/a");
		await client.transportAction("show-a", "timecode/a", {
			type: "seek",
			frame: 220,
		});
		expect(wire.request.mock.calls.map(([path]) => path)).toEqual([
			"/api/v2/timecodes",
			"/api/v2/timecodes/runtime",
			"/api/v2/timecodes/timecode%2Fa/runtime",
			"/api/v2/timecodes/timecode%2Fa/audio/waveform",
		]);
		expect(wire.sendAction).toHaveBeenCalledOnce();
		expect(wire.sendAction).toHaveBeenCalledWith({
			type: "timecode",
			request: {
				timecode_id: "timecode/a",
				action: { type: "seek", frame: 220 },
			},
		});
		expect(snapshot.cue_list_clips[0]).toMatchObject({
			state: "unable",
			message: "start Cue does not exist",
		});
	});

	it("discovers server outputs and uploads audio as a show-scoped raw import", async () => {
		const wire = transport();
		const client = new TimecodesApiClient(wire);
		await client.outputDevices();
		const file = new File(
			[new Uint8Array([0x49, 0x44, 0x33])],
			"intro mix.mp3",
			{
				type: "audio/mpeg",
			},
		);
		await client.importAudio("show-a", file);
		expect(wire.request.mock.calls[0]?.[0]).toBe(
			"/api/v2/timecodes/audio/outputs",
		);
		const [path, init] = wire.request.mock.calls[1] ?? [];
		expect(path).toBe("/api/v2/timecodes/audio/import?name=intro%20mix.mp3");
		expect(init.method).toBe("POST");
		expect(init.body).toBe(file);
		expect(new Headers(init.headers).get("content-type")).toBe("audio/mpeg");
		expect(new Headers(init.headers).get("x-tosk-show")).toBe("show-a");
	});
});
