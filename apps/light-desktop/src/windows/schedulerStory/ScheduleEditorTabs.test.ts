import { describe, expect, it } from "vitest";
import type { PlaybackScheduleTarget } from "../../features/scheduler/contracts";
import {
	draftFromScheduleEditor,
	scheduleEditorForm,
} from "./ScheduleEditorTabs";

const targets: PlaybackScheduleTarget[] = [
	{
		id: "playback-main",
		label: "Main",
		page: 2,
		slot: 8,
		playback: 8,
		supportedActions: ["on", "off", "release"],
		supportsMaster: true,
	},
];

describe("ScheduleEditorTabs model", () => {
	it("preserves One-time seconds and the authoritative server date default", () => {
		const defaults = scheduleEditorForm(null, targets, "2031-12-31");
		expect(defaults.oneTimeDate).toBe("2031-12-31");
		expect(defaults.oneTimeTime).toBe("14:00:00");

		const form = {
			...defaults,
			timingType: "one_time" as const,
			oneTimeDate: "2032-01-01",
			oneTimeTime: "00:00:45",
		};
		expect(draftFromScheduleEditor(form, targets)?.timing).toEqual({
			type: "one_time",
			localDate: "2032-01-01",
			localTime: "00:00:45",
			remainEnabledAfterSuccess: false,
		});
	});

	it("sends Calendar expressions unchanged for authoritative server validation", () => {
		const form = {
			...scheduleEditorForm(null, targets, "2031-12-31"),
			timingType: "calendar_expression" as const,
			calendarExpression: " 0 14 * * 1#1 ",
		};
		expect(draftFromScheduleEditor(form, targets)?.timing).toEqual({
			type: "calendar_expression",
			expression: "0 14 * * 1#1",
			summary: "",
		});
	});

	it("keeps Interval as an anchored duration rather than converting it to cron", () => {
		const form = {
			...scheduleEditorForm(null, targets, "2031-12-31"),
			timingType: "interval" as const,
			intervalSeconds: 300,
			setMaster: true,
			masterPercent: 65,
			fadeSeconds: 2.5,
			action: "on" as const,
		};
		expect(draftFromScheduleEditor(form, targets)).toMatchObject({
			timing: {
				type: "interval",
				everySeconds: 300,
				anchor: "activation",
			},
			target: {
				type: "playback",
				playbackId: "playback-main",
				action: "on",
				masterPercent: 65,
				fadeMillis: 2_500,
			},
		});
	});
});
