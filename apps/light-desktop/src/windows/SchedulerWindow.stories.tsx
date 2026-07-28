/**
 * Intentional Storybook product-design surface for the future Scheduler.
 * This is the application-level mock contract, not abandoned production code;
 * retain it until the Scheduler is implemented and wired into ToskLight.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import {
	Button,
	FormField,
	FormLayout,
	ModalFrame,
	ModalLayer,
	ModalTitleBar,
	NumberField,
	SelectField,
	SwitchField,
	TextField,
} from "@tosklight/ui";
import { GridDesktop, PaneView } from "@tosklight/ui/desktop";
import { useMemo, useState } from "react";
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

type TimingKind = "fixed" | "recurring";
type Frequency =
	| "selected_days"
	| "interval_minutes"
	| "interval_days"
	| "monthly"
	| "custom";
type TargetKind = "macro" | "playback";

const weekdayOptions = [
	{ value: "1", short: "Mon", label: "Monday" },
	{ value: "2", short: "Tue", label: "Tuesday" },
	{ value: "3", short: "Wed", label: "Wednesday" },
	{ value: "4", short: "Thu", label: "Thursday" },
	{ value: "5", short: "Fri", label: "Friday" },
	{ value: "6", short: "Sat", label: "Saturday" },
	{ value: "0", short: "Sun", label: "Sunday" },
] as const;

interface PlaybackPageOption {
	value: string;
	label: string;
	groups: {
		label: string;
		playbacks: { number: number; name: string; detail: string }[];
	}[];
}

const playbackPages: PlaybackPageOption[] = [
	{
		value: "1",
		label: "Page 1 · Main",
		groups: [
			{
				label: "Main looks",
				playbacks: [
					{ number: 1, name: "Opening Look", detail: "Cue 4 · Solo" },
					{ number: 2, name: "Front Wash", detail: "12 fixtures · 48%" },
					{ number: 3, name: "Foyer Preset", detail: "House · 24%" },
					{ number: 4, name: "Circle Dynamic", detail: "Dynamic 12" },
				],
			},
			{
				label: "Utilities",
				playbacks: [
					{ number: 5, name: "Grand Master", detail: "Master · 100%" },
					{ number: 6, name: "House Preset", detail: "House · 24%" },
				],
			},
		],
	},
	{
		value: "2",
		label: "Page 2 · Party",
		groups: [
			{
				label: "Party",
				playbacks: [
					{ number: 1, name: "Dance Floor", detail: "Cue list 21" },
					{ number: 4, name: "Mirrorball", detail: "Cue list 24" },
					{ number: 8, name: "Worklights", detail: "Utility · 100%" },
					{ number: 10, name: "Last Song", detail: "Macro handoff" },
				],
			},
		],
	},
	{
		value: "3",
		label: "Page 3 · House",
		groups: [
			{
				label: "House and safety",
				playbacks: [
					{ number: 1, name: "Auditorium", detail: "House · 60%" },
					{ number: 2, name: "Foyer", detail: "House · 80%" },
					{ number: 3, name: "Cleaning", detail: "Worklight preset" },
				],
			},
		],
	},
];

function eventFormDefaults(event?: ScheduleEvent) {
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

function frequencySummary(
	frequency: Frequency,
	selectedDays: string[],
	intervalMinutes: number,
	intervalDays: number,
	monthlyOrdinal: string,
	monthlyWeekday: string,
	time: string,
	cron: string,
) {
	const selectedLabels = weekdayOptions
		.filter((day) => selectedDays.includes(day.value))
		.map((day) => day.label);
	const monthlyDay =
		weekdayOptions.find((day) => day.value === monthlyWeekday)?.label ??
		"Monday";
	if (frequency === "selected_days")
		return `Every ${selectedLabels.join(", ")} at ${time}`;
	if (frequency === "interval_minutes")
		return `Every ${intervalMinutes} minutes`;
	if (frequency === "interval_days")
		return `Every ${intervalDays} days at ${time}`;
	if (frequency === "monthly")
		return `Every ${monthlyOrdinal} ${monthlyDay} of the month at ${time}`;
	return `Custom rule · ${cron}`;
}

function cronFor(
	frequency: Frequency,
	selectedDays: string[],
	intervalMinutes: number,
	intervalDays: number,
	monthlyOrdinal: string,
	monthlyWeekday: string,
	time: string,
) {
	const [hour = "0", minute = "0"] = time.split(":");
	if (frequency === "selected_days")
		return `${minute} ${hour} * * ${selectedDays.join(",")}`;
	if (frequency === "interval_minutes") return `*/${intervalMinutes} * * * *`;
	if (frequency === "interval_days")
		return `${minute} ${hour} */${intervalDays} * *`;
	if (frequency === "monthly") {
		const ordinal = { first: "1", second: "2", third: "3", fourth: "4" }[
			monthlyOrdinal
		];
		return ordinal
			? `${minute} ${hour} * * ${monthlyWeekday}#${ordinal}`
			: `${minute} ${hour} * * ${monthlyWeekday}L`;
	}
	return "0 19 * * 5";
}

