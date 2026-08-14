import { describe, expect, it, vi } from "vitest";
import { ApiRequestError } from "../../api/ApiRequestError";
import type { ShowObjectActionOutcome } from "../../api/types";
import type {
	TimecodeDefinition,
	TimecodeObjectRecord,
} from "../../api/types/timecode";
import {
	TimecodeAutosaveWriter,
	timecodePatch,
} from "./TimecodeAutosaveWriter";

const SHOW_ID = "00000000-0000-4000-8000-000000000161";
const TIMECODE_ID = "00000000-0000-4000-8000-000000000162";

describe("TimecodeAutosaveWriter", () => {
	it("creates a new empty-slot Timecode without a Save action", async () => {
		const create = vi.fn(async (_showId: string, body: TimecodeDefinition) =>
			outcome(1, body),
		);
		const writer = new TimecodeAutosaveWriter(SHOW_ID, null, {
			create,
			update: vi.fn(),
			objects: vi.fn(),
		});

		await expect(writer.enqueue(definition())).resolves.toMatchObject({
			revision: 1,
		});
		expect(create).toHaveBeenCalledOnce();
	});

	it("serializes successive edits and advances the authoritative revision", async () => {
		const initial = record(4, definition());
		let server = initial;
		let releaseFirst!: () => void;
		const firstGate = new Promise<void>((resolve) => {
			releaseFirst = resolve;
		});
		const update = vi.fn(
			async (
				_showId: string,
				_id: string,
				expectedRevision: number,
				patch: ReturnType<typeof timecodePatch>,
			) => {
				expect(expectedRevision).toBe(server.revision);
				if (server.revision === 4) await firstGate;
				server = record(server.revision + 1, {
					...server.definition,
					...patch,
				});
				return outcome(server.revision, server.definition);
			},
		);
		const writer = new TimecodeAutosaveWriter(SHOW_ID, initial, {
			create: vi.fn(),
			update,
			objects: vi.fn(),
		});
		const first = writer.enqueue({ ...initial.definition, name: "Opening" });
		await vi.waitFor(() => expect(update).toHaveBeenCalledOnce());
		const second = writer.enqueue({
			...initial.definition,
			name: "Opening",
			auto_start: true,
		});
		releaseFirst();

		await Promise.all([first, second]);
		expect(update.mock.calls.map((call) => call[2])).toEqual([4, 5]);
		expect(update.mock.calls.map((call) => call[3])).toEqual([
			{ name: "Opening" },
			{ auto_start: true },
		]);
		expect(writer.current()).toEqual(
			record(6, {
				...initial.definition,
				name: "Opening",
				auto_start: true,
			}),
		);
	});

	it("builds intent patches containing only changed fields", () => {
		const before = definition();
		const marker = {
			id: "00000000-0000-4000-8000-000000000163",
			frame: 44,
			name: "Intro",
		};
		expect(
			timecodePatch(before, {
				...before,
				duration_frame: 880,
				markers: [marker],
			}),
		).toEqual({ duration_frame: 880, markers: [marker] });
	});

	it("refreshes authority and deliberately reapplies after a revision conflict", async () => {
		const initial = record(4, definition());
		const concurrent = record(5, {
			...initial.definition,
			transport_offset_frame: 22,
		});
		const update = vi
			.fn()
			.mockRejectedValueOnce(
				new ApiRequestError("object revision conflict", 409),
			)
			.mockResolvedValueOnce(
				outcome(6, { ...concurrent.definition, name: "Reapplied" }),
			);
		const objects = vi.fn(async () => ({
			show_revision: 5,
			objects: [concurrent],
		}));
		const writer = new TimecodeAutosaveWriter(SHOW_ID, initial, {
			create: vi.fn(),
			update,
			objects,
		});

		await writer.enqueue({ ...initial.definition, name: "Reapplied" });

		expect(objects).toHaveBeenCalledOnce();
		expect(update.mock.calls.map((call) => call[2])).toEqual([4, 5]);
		expect(update.mock.calls.map((call) => call[3])).toEqual([
			{ name: "Reapplied" },
			{ name: "Reapplied" },
		]);
		expect(writer.current()?.definition.transport_offset_frame).toBe(22);
	});
});

function definition(): TimecodeDefinition {
	return {
		id: TIMECODE_ID,
		number: 1,
		name: "Timecode 1",
		duration_frame: 440,
		transport_offset_frame: 0,
		auto_start: false,
		markers: [],
		lanes: [],
	};
}

function record(
	revision: number,
	body: TimecodeDefinition,
): TimecodeObjectRecord {
	return { revision, definition: structuredClone(body) };
}

function outcome(
	revision: number,
	body: TimecodeDefinition,
): ShowObjectActionOutcome {
	return {
		request_id: `save-${revision}`,
		replayed: false,
		show_id: SHOW_ID,
		show_revision: revision,
		object: {
			kind: "timecode",
			id: TIMECODE_ID,
			revision,
			updated_at: "2026-08-11T00:00:00Z",
			body,
		},
	};
}
