import type { Meta, StoryObj } from "@storybook/react-vite";
import { GridDesktop, PaneView } from "@tosklight/ui/desktop";
import { useMemo, useState } from "react";
import { CommandSectionFixture } from "../../../ui-library/storybook/fixtures/controlSection";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import { AppShellView } from "../components/shell/AppShell";
import { Clock } from "../components/shell/Clock";
import { LeftDock } from "../components/shell/LeftDock";
import type {
	ScheduleDraft,
	ScheduleOccurrence,
	ScheduleProjection,
	SchedulerController,
} from "../features/scheduler/contracts";
import { SchedulerWindow } from "./SchedulerWindow";

const meta = {
	title: "ToskLight/Windows/Scheduler",
	tags: ["autodocs"],
	parameters: {
		layout: "fullscreen",
		docs: {
			description: {
				component:
					"Production Scheduler presentation driven by a deterministic Storybook authority adapter.",
			},
		},
	},
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

const occurrence = (
	id: string,
	localDate: string,
	localTime: string,
): ScheduleOccurrence => ({
	id,
	localDate,
	localTime,
	instant: `${localDate}T${localTime}+02:00`,
});

const initialSchedules: ScheduleProjection[] = [
	{
		definition: {
			id: "doors",
			revision: 3,
			name: "Doors open",
			enabled: true,
			timing: {
				type: "calendar_expression",
				expression: "0 19 * * 5",
				summary: "Every Friday at 19:00",
			},
			target: {
				type: "playback",
				playbackId: "playback-main-3",
				label: "Foyer Preset",
				page: 1,
				slot: 3,
				playback: 3,
				action: "go",
				masterPercent: null,
				fadeMillis: null,
			},
		},
		nextOccurrence: occurrence("doors-1", "2026-07-31", "19:00:00"),
		upcomingOccurrences: [
			occurrence("doors-1", "2026-07-31", "19:00:00"),
			occurrence("doors-2", "2026-08-07", "19:00:00"),
		],
		lastResult: {
			status: "completed",
			occurredAt: "2026-07-24T19:00:00+02:00",
			message: "Playback started",
		},
		validationMessage: null,
	},
	{
		definition: {
			id: "worklights",
			revision: 1,
			name: "Worklights after party",
			enabled: true,
			timing: {
				type: "one_time",
				localDate: "2026-07-30",
				localTime: "03:00:30",
				remainEnabledAfterSuccess: false,
			},
			target: {
				type: "playback",
				playbackId: "playback-party-8",
				label: "Worklights",
				page: 2,
				slot: 8,
				playback: 8,
				action: "on",
				masterPercent: 100,
				fadeMillis: 8_000,
			},
		},
		nextOccurrence: occurrence("worklights-1", "2026-07-30", "03:00:30"),
		upcomingOccurrences: [occurrence("worklights-1", "2026-07-30", "03:00:30")],
		lastResult: null,
		validationMessage: null,
	},
	{
		definition: {
			id: "missing",
			revision: 5,
			name: "Old festival playback",
			enabled: false,
			timing: {
				type: "interval",
				everySeconds: 300,
				anchor: "activation",
			},
			target: {
				type: "playback",
				playbackId: "deleted",
				label: "Deleted Playback",
				page: 2,
				slot: 10,
				playback: 10,
				action: "toggle",
				masterPercent: null,
				fadeMillis: null,
			},
		},
		nextOccurrence: null,
		upcomingOccurrences: [],
		lastResult: {
			status: "skipped",
			occurredAt: "2026-07-28T17:30:00+02:00",
			message: "Owning show was inactive",
		},
		validationMessage: "Playback target no longer exists",
	},
];

function projectionFromDraft(
	id: string,
	revision: number,
	draft: ScheduleDraft,
): ScheduleProjection {
	const first = occurrence(`${id}-next`, "2026-07-31", "19:00:00");
	return {
		definition: { id, revision, ...draft },
		nextOccurrence: draft.enabled ? first : null,
		upcomingOccurrences: draft.enabled ? [first] : [],
		lastResult: null,
		validationMessage: null,
	};
}

function SchedulerStory() {
	const [schedules, setSchedules] = useState(initialSchedules);
	const controller = useMemo<SchedulerController>(
		() => ({
			snapshot: {
				status: "ready",
				timezone: "Europe/Berlin",
				serverDate: "2026-07-28",
				schedules,
				canWrite: true,
				error: null,
				playbackTargets: [
					{
						id: "playback-main-3",
						label: "Foyer Preset",
						page: 1,
						slot: 3,
						playback: 3,
						supportedActions: ["go", "pause", "on", "off", "release", "toggle"],
						supportsMaster: true,
					},
					{
						id: "playback-party-8",
						label: "Worklights",
						page: 2,
						slot: 8,
						playback: 8,
						supportedActions: ["on", "off", "release", "toggle"],
						supportsMaster: true,
					},
				],
			},
			preview: async () => ({
				status: "ready",
				occurrences: [
					occurrence("preview-1", "2026-07-31", "19:00:00"),
					occurrence("preview-2", "2026-08-07", "19:00:00"),
				],
				message: null,
			}),
			create: async (draft) => {
				setSchedules((current) => [
					projectionFromDraft(`schedule-${current.length + 1}`, 1, draft),
					...current,
				]);
				return true;
			},
			update: async (id, revision, draft) => {
				setSchedules((current) =>
					current.map((schedule) =>
						schedule.definition.id === id
							? projectionFromDraft(id, revision + 1, draft)
							: schedule,
					),
				);
				return true;
			},
			setEnabled: async (id, revision, enabled) => {
				setSchedules((current) =>
					current.map((schedule) =>
						schedule.definition.id === id
							? {
									...schedule,
									definition: {
										...schedule.definition,
										revision: revision + 1,
										enabled,
									},
								}
							: schedule,
					),
				);
				return true;
			},
			duplicate: async (id) => {
				setSchedules((current) => {
					const source = current.find(
						(schedule) => schedule.definition.id === id,
					);
					if (!source) return current;
					return [
						{
							...source,
							definition: {
								...source.definition,
								id: `${id}-copy`,
								revision: 1,
								name: `${source.definition.name} copy`,
							},
						},
						...current,
					];
				});
				return true;
			},
			delete: async (id) => {
				setSchedules((current) =>
					current.filter((schedule) => schedule.definition.id !== id),
				);
				return true;
			},
		}),
		[schedules],
	);
	return (
		<ApplicationStateHarness>
			<AppShellView
				dock={
					<LeftDock
						presentation={{
							showIdentity: "Demo Show",
							showIndicator: {
								label: "Demo show",
								detail: "Deterministic Scheduler presentation.",
								className: "show-status-connected",
								connected: true,
							},
							clock: <Clock now={new Date(2026, 6, 28, 18, 42, 0)} />,
						}}
					/>
				}
				workspace={
					<GridDesktop id="scheduler-review" name="Scheduler Review">
						<PaneView
							maximized
							showHeader={false}
							pane={{
								id: "scheduler",
								title: "Scheduler",
								type: "scheduler",
								x: 1,
								y: 1,
								width: 24,
								height: 18,
							}}
						>
							<SchedulerWindow controller={controller} />
						</PaneView>
					</GridDesktop>
				}
				control={
					<CommandSectionFixture initialMode="playbacks" hardware={false} />
				}
			/>
		</ApplicationStateHarness>
	);
}

export function MarketingSchedulerApplication() {
	return <SchedulerStory />;
}

export const FullApplicationDiscussion: Story = {
	render: () => <SchedulerStory />,
};