function PlaybackPickerField({
	page,
	playback,
	onChange,
}: {
	page: number;
	playback: number;
	onChange: (page: number, playback: number) => void;
}) {
	const [open, setOpen] = useState(false);
	const [pickerPage, setPickerPage] = useState(String(page));
	const currentPage =
		playbackPages.find((candidate) => candidate.value === String(page)) ??
		playbackPages[0];
	const currentPlayback = currentPage.groups
		.flatMap((group) => group.playbacks)
		.find((candidate) => candidate.number === playback);
	const pickerPageDefinition =
		playbackPages.find((candidate) => candidate.value === pickerPage) ??
		playbackPages[0];
	return (
		<FormField
			label="Playback"
			description="Choose a page, then select a named playback."
		>
			<Button
				className="scheduler-playback-trigger"
				contentAlign="left"
				aria-haspopup="dialog"
				onClick={() => {
					setPickerPage(String(page));
					setOpen(true);
				}}
			>
				<span>
					<strong>{currentPlayback?.name ?? `Playback ${playback}`}</strong>
					<small>
						{currentPage.label} · Playback {playback}
					</small>
				</span>
				<b aria-hidden="true">›</b>
			</Button>
			{open && (
				<ModalLayer
					ariaLabel="Choose playback"
					className="scheduler-playback-picker-layer"
					dialogClassName="scheduler-playback-picker"
					onClose={() => setOpen(false)}
				>
					<ModalTitleBar
						title="Choose playback"
						details="Select a page, then choose from its available playbacks"
						actions={
							<SelectField
								className="ui-icon-group-control"
								label="Playback page"
								ariaLabel="Playback page"
								size="compact"
								value={pickerPage}
								options={playbackPages.map((candidate) => ({
									value: candidate.value,
									label: candidate.label,
								}))}
								onChange={setPickerPage}
							/>
						}
						closeLabel="Close playback picker"
						onClose={() => setOpen(false)}
					/>
					<div className="scheduler-playback-groups">
						{pickerPageDefinition.groups.map((group) => (
							<section key={group.label}>
								<h3>{group.label}</h3>
								<div className="scheduler-playback-grid">
									{group.playbacks.map((candidate) => (
										<Button
											key={candidate.number}
											active={
												Number(pickerPage) === page &&
												candidate.number === playback
											}
											contentAlign="left"
											onClick={() => {
												onChange(Number(pickerPage), candidate.number);
												setOpen(false);
											}}
										>
											<b>{candidate.number}</b>
											<span>
												<strong>{candidate.name}</strong>
												<small>{candidate.detail}</small>
											</span>
										</Button>
									))}
								</div>
							</section>
						))}
					</div>
				</ModalLayer>
			)}
		</FormField>
	);
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
	const initial = useMemo(() => eventFormDefaults(event), [event]);
	const [editorTab, setEditorTab] = useState("schedule");
	const [name, setName] = useState(initial.name);
	const [enabled, setEnabled] = useState(initial.enabled);
	const [timingKind, setTimingKind] = useState(initial.timingKind);
	const [date, setDate] = useState(initial.date);
	const [time, setTime] = useState(initial.time);
	const [frequency, setFrequency] = useState(initial.frequency);
	const [selectedDays, setSelectedDays] = useState<string[]>(
		initial.selectedDays,
	);
	const [intervalMinutes, setIntervalMinutes] = useState(
		initial.intervalMinutes,
	);
	const [intervalDays, setIntervalDays] = useState(initial.intervalDays);
	const [monthlyOrdinal, setMonthlyOrdinal] = useState(initial.monthlyOrdinal);
	const [monthlyWeekday, setMonthlyWeekday] = useState(initial.monthlyWeekday);
	const [cron, setCron] = useState(initial.cron);
	const [targetKind, setTargetKind] = useState(initial.targetKind);
	const [macro, setMacro] = useState(initial.macro);
	const [page, setPage] = useState(initial.page);
	const [playback, setPlayback] = useState(initial.playback);
	const [action, setAction] = useState(initial.action);
	const [shouldSetMaster, setShouldSetMaster] = useState(initial.setMaster);
	const [master, setMasterValue] = useState(initial.master);
	const [fade, setFade] = useState(initial.fade);
	const summary = frequencySummary(
		frequency,
		selectedDays,
		intervalMinutes,
		intervalDays,
		monthlyOrdinal,
		monthlyWeekday,
		time,
		cron,
	);
	const resolvedCron =
		frequency === "custom"
			? cron
			: cronFor(
					frequency,
					selectedDays,
					intervalMinutes,
					intervalDays,
					monthlyOrdinal,
					monthlyWeekday,
					time,
				);
	const save = () => {
		const trigger: ScheduleTrigger =
			targetKind === "macro"
				? { type: "macro", macro }
				: {
						type: "playback",
						page,
						playback,
						action,
						...(shouldSetMaster ? { master, fadeSeconds: fade } : {}),
					};
		onSave({
			id: event?.id ?? `schedule-${Date.now()}`,
			name,
			enabled,
			timing:
				timingKind === "fixed"
					? { type: "fixed", date, time }
					: {
							type: "recurring",
							summary,
							time,
							cron: resolvedCron,
							occurrences:
								event?.timing.type === "recurring"
									? event.timing.occurrences
									: fridayOccurrences,
						},
			trigger,
		});
	};
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
				<Button variant="primary" onClick={save}>
					{event ? "Save changes" : "Create schedule"}
				</Button>
			}
			onClose={onClose}
		>
			<div className="scheduler-editor">
				{editorTab === "schedule" && (
					<section className="scheduler-editor-section">
						<FormLayout columns={2}>
							<TextField
								label="Name"
								value={name}
								onChange={(event) => setName(event.target.value)}
							/>
							<SwitchField
								label="Enabled"
								checked={enabled}
								onChange={(event) => setEnabled(event.target.checked)}
							/>
						</FormLayout>
					</section>
				)}
				{editorTab === "when" && (
					<section className="scheduler-editor-section">
						<div
							className={
								timingKind === "fixed"
									? "scheduler-fixed-timing-row"
									: "scheduler-when-repeat-row"
							}
						>
							<SelectField
								label="When"
								value={timingKind}
								options={[
									{ value: "fixed", label: "Fixed date" },
									{ value: "recurring", label: "Repeating rule" },
								]}
								onChange={setTimingKind}
							/>
							{timingKind === "fixed" ? (
								<>
									<TextField
										label="Date"
										value={date}
										description="YYYY-MM-DD"
										onChange={(event) => setDate(event.target.value)}
									/>
									<div className="scheduler-compact-time-field">
										<TextField
											label="Time"
											value={time}
											description="24-hour time"
											onChange={(event) => setTime(event.target.value)}
										/>
									</div>
								</>
							) : (
								<SelectField
									className="scheduler-repeat-control"
									label="Repeat"
									value={frequency}
									options={[
										{
											value: "selected_days",
											label: "On selected days of the week",
										},
										{
											value: "interval_minutes",
											label: "Every <n> minutes",
										},
										{ value: "interval_days", label: "Every <n> days" },
										{
											value: "monthly",
											label: "On a weekday of the month",
										},
										{ value: "custom", label: "Advanced / custom cron" },
									]}
									onChange={setFrequency}
								/>
							)}
						</div>
						{timingKind === "recurring" && frequency === "custom" ? (
							<div className="scheduler-custom-rule">
								<TextField
									label="Cron expression"
									value={cron}
									description="minute · hour · day · month · weekday"
									onChange={(event) => setCron(event.target.value)}
								/>
								<aside aria-label="Next five occurrences">
									<strong>Next 5 occurrences</strong>
									<ol>
										<li>
											<span>Friday, 31 July 2026</span>
											<time>19:00</time>
										</li>
										<li>
											<span>Friday, 7 August 2026</span>
											<time>19:00</time>
										</li>
										<li>
											<span>Friday, 14 August 2026</span>
											<time>19:00</time>
										</li>
										<li>
											<span>Friday, 21 August 2026</span>
											<time>19:00</time>
										</li>
										<li>
											<span>Friday, 28 August 2026</span>
											<time>19:00</time>
										</li>
									</ol>
								</aside>
							</div>
						) : timingKind === "recurring" ? (
							<div className="scheduler-rule-builder">
								{frequency === "selected_days" && (
									<FormField
										label="Days"
										description="Choose one or more days."
										className="scheduler-weekday-field"
									>
										<fieldset className="scheduler-weekdays">
											<legend>Days of the week</legend>
											{weekdayOptions.map((day) => {
												const active = selectedDays.includes(day.value);
												return (
													<Button
														key={day.value}
														active={active}
														aria-pressed={active}
														onClick={() =>
															setSelectedDays((current) =>
																active
																	? current.length > 1
																		? current.filter(
																				(value) => value !== day.value,
																			)
																		: current
																	: [...current, day.value],
															)
														}
													>
														{day.short}
													</Button>
												);
											})}
										</fieldset>
									</FormField>
								)}
								{frequency === "interval_days" && (
									<NumberField
										label="Interval"
										min={1}
										unit="days"
										value={intervalDays}
										onChange={(event) =>
											setIntervalDays(Number(event.target.value) || 1)
										}
									/>
								)}
								{frequency === "interval_minutes" && (
									<NumberField
										label="Interval"
										min={1}
										max={59}
										unit="minutes"
										value={intervalMinutes}
										onChange={(event) =>
											setIntervalMinutes(Number(event.target.value) || 1)
										}
									/>
								)}
								{frequency === "monthly" && (
									<>
										<SelectField
											label="Occurrence"
											value={monthlyOrdinal}
											options={[
												{ value: "first", label: "First" },
												{ value: "second", label: "Second" },
												{ value: "third", label: "Third" },
												{ value: "fourth", label: "Fourth" },
												{ value: "last", label: "Last" },
											]}
											onChange={setMonthlyOrdinal}
										/>
										<SelectField
											label="Weekday"
											value={monthlyWeekday}
											options={weekdayOptions.map((day) => ({
												value: day.value,
												label: day.label,
											}))}
											onChange={setMonthlyWeekday}
										/>
									</>
								)}
								{frequency !== "interval_minutes" && (
									<div className="scheduler-compact-time-field">
										<TextField
											label="Time"
											value={time}
											description="24-hour time"
											onChange={(event) => setTime(event.target.value)}
										/>
									</div>
								)}
								<div className="scheduler-rule-summary">
									<small>This schedule will run</small>
									<strong>{summary}</strong>
									<code>{resolvedCron}</code>
								</div>
							</div>
						) : null}
					</section>
				)}
				{editorTab === "trigger" && (
					<section className="scheduler-editor-section">
						<div className="scheduler-trigger-target-row">
							<SelectField
								label="Trigger"
								value={targetKind}
								options={[
									{ value: "macro", label: "Macro" },
									{ value: "playback", label: "Playback" },
								]}
								onChange={setTargetKind}
							/>
							{targetKind === "macro" ? (
								<SelectField
									label="Macro"
									value={macro}
									options={[
										{ value: "101 · Venue open", label: "101 · Venue open" },
										{
											value: "102 · Venue close",
											label: "102 · Venue close",
										},
										{
											value: "204 · Festival day",
											label: "204 · Festival day",
										},
									]}
									onChange={setMacro}
								/>
							) : (
								<PlaybackPickerField
									page={page}
									playback={playback}
									onChange={(nextPage, nextPlayback) => {
										setPage(nextPage);
										setPlayback(nextPlayback);
									}}
								/>
							)}
						</div>
						{targetKind === "playback" && (
							<>
								<FormLayout columns={1}>
									<SelectField
										label="Button action"
										value={action}
										options={[
											{ value: "Go", label: "Go" },
											{ value: "Pause", label: "Pause" },
											{ value: "On", label: "On" },
											{ value: "Off", label: "Off" },
											{ value: "Release", label: "Release" },
											{ value: "Toggle", label: "Toggle" },
										]}
										onChange={setAction}
									/>
								</FormLayout>
								<SwitchField
									label="Set playback master"
									description="Move the master when the scheduled action starts."
									checked={shouldSetMaster}
									onChange={(event) => setShouldSetMaster(event.target.checked)}
								/>
								{shouldSetMaster && (
									<FormLayout columns={2}>
										<NumberField
											label="Master"
											min={0}
											max={100}
											unit="%"
											value={master}
											onChange={(event) =>
												setMasterValue(Number(event.target.value) || 0)
											}
										/>
										<NumberField
											label="Fade in"
											min={0}
											unit="s"
											value={fade}
											onChange={(event) =>
												setFade(Number(event.target.value) || 0)
											}
										/>
									</FormLayout>
								)}
							</>
						)}
					</section>
				)}
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
