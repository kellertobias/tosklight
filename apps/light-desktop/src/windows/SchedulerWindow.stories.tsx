/**
 * Intentional Storybook product-design surface for the future Scheduler.
 * This is the application-level mock contract, not abandoned production code;
 * retain it until the Scheduler is implemented and wired into ToskLight.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button, ModalFrame } from "@tosklight/ui";
import { GridDesktop, PaneView } from "@tosklight/ui/desktop";
import { useState } from "react";
import { CommandSectionFixture } from "../../../ui-library/storybook/fixtures/controlSection";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import { AppShellView } from "../components/shell/AppShell";
import { Clock } from "../components/shell/Clock";
import { LeftDock } from "../components/shell/LeftDock";
import {
	type ScheduleEvent,
	SchedulerWindow,
	type ScheduleTrigger,
} from "./SchedulerWindow";
import {
	type Frequency,
	type ScheduleEditorForm,
	ScheduleEditorTabs,
	type TargetKind,
	type TimingKind,
	weekdayOptions,
} from "./schedulerStory/ScheduleEditorTabs";

const meta = {
	title: "ToskLight/Windows/Scheduler",
	tags: ["autodocs"],
	parameters: {
		layout: "fullscreen",
		docs: {
			description: {
				component:
					"Storybook-first application mock for the future Scheduler. It uses the production shell and shared controls while keeping all changes in local Storybook state.",
			},
		},
	},
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

const fridayOccurrences = [
	"2026-07-03",
	"2026-07-10",
	"2026-07-17",
	"2026-07-24",
	"2026-07-31",
	"2026-08-07",
	"2026-08-14",
	"2026-08-21",
	"2026-08-28",
];
const weekdayOccurrences = [
	"2026-07-27",
	"2026-07-28",
	"2026-07-29",
	"2026-07-30",
	"2026-07-31",
	"2026-08-03",
	"2026-08-04",
	"2026-08-05",
	"2026-08-06",
	"2026-08-07",
];

const storyEvents: ScheduleEvent[] = [
	{
		id: "doors",
		name: "Doors open",
		enabled: true,
		timing: {
			type: "recurring",
			summary: "Every Friday at 19:00",
			time: "19:00",
			cron: "0 19 * * 5",
			occurrences: fridayOccurrences,
		},
		trigger: { type: "macro", macro: "101 · Venue open" },
	},
	{
		id: "worklights",
		name: "Worklights after party",
		enabled: true,
		timing: {
			type: "fixed",
			date: "2026-07-28",
			time: "03:00",
		},
		trigger: {
			type: "playback",
			page: 2,
			playback: 8,
			action: "Go",
			master: 100,
			fadeSeconds: 8,
		},
	},
	{
		id: "foyer",
		name: "Foyer preset",
		enabled: true,
		timing: {
			type: "recurring",
			summary: "Every Monday, Tuesday, Wednesday, Thursday, Friday at 17:30",
			time: "17:30",
			cron: "30 17 * * 1-5",
			occurrences: weekdayOccurrences,
		},
		trigger: {
			type: "playback",
			page: 1,
			playback: 3,
			action: "Toggle",
			master: 75,
			fadeSeconds: 4,
		},
	},
	{
		id: "festival",
		name: "Festival takeover",
		enabled: false,
		timing: {
			type: "fixed",
			date: "2026-08-01",
			time: "12:00",
		},
		trigger: { type: "macro", macro: "204 · Festival day" },
	},
];

function eventFormDefaults(event?: ScheduleEvent): ScheduleEditorForm {
	const recurring = event?.timing.type === "recurring";
	return {
		name: event?.name ?? "New schedule",
		enabled: event?.enabled ?? true,
		timingKind: (recurring ? "recurring" : "fixed") as TimingKind,
		date: event?.timing.type === "fixed" ? event.timing.date : "2026-07-29",
		time: event?.timing.time ?? "19:00",
		frequency: "selected_days" as Frequency,
		selectedDays: recurring ? ["5"] : ["1"],
		intervalMinutes: 15,
		intervalDays: 2,
		monthlyOrdinal: "first",
		monthlyWeekday: "1",
		cron: event?.timing.type === "recurring" ? event.timing.cron : "0 19 * * 5",
		targetKind: (event?.trigger.type ?? "macro") as TargetKind,
		macro:
			event?.trigger.type === "macro"
				? event.trigger.macro
				: "101 · Venue open",
		page: event?.trigger.type === "playback" ? event.trigger.page : 1,
		playback: event?.trigger.type === "playback" ? event.trigger.playback : 1,
		action: event?.trigger.type === "playback" ? event.trigger.action : "Go",
		setMaster:
			event?.trigger.type === "playback" && event.trigger.master !== undefined,
		master:
			event?.trigger.type === "playback" ? (event.trigger.master ?? 100) : 100,
		fade:
			event?.trigger.type === "playback" ? (event.trigger.fadeSeconds ?? 0) : 3,
	};
}

function frequencySummary(form: ScheduleEditorForm) {
	const selectedLabels = weekdayOptions
		.filter((day) => form.selectedDays.includes(day.value))
		.map((day) => day.label);
	const monthlyDay =
		weekdayOptions.find((day) => day.value === form.monthlyWeekday)?.label ??
		"Monday";
	if (form.frequency === "selected_days")
		return `Every ${selectedLabels.join(", ")} at ${form.time}`;
	if (form.frequency === "interval_minutes")
		return `Every ${form.intervalMinutes} minutes`;
	if (form.frequency === "interval_days")
		return `Every ${form.intervalDays} days at ${form.time}`;
	if (form.frequency === "monthly")
		return `Every ${form.monthlyOrdinal} ${monthlyDay} of the month at ${form.time}`;
	return `Custom rule · ${form.cron}`;
}

function cronFor(form: ScheduleEditorForm) {
	const [hour = "0", minute = "0"] = form.time.split(":");
	if (form.frequency === "selected_days")
		return `${minute} ${hour} * * ${form.selectedDays.join(",")}`;
	if (form.frequency === "interval_minutes")
		return `*/${form.intervalMinutes} * * * *`;
	if (form.frequency === "interval_days")
		return `${minute} ${hour} */${form.intervalDays} * *`;
	if (form.frequency === "monthly") {
		const ordinal = { first: "1", second: "2", third: "3", fourth: "4" }[
			form.monthlyOrdinal
		];
		return ordinal
			? `${minute} ${hour} * * ${form.monthlyWeekday}#${ordinal}`
			: `${minute} ${hour} * * ${form.monthlyWeekday}L`;
	}
	return "0 19 * * 5";
}

