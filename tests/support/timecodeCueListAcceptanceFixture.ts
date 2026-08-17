export interface TimecodeCueListAcceptanceFixture {
	ids: {
		cueList: string;
		timecode: string;
		lane: string;
		clip: string;
		cues: readonly [string, string, string];
	};
	cues: Array<{
		id: string;
		number: string;
		name: string;
		fade_millis: number;
		delay_millis: number;
		out_fade_millis: number;
		out_delay_millis: number;
		trigger:
			| { type: "manual" }
			| { type: "follow"; delay_millis: number }
			| { type: "wait"; delay_millis: number };
	}>;
	clip: {
		id: string;
		start_frame: number;
		end_frame: number;
		start_cue_id: string;
		end_cue_id: string;
		start_behavior: "state";
		end_behavior: "release";
	};
}

/**
 * Stable cross-layer data for the pending real-server TIMECODE-002 operator test.
 * Consumers add fixture changes and the current complete CueList/Timecode wire envelopes.
 */
export function timecodeCueListAcceptanceFixture(): TimecodeCueListAcceptanceFixture {
	const cueIds = [
		"00000000-0000-4000-8000-000000000361",
		"00000000-0000-4000-8000-000000000362",
		"00000000-0000-4000-8000-000000000363",
	] as const;
	const ids = {
		cueList: "00000000-0000-4000-8000-000000000360",
		timecode: "00000000-0000-4000-8000-000000000364",
		lane: "00000000-0000-4000-8000-000000000365",
		clip: "00000000-0000-4000-8000-000000000366",
		cues: cueIds,
	};
	return {
		ids,
		cues: [
			{
				id: cueIds[0],
				number: "1",
				name: "Preset",
				fade_millis: 0,
				delay_millis: 0,
				out_fade_millis: 0,
				out_delay_millis: 0,
				trigger: { type: "manual" },
			},
			{
				id: cueIds[1],
				number: "2",
				name: "Overlapping transition",
				fade_millis: 1_000,
				delay_millis: 500,
				out_fade_millis: 750,
				out_delay_millis: 250,
				trigger: { type: "wait", delay_millis: 500 },
			},
			{
				id: cueIds[2],
				number: "3",
				name: "Final look",
				fade_millis: 500,
				delay_millis: 0,
				out_fade_millis: 1_000,
				out_delay_millis: 0,
				trigger: { type: "follow", delay_millis: 0 },
			},
		],
		clip: {
			id: ids.clip,
			start_frame: 44,
			end_frame: 220,
			start_cue_id: cueIds[0],
			end_cue_id: cueIds[2],
			start_behavior: "state",
			end_behavior: "release",
		},
	};
}
