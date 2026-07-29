import {
	Button,
	FormField,
	FormLayout,
	ModalLayer,
	ModalTitleBar,
	NumberField,
	SelectField,
	SwitchField,
	TextField,
} from "@tosklight/ui";
import { useState } from "react";

export type TimingKind = "fixed" | "recurring";
export type Frequency =
	| "selected_days"
	| "interval_minutes"
	| "interval_days"
	| "monthly"
	| "custom";
export type TargetKind = "macro" | "playback";

export interface ScheduleEditorForm {
	name: string;
	enabled: boolean;
	timingKind: TimingKind;
	date: string;
	time: string;
	frequency: Frequency;
	selectedDays: string[];
	intervalMinutes: number;
	intervalDays: number;
	monthlyOrdinal: string;
	monthlyWeekday: string;
	cron: string;
	targetKind: TargetKind;
	macro: string;
	page: number;
	playback: number;
	action: string;
	setMaster: boolean;
	master: number;
	fade: number;
}

type UpdateForm = <Key extends keyof ScheduleEditorForm>(
	key: Key,
	value: ScheduleEditorForm[Key],
) => void;

export const weekdayOptions = [
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

export function ScheduleEditorTabs({
	activeTab,
	form,
	summary,
	resolvedCron,
	update,
}: {
	activeTab: string;
	form: ScheduleEditorForm;
	summary: string;
	resolvedCron: string;
	update: UpdateForm;
}) {
	if (activeTab === "schedule") return <NameTab form={form} update={update} />;
	if (activeTab === "when")
		return (
			<WhenTab
				form={form}
				summary={summary}
				resolvedCron={resolvedCron}
				update={update}
			/>
		);
	if (activeTab === "trigger")
		return <TriggerTab form={form} update={update} />;
	return null;
}

function NameTab({
	form,
	update,
}: {
	form: ScheduleEditorForm;
	update: UpdateForm;
}) {
	return (
		<section className="scheduler-editor-section">
			<FormLayout columns={2}>
				<TextField
					label="Name"
					value={form.name}
					onChange={(event) => update("name", event.target.value)}
				/>
				<SwitchField
					label="Enabled"
					offLabel="Disabled"
					onLabel="Enabled"
					checked={form.enabled}
					onChange={(event) => update("enabled", event.target.checked)}
				/>
			</FormLayout>
		</section>
	);
}

function WhenTab({
	form,
	summary,
	resolvedCron,
	update,
}: {
	form: ScheduleEditorForm;
	summary: string;
	resolvedCron: string;
	update: UpdateForm;
}) {
	return (
		<section className="scheduler-editor-section">
			<div
				className={
					form.timingKind === "fixed"
						? "scheduler-fixed-timing-row"
						: "scheduler-when-repeat-row"
				}
			>
				<SelectField
					label="When"
					value={form.timingKind}
					options={[
						{ value: "fixed", label: "Fixed date" },
						{ value: "recurring", label: "Repeating rule" },
					]}
					onChange={(value) => update("timingKind", value as TimingKind)}
				/>
				{form.timingKind === "fixed" ? (
					<FixedTimingFields form={form} update={update} />
				) : (
					<SelectField
						className="scheduler-repeat-control"
						label="Repeat"
						value={form.frequency}
						options={[
							{
								value: "selected_days",
								label: "On selected days of the week",
							},
							{ value: "interval_minutes", label: "Every <n> minutes" },
							{ value: "interval_days", label: "Every <n> days" },
							{ value: "monthly", label: "On a weekday of the month" },
							{ value: "custom", label: "Advanced / custom cron" },
						]}
						onChange={(value) => update("frequency", value as Frequency)}
					/>
				)}
			</div>
			{form.timingKind === "recurring" && (
				<RecurringTiming
					form={form}
					summary={summary}
					resolvedCron={resolvedCron}
					update={update}
				/>
			)}
		</section>
	);
}

function FixedTimingFields({
	form,
	update,
}: {
	form: ScheduleEditorForm;
	update: UpdateForm;
}) {
	return (
		<>
			<TextField
				label="Date"
				value={form.date}
				description="YYYY-MM-DD"
				onChange={(event) => update("date", event.target.value)}
			/>
			<div className="scheduler-compact-time-field">
				<TextField
					label="Time"
					value={form.time}
					description="24-hour time"
					onChange={(event) => update("time", event.target.value)}
				/>
			</div>
		</>
	);
}

function RecurringTiming({
	form,
	summary,
	resolvedCron,
	update,
}: {
	form: ScheduleEditorForm;
	summary: string;
	resolvedCron: string;
	update: UpdateForm;
}) {
	if (form.frequency === "custom")
		return <CustomRule cron={form.cron} update={update} />;
	return (
		<RuleBuilder
			form={form}
			summary={summary}
			resolvedCron={resolvedCron}
			update={update}
		/>
	);
}

function CustomRule({ cron, update }: { cron: string; update: UpdateForm }) {
	return (
		<div className="scheduler-custom-rule">
			<TextField
				label="Cron expression"
				value={cron}
				description="minute · hour · day · month · weekday"
				onChange={(event) => update("cron", event.target.value)}
			/>
			<aside aria-label="Next five occurrences">
				<strong>Next 5 occurrences</strong>
				<ol>
					{[
						"Friday, 31 July 2026",
						"Friday, 7 August 2026",
						"Friday, 14 August 2026",
						"Friday, 21 August 2026",
						"Friday, 28 August 2026",
					].map((occurrence) => (
						<li key={occurrence}>
							<span>{occurrence}</span>
							<time>19:00</time>
						</li>
					))}
				</ol>
			</aside>
		</div>
	);
}

function RuleBuilder({
	form,
	summary,
	resolvedCron,
	update,
}: {
	form: ScheduleEditorForm;
	summary: string;
	resolvedCron: string;
	update: UpdateForm;
}) {
	return (
		<div className="scheduler-rule-builder">
			{form.frequency === "selected_days" && (
				<WeekdayPicker selected={form.selectedDays} update={update} />
			)}
			{form.frequency === "interval_days" && (
				<NumberField
					label="Interval"
					min={1}
					unit="days"
					value={form.intervalDays}
					onChange={(event) =>
						update("intervalDays", Number(event.target.value) || 1)
					}
				/>
			)}
			{form.frequency === "interval_minutes" && (
				<NumberField
					label="Interval"
					min={1}
					max={59}
					unit="minutes"
					value={form.intervalMinutes}
					onChange={(event) =>
						update("intervalMinutes", Number(event.target.value) || 1)
					}
				/>
			)}
			{form.frequency === "monthly" && (
				<MonthlyFields form={form} update={update} />
			)}
			{form.frequency !== "interval_minutes" && (
				<div className="scheduler-compact-time-field">
					<TextField
						label="Time"
						value={form.time}
						description="24-hour time"
						onChange={(event) => update("time", event.target.value)}
					/>
				</div>
			)}
			<div className="scheduler-rule-summary">
				<small>This schedule will run</small>
				<strong>{summary}</strong>
				<code>{resolvedCron}</code>
			</div>
		</div>
	);
}

function WeekdayPicker({
	selected,
	update,
}: {
	selected: string[];
	update: UpdateForm;
}) {
	return (
		<FormField
			label="Days"
			description="Choose one or more days."
			className="scheduler-weekday-field"
		>
			<fieldset className="scheduler-weekdays">
				<legend>Days of the week</legend>
				{weekdayOptions.map((day) => {
					const active = selected.includes(day.value);
					return (
						<Button
							key={day.value}
							active={active}
							aria-pressed={active}
							onClick={() =>
								update(
									"selectedDays",
									active
										? selected.length > 1
											? selected.filter((value) => value !== day.value)
											: selected
										: [...selected, day.value],
								)
							}
						>
							{day.short}
						</Button>
					);
				})}
			</fieldset>
		</FormField>
	);
}

function MonthlyFields({
	form,
	update,
}: {
	form: ScheduleEditorForm;
	update: UpdateForm;
}) {
	return (
		<>
			<SelectField
				label="Occurrence"
				value={form.monthlyOrdinal}
				options={[
					{ value: "first", label: "First" },
					{ value: "second", label: "Second" },
					{ value: "third", label: "Third" },
					{ value: "fourth", label: "Fourth" },
					{ value: "last", label: "Last" },
				]}
				onChange={(value) => update("monthlyOrdinal", value)}
			/>
			<SelectField
				label="Weekday"
				value={form.monthlyWeekday}
				options={weekdayOptions.map((day) => ({
					value: day.value,
					label: day.label,
				}))}
				onChange={(value) => update("monthlyWeekday", value)}
			/>
		</>
	);
}

function TriggerTab({
	form,
	update,
}: {
	form: ScheduleEditorForm;
	update: UpdateForm;
}) {
	return (
		<section className="scheduler-editor-section">
			<div className="scheduler-trigger-target-row">
				<SelectField
					label="Trigger"
					value={form.targetKind}
					options={[
						{ value: "macro", label: "Macro" },
						{ value: "playback", label: "Playback" },
					]}
					onChange={(value) => update("targetKind", value as TargetKind)}
				/>
				{form.targetKind === "macro" ? (
					<SelectField
						label="Macro"
						value={form.macro}
						options={[
							{ value: "101 · Venue open", label: "101 · Venue open" },
							{ value: "102 · Venue close", label: "102 · Venue close" },
							{ value: "204 · Festival day", label: "204 · Festival day" },
						]}
						onChange={(value) => update("macro", value)}
					/>
				) : (
					<PlaybackPickerField
						page={form.page}
						playback={form.playback}
						onChange={(page, playback) => {
							update("page", page);
							update("playback", playback);
						}}
					/>
				)}
			</div>
			{form.targetKind === "playback" && (
				<PlaybackActionFields form={form} update={update} />
			)}
		</section>
	);
}

function PlaybackActionFields({
	form,
	update,
}: {
	form: ScheduleEditorForm;
	update: UpdateForm;
}) {
	return (
		<>
			<FormLayout columns={1}>
				<SelectField
					label="Button action"
					value={form.action}
					options={["Go", "Pause", "On", "Off", "Release", "Toggle"].map(
						(value) => ({ value, label: value }),
					)}
					onChange={(value) => update("action", value)}
				/>
			</FormLayout>
			<SwitchField
				label="Set playback master"
				offLabel="Leave unchanged"
				onLabel="Set master"
				description="Move the master when the scheduled action starts."
				checked={form.setMaster}
				onChange={(event) => update("setMaster", event.target.checked)}
			/>
			{form.setMaster && (
				<FormLayout columns={2}>
					<NumberField
						label="Master"
						min={0}
						max={100}
						unit="%"
						value={form.master}
						onChange={(event) =>
							update("master", Number(event.target.value) || 0)
						}
					/>
					<NumberField
						label="Fade in"
						min={0}
						unit="s"
						value={form.fade}
						onChange={(event) =>
							update("fade", Number(event.target.value) || 0)
						}
					/>
				</FormLayout>
			)}
		</>
	);
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
				<PlaybackPickerModal
					page={page}
					playback={playback}
					pickerPage={pickerPage}
					definition={pickerPageDefinition}
					onPage={setPickerPage}
					onPick={(nextPlayback) => {
						onChange(Number(pickerPage), nextPlayback);
						setOpen(false);
					}}
					onClose={() => setOpen(false)}
				/>
			)}
		</FormField>
	);
}

function PlaybackPickerModal({
	page,
	playback,
	pickerPage,
	definition,
	onPage,
	onPick,
	onClose,
}: {
	page: number;
	playback: number;
	pickerPage: string;
	definition: PlaybackPageOption;
	onPage: (page: string) => void;
	onPick: (playback: number) => void;
	onClose: () => void;
}) {
	return (
		<ModalLayer
			ariaLabel="Choose playback"
			className="scheduler-playback-picker-layer"
			dialogClassName="scheduler-playback-picker"
			onClose={onClose}
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
						onChange={onPage}
					/>
				}
				closeLabel="Close playback picker"
				onClose={onClose}
			/>
			<div className="scheduler-playback-groups">
				{definition.groups.map((group) => (
					<section key={group.label}>
						<h3>{group.label}</h3>
						<div className="scheduler-playback-grid">
							{group.playbacks.map((candidate) => (
								<Button
									key={candidate.number}
									active={
										Number(pickerPage) === page && candidate.number === playback
									}
									contentAlign="left"
									onClick={() => onPick(candidate.number)}
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
	);
}
