import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
	ScheduleDraft,
	ScheduleProjection,
	SchedulerController,
	SchedulerSnapshot,
} from "../features/scheduler/contracts";
import { SchedulerWindow } from "./SchedulerWindow";

const occurrence = {
	id: "occurrence-1",
	instant: "2026-07-31T19:00:00+02:00",
	localDate: "2026-07-31",
	localTime: "19:00:00",
};

const schedule: ScheduleProjection = {
	definition: {
		id: "schedule-1",
		revision: 4,
		name: "Doors open",
		enabled: true,
		timing: {
			type: "calendar_expression",
			expression: "0 19 * * 5",
			summary: "Every Friday at 19:00",
		},
		target: {
			type: "playback",
			playbackId: "playback-3",
			label: "Foyer",
			page: 1,
			slot: 3,
			playback: 3,
			action: "go",
			masterPercent: null,
			fadeMillis: null,
		},
	},
	nextOccurrence: occurrence,
	upcomingOccurrences: [occurrence],
	lastResult: {
		status: "completed",
		occurredAt: "2026-07-24T19:00:00+02:00",
		message: "Playback started",
	},
	validationMessage: null,
};

function snapshot(
	overrides: Partial<SchedulerSnapshot> = {},
): SchedulerSnapshot {
	return {
		status: "ready",
		timezone: "Europe/Berlin",
		serverDate: "2026-07-28",
		schedules: [schedule],
		playbackTargets: [
			{
				id: "playback-3",
				label: "Foyer",
				page: 1,
				slot: 3,
				playback: 3,
				supportedActions: ["go", "pause", "release"],
				supportsMaster: true,
			},
		],
		canWrite: true,
		error: null,
		...overrides,
	};
}

function controller(
	overrides: Partial<SchedulerController> = {},
): SchedulerController {
	return {
		snapshot: snapshot(),
		preview: vi.fn(async () => ({
			status: "ready" as const,
			occurrences: [occurrence],
			message: null,
		})),
		create: vi.fn(async () => true),
		update: vi.fn(async () => true),
		setEnabled: vi.fn(async () => true),
		duplicate: vi.fn(async () => true),
		delete: vi.fn(async () => true),
		...overrides,
	};
}

beforeEach(() => {
	vi.stubGlobal(
		"confirm",
		vi.fn(() => true),
	);
});

afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
	vi.useRealTimers();
});

describe("SchedulerWindow production presentation", () => {
	it("renders deliberate loading, error, and unfiltered empty states", () => {
		const loading = controller({
			snapshot: snapshot({ status: "loading", schedules: [] }),
		});
		const view = render(<SchedulerWindow controller={loading} />);
		expect(screen.getByRole("status")).toHaveTextContent("Loading Schedules");

		const retry = vi.fn();
		view.rerender(
			<SchedulerWindow
				controller={controller({
					snapshot: snapshot({
						status: "error",
						schedules: [],
						error: "Desk disconnected",
					}),
					retry,
				})}
			/>,
		);
		expect(screen.getByRole("alert")).toHaveTextContent("Desk disconnected");
		fireEvent.click(screen.getByRole("button", { name: "Retry" }));
		expect(retry).toHaveBeenCalledOnce();

		view.rerender(
			<SchedulerWindow
				controller={controller({ snapshot: snapshot({ schedules: [] }) })}
			/>,
		);
		expect(screen.getByText("No Schedules")).toBeInTheDocument();
		expect(
			screen.getByText(/Interval, Calendar expression, or One-time/),
		).toBeInTheDocument();
	});

	it("shows authoritative status and exposes enable, duplicate, and delete mutations", async () => {
		const value = controller();
		render(<SchedulerWindow controller={value} />);
		expect(screen.getByText("1 schedules · Europe/Berlin")).toBeInTheDocument();
		expect(
			screen.getByText("Calendar expression · Every Friday at 19:00"),
		).toBeInTheDocument();
		expect(
			screen.getByText("Completed · Playback started"),
		).toBeInTheDocument();

		const actions = screen.getByLabelText("Schedule actions");
		fireEvent.click(within(actions).getByRole("button", { name: "Disable" }));
		fireEvent.click(within(actions).getByRole("button", { name: "Duplicate" }));
		fireEvent.click(within(actions).getByRole("button", { name: "Delete" }));
		await act(async () => undefined);

		expect(value.setEnabled).toHaveBeenCalledWith("schedule-1", 4, false);
		expect(value.duplicate).toHaveBeenCalledWith("schedule-1", 4);
		expect(value.delete).toHaveBeenCalledWith("schedule-1", 4);
	});

	it("uses the authoritative server date for Today", async () => {
		const value = controller();
		render(<SchedulerWindow controller={value} />);
		await act(async () => {
			fireEvent.click(screen.getByRole("button", { name: "Today" }));
		});
		expect(screen.getByText("Day filter · 2026-07-28")).toBeInTheDocument();
	});

	it("shows the server timezone and server-supplied preview occurrences", async () => {
		vi.useFakeTimers();
		const value = controller();
		render(<SchedulerWindow controller={value} />);

		fireEvent.click(screen.getByRole("button", { name: "+ Schedule" }));
		fireEvent.click(screen.getByRole("tab", { name: "When" }));
		expect(
			screen.getByText(/Authoritative server timezone:/),
		).toHaveTextContent("Europe/Berlin");
		fireEvent.click(screen.getByRole("tab", { name: "Action" }));
		expect(
			screen.getByText(/Macro scheduling becomes available/),
		).toBeInTheDocument();
		expect(
			screen.getByText("Loading authoritative preview…"),
		).toBeInTheDocument();

		await act(async () => {
			vi.advanceTimersByTime(200);
			await Promise.resolve();
		});
		expect(value.preview).toHaveBeenCalled();
		expect(screen.getByText("2026-07-31 · 19:00:00")).toBeInTheDocument();
	});

	it("submits exactly the typed server-previewed draft", async () => {
		vi.useFakeTimers();
		const value = controller();
		render(<SchedulerWindow controller={value} />);
		fireEvent.click(screen.getByRole("button", { name: "+ Schedule" }));
		await act(async () => {
			vi.advanceTimersByTime(200);
			await Promise.resolve();
		});
		fireEvent.click(screen.getByRole("button", { name: "Create Schedule" }));
		await act(async () => undefined);
		expect(value.create).toHaveBeenCalledOnce();
		const draft = vi.mocked(value.create).mock.calls[0][0] as ScheduleDraft;
		expect(draft).toMatchObject({
			name: "New schedule",
			enabled: true,
			timing: {
				type: "interval",
				everySeconds: 300,
				anchor: "activation",
			},
			target: {
				type: "playback",
				playbackId: "playback-3",
				action: "go",
			},
		});
	});
});