function scheduleFromForm(
	form: ScheduleEditorForm,
	event?: ScheduleEvent,
): ScheduleEvent {
	const summary = frequencySummary(form);
	const resolvedCron = form.frequency === "custom" ? form.cron : cronFor(form);
	const trigger: ScheduleTrigger =
		form.targetKind === "macro"
			? { type: "macro", macro: form.macro }
			: {
					type: "playback",
					page: form.page,
					playback: form.playback,
					action: form.action,
					...(form.setMaster
						? { master: form.master, fadeSeconds: form.fade }
						: {}),
				};
	return {
		id: event?.id ?? `schedule-${Date.now()}`,
		name: form.name,
		enabled: form.enabled,
		timing:
			form.timingKind === "fixed"
				? { type: "fixed", date: form.date, time: form.time }
				: {
						type: "recurring",
						summary,
						time: form.time,
						cron: resolvedCron,
						occurrences:
							event?.timing.type === "recurring"
								? event.timing.occurrences
								: fridayOccurrences,
					},
		trigger,
	};
}

function ScheduleEditor({
	event,
	onClose,
	onSave,
}: {
	event?: ScheduleEvent;
	onClose: () => void;
	onSave: (event: ScheduleEvent) => void;
}) {
	const [editorTab, setEditorTab] = useState("schedule");
	const [form, setForm] = useState(() => eventFormDefaults(event));
	const summary = frequencySummary(form);
	const resolvedCron = form.frequency === "custom" ? form.cron : cronFor(form);
	const update = <Key extends keyof ScheduleEditorForm>(
		key: Key,
		value: ScheduleEditorForm[Key],
	) => setForm((current) => ({ ...current, [key]: value }));
	return (
		<ModalFrame
			id="scheduler-editor"
			ariaLabel={event ? `Edit ${event.name}` : "Create schedule"}
			dialogClassName="scheduler-editor-modal"
			title={event ? "Edit schedule" : "New schedule"}
			details="Choose when it happens, then what the desk should do"
			tabs={[
				{ id: "schedule", label: "Name" },
				{ id: "when", label: "When" },
				{ id: "trigger", label: "Action" },
			]}
			activeTab={editorTab}
			onTabChange={setEditorTab}
			actions={
				<Button
					variant="primary"
					onClick={() => onSave(scheduleFromForm(form, event))}
				>
					{event ? "Save changes" : "Create schedule"}
				</Button>
			}
			onClose={onClose}
		>
			<div className="scheduler-editor">
				<ScheduleEditorTabs
					activeTab={editorTab}
					form={form}
					summary={summary}
					resolvedCron={resolvedCron}
					update={update}
				/>
			</div>
		</ModalFrame>
	);
}

function FullApplicationSchedulerMock() {
	const [events, setEvents] = useState(storyEvents);
	const [editing, setEditing] = useState<ScheduleEvent | "new" | null>(null);
	const save = (event: ScheduleEvent) => {
		setEvents((current) => {
			const existing = current.some((candidate) => candidate.id === event.id);
			return existing
				? current.map((candidate) =>
						candidate.id === event.id ? event : candidate,
					)
				: [event, ...current];
		});
		setEditing(null);
	};
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
							<SchedulerWindow
								initialEvents={events}
								onCreate={() => setEditing("new")}
								onEdit={setEditing}
							/>
						</PaneView>
					</GridDesktop>
				}
				control={
					<CommandSectionFixture initialMode="playbacks" hardware={false} />
				}
			/>
			{editing && (
				<ScheduleEditor
					event={editing === "new" ? undefined : editing}
					onClose={() => setEditing(null)}
					onSave={save}
				/>
			)}
		</ApplicationStateHarness>
	);
}

export function MarketingSchedulerApplication() {
	return <FullApplicationSchedulerMock />;
}

export const FullApplicationDiscussion: Story = {
	render: () => <FullApplicationSchedulerMock />,
};
