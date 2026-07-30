export type ScheduleTiming =
	| {
			type: "interval";
			everySeconds: number;
			anchor: "activation";
	  }
	| {
			type: "calendar_expression";
			expression: string;
			summary: string;
	  }
	| {
			type: "one_time";
			localDate: string;
			localTime: string;
			remainEnabledAfterSuccess: boolean;
	  };

export type ScheduledPlaybackAction =
	| "go"
	| "pause"
	| "on"
	| "off"
	| "release"
	| "toggle";

export type ScheduleTarget =
	| {
			type: "playback";
			playbackId: string;
			label: string;
			page: number;
			slot: number;
			playback: number;
			action: ScheduledPlaybackAction;
			masterPercent: number | null;
			fadeMillis: number | null;
	  }
	| {
			type: "macro";
			macroId: string;
			label: string;
	  };

export interface ScheduleDefinition {
	id: string;
	revision: number;
	name: string;
	enabled: boolean;
	timing: ScheduleTiming;
	target: ScheduleTarget;
}

export interface ScheduleOccurrence {
	id: string;
	instant: string;
	localDate: string;
	localTime: string;
}

export interface ScheduleResult {
	status: "completed" | "failed" | "skipped";
	occurredAt: string;
	message: string;
}

export interface ScheduleProjection {
	definition: ScheduleDefinition;
	nextOccurrence: ScheduleOccurrence | null;
	upcomingOccurrences: readonly ScheduleOccurrence[];
	lastResult: ScheduleResult | null;
	validationMessage: string | null;
	pending?: boolean;
}

export interface PlaybackScheduleTarget {
	id: string;
	label: string;
	page: number;
	slot: number;
	playback: number;
	supportedActions: readonly ScheduledPlaybackAction[];
	supportsMaster: boolean;
}

export interface SchedulerSnapshot {
	status: "loading" | "ready" | "error";
	timezone: string;
	serverDate: string;
	schedules: readonly ScheduleProjection[];
	playbackTargets: readonly PlaybackScheduleTarget[];
	canWrite: boolean;
	error: string | null;
}

export interface ScheduleDraft {
	name: string;
	enabled: boolean;
	timing: ScheduleTiming;
	target: ScheduleTarget;
}

export interface SchedulePreview {
	status: "ready" | "invalid";
	occurrences: readonly ScheduleOccurrence[];
	message: string | null;
}

export interface SchedulerController {
	snapshot: SchedulerSnapshot;
	activate?(): () => void;
	retry?(): void | Promise<void>;
	preview(draft: ScheduleDraft, signal: AbortSignal): Promise<SchedulePreview>;
	create(draft: ScheduleDraft): Promise<boolean>;
	update(
		id: string,
		expectedRevision: number,
		draft: ScheduleDraft,
	): Promise<boolean>;
	setEnabled(
		id: string,
		expectedRevision: number,
		enabled: boolean,
	): Promise<boolean>;
	duplicate(id: string, expectedRevision: number): Promise<boolean>;
	delete(id: string, expectedRevision: number): Promise<boolean>;
}
