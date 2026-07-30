import {
	Button,
	Calendar,
	ModalFrame,
	SwitchField,
	WindowHeader,
	WindowScrollArea,
	WindowSettings,
} from "@tosklight/ui";
import { useEffect, useMemo, useState } from "react";
import type {
	ScheduleOccurrence,
	SchedulePreview,
	ScheduleProjection,
	SchedulerController,
} from "../features/scheduler/contracts";
import {
	useSchedulerController,
	useSchedulerView,
} from "../features/scheduler/SchedulerContext";
import {
	draftFromScheduleEditor,
	playbackActionLabel,
	type ScheduleEditorTab,
	ScheduleEditorTabs,
	scheduleEditorForm,
} from "./schedulerStory/ScheduleEditorTabs";
import type { WindowProps } from "./windowTypes";
import "./SchedulerWindow.css";

export interface SchedulerWindowProps extends WindowProps {
	controller?: SchedulerController;
	schedulerShowList?: boolean;
	schedulerShowCalendar?: boolean;
	onSchedulerLayoutChange?: (layout: {
		showList: boolean;
		showCalendar: boolean;
	}) => void;
}

function timingLabel(schedule: ScheduleProjection) {
	const timing = schedule.definition.timing;
	if (timing.type === "interval")
		return `Interval · every ${timing.everySeconds} seconds`;
	if (timing.type === "calendar_expression")
		return `Calendar expression · ${timing.summary || timing.expression}`;
	return `One-time · ${timing.localDate} ${timing.localTime}`;
}

function targetLabel(schedule: ScheduleProjection) {
	const target = schedule.definition.target;
	if (target.type === "macro") return `Start Macro · ${target.label}`;
	return `Page ${target.page} · Playback ${target.playback} · ${playbackActionLabel(target.action)}`;
}

function resultLabel(schedule: ScheduleProjection) {
	if (schedule.validationMessage)
		return { tone: "invalid", text: schedule.validationMessage };
	if (!schedule.lastResult)
		return { tone: "quiet", text: "No occurrences yet" };
	return {
		tone: schedule.lastResult.status,
		text: `${capitalize(schedule.lastResult.status)} · ${schedule.lastResult.message}`,
	};
}

function ScheduleCard({
	schedule,
	canWrite,
	onEdit,
	onEnabled,
	onDuplicate,
	onDelete,
}: {
	schedule: ScheduleProjection;
	canWrite: boolean;
	onEdit(): void;
	onEnabled(enabled: boolean): void;
	onDuplicate(): void;
	onDelete(): void;
}) {
	const next = schedule.nextOccurrence;
	const result = resultLabel(schedule);
	return (
		<article
			className={`scheduler-event-card ${schedule.pending ? "is-pending" : ""}`}
			aria-busy={schedule.pending || undefined}
		>
			<Button
				className="scheduler-event-main"
				contentAlign="left"
				onClick={onEdit}
			>
				<span className="scheduler-event-time">
					{next?.localTime ?? "—"}
					<i className={schedule.definition.timing.type} aria-hidden="true" />
				</span>
				<span className="scheduler-event-copy">
					<strong>{schedule.definition.name}</strong>
					<small>{timingLabel(schedule)}</small>
					<small>{targetLabel(schedule)}</small>
					<small className={`scheduler-result is-${result.tone}`}>
						{result.text}
					</small>
				</span>
				<span
					className={`scheduler-event-status ${schedule.definition.enabled ? "" : "is-off"}`}
				>
					{schedule.definition.enabled ? "Enabled" : "Disabled"}
				</span>
			</Button>
			<div
				className="scheduler-event-actions"
				role="toolbar"
				aria-label="Schedule actions"
			>
				<Button
					size="compact"
					disabled={!canWrite || schedule.pending}
					onClick={() => onEnabled(!schedule.definition.enabled)}
				>
					{schedule.definition.enabled ? "Disable" : "Enable"}
				</Button>
				<Button
					size="compact"
					disabled={!canWrite || schedule.pending}
					onClick={onDuplicate}
				>
					Duplicate
				</Button>
				<Button
					size="compact"
					disabled={!canWrite || schedule.pending}
					onClick={onDelete}
				>
					Delete
				</Button>
			</div>
		</article>
	);
}

function markers(schedules: readonly ScheduleProjection[]) {
	const byDate = new Map<
		string,
		{ date: string; explicit?: number; recurring?: number }
	>();
	for (const schedule of schedules)
		for (const occurrence of schedule.upcomingOccurrences) {
			const marker = byDate.get(occurrence.localDate) ?? {
				date: occurrence.localDate,
			};
			if (schedule.definition.timing.type === "one_time")
				marker.explicit = (marker.explicit ?? 0) + 1;
			else marker.recurring = (marker.recurring ?? 0) + 1;
			byDate.set(occurrence.localDate, marker);
		}
	return [...byDate.values()];
}

function occursOn(schedule: ScheduleProjection, date: string) {
	return schedule.upcomingOccurrences.some(
		(occurrence) => occurrence.localDate === date,
	);
}

function serverDateParts(serverDate: string) {
	const match = /^(\d{4})-(\d{2})-(\d{2})$/u.exec(serverDate);
	if (!match) return { year: 1970, month: 0 };
	return { year: Number(match[1]), month: Number(match[2]) - 1 };
}

function shiftPeriod(
	year: number,
	month: number,
	direction: -1 | 1,
	view: "month" | "year",
) {
	if (view === "year") return { year: year + direction, month };
	const next = month + direction;
	if (next < 0) return { year: year - 1, month: 11 };
	if (next > 11) return { year: year + 1, month: 0 };
	return { year, month: next };
}

export function SchedulerWindow({
	active = true,
	controller: providedController,
	schedulerShowList = true,
	schedulerShowCalendar = true,
	onSchedulerLayoutChange,
}: SchedulerWindowProps) {
	const contextController = useSchedulerController();
	const controller = providedController ?? contextController;
	useSchedulerView(controller, active);
	const snapshot = controller?.snapshot ?? unavailableSchedulerSnapshot();
	const today = serverDateParts(snapshot.serverDate);
	const [view, setView] = useState<"month" | "year">("month");
	const [year, setYear] = useState(today.year);
	const [month, setMonth] = useState(today.month);
	const [selectedDate, setSelectedDate] = useState<string | null>(null);
	const [localShowList, setLocalShowList] = useState(schedulerShowList);
	const [localShowCalendar, setLocalShowCalendar] = useState(
		schedulerShowCalendar,
	);
	const [settingsOpen, setSettingsOpen] = useState(false);
	const [editing, setEditing] = useState<ScheduleProjection | "new" | null>(
		null,
	);
	const [mutationError, setMutationError] = useState<string | null>(null);
	useEffect(() => {
		setLocalShowList(schedulerShowList);
	}, [schedulerShowList]);
	useEffect(() => {
		setLocalShowCalendar(schedulerShowCalendar);
	}, [schedulerShowCalendar]);
	const updateLayout = (showList: boolean, showCalendar: boolean) => {
		setLocalShowList(showList);
		setLocalShowCalendar(showCalendar);
		onSchedulerLayoutChange?.({ showList, showCalendar });
	};
	const visible = selectedDate
		? snapshot.schedules.filter((schedule) => occursOn(schedule, selectedDate))
		: snapshot.schedules;
	const period = (direction: -1 | 1) => {
		const next = shiftPeriod(year, month, direction, view);
		setYear(next.year);
		setMonth(next.month);
		setSelectedDate(null);
	};
	const mutate = async (operation: () => Promise<boolean>) => {
		setMutationError(null);
		if (!(await operation()))
			setMutationError(
				"The server rejected the Schedule change. Refresh and retry.",
			);
	};
	return (
		<section className="scheduler-window">
			<SchedulerHeader
				controller={controller}
				onCreate={() => setEditing("new")}
				onPeriod={period}
				onSettings={() => setSettingsOpen(true)}
				onToday={() => {
					setYear(today.year);
					setMonth(today.month);
					setSelectedDate(snapshot.serverDate || null);
				}}
				selectedDate={selectedDate}
				setView={setView}
				snapshot={snapshot}
				view={view}
			/>
			{snapshot.status === "loading" ? (
				<div className="scheduler-message" role="status">
					Loading Schedules…
				</div>
			) : snapshot.status === "error" ? (
				<div className="scheduler-message is-error" role="alert">
					<strong>Schedules could not be loaded</strong>
					<span>{snapshot.error}</span>
					{controller?.retry && (
						<Button onClick={() => void controller.retry?.()}>Retry</Button>
					)}
				</div>
			) : (
				<SchedulerReadyContent
					controller={controller}
					localShowCalendar={localShowCalendar}
					localShowList={localShowList}
					month={month}
					mutate={mutate}
					mutationError={mutationError}
					selectedDate={selectedDate}
					setEditing={setEditing}
					setMonth={setMonth}
					setSelectedDate={setSelectedDate}
					setView={setView}
					snapshot={snapshot}
					view={view}
					visible={visible}
					year={year}
				/>
			)}
			{settingsOpen && (
				<WindowSettings
					title="Scheduler settings"
					onClose={() => setSettingsOpen(false)}
					tabs={[
						{
							id: "layout",
							label: "Layout",
							content: (
								<div className="scheduler-settings">
									<SwitchField
										label="Schedule list"
										offLabel="Hidden"
										onLabel="Visible"
										checked={localShowList}
										disabled={localShowList && !localShowCalendar}
										onChange={(event) =>
											updateLayout(event.target.checked, localShowCalendar)
										}
									/>
									<SwitchField
										label="Calendar"
										offLabel="Hidden"
										onLabel="Visible"
										checked={localShowCalendar}
										disabled={localShowCalendar && !localShowList}
										onChange={(event) =>
											updateLayout(localShowList, event.target.checked)
										}
									/>
								</div>
							),
						},
					]}
				/>
			)}
			{editing && controller && (
				<ScheduleEditor
					controller={controller}
					schedule={editing === "new" ? null : editing}
					onClose={() => setEditing(null)}
				/>
			)}
		</section>
	);
}

function unavailableSchedulerSnapshot(): SchedulerController["snapshot"] {
	return {
		status: "error",
		timezone: "",
		serverDate: "",
		schedules: [],
		playbackTargets: [],
		canWrite: false,
		error: "Scheduler runtime is not connected.",
	};
}

function SchedulerHeader({
	controller,
	onCreate,
	onPeriod,
	onSettings,
	onToday,
	selectedDate,
	setView,
	snapshot,
	view,
}: {
	controller: SchedulerController | null | undefined;
	onCreate(): void;
	onPeriod(direction: -1 | 1): void;
	onSettings(): void;
	onToday(): void;
	selectedDate: string | null;
	setView(view: "month" | "year"): void;
	snapshot: SchedulerController["snapshot"];
	view: "month" | "year";
}) {
	return (
		<WindowHeader
			title="Scheduler"
			info={{
				primary:
					snapshot.status === "ready"
						? `${snapshot.schedules.length} schedules · ${snapshot.timezone}`
						: "Schedule authority unavailable",
				secondary: selectedDate ? `Day filter · ${selectedDate}` : undefined,
			}}
			actions={[
				[
					{
						id: "create",
						label: "+ Schedule",
						variant: "primary",
						disabled:
							!controller || !snapshot.canWrite || snapshot.status !== "ready",
						onClick: onCreate,
					},
				],
				[
					{
						id: "previous",
						label: "‹",
						ariaLabel: "Previous period",
						onClick: () => onPeriod(-1),
					},
					{
						id: "today",
						label: "Today",
						disabled: !snapshot.serverDate,
						onClick: onToday,
					},
					{
						id: "next",
						label: "›",
						ariaLabel: "Next period",
						onClick: () => onPeriod(1),
					},
				],
				[
					{
						id: "month",
						label: "Month",
						active: view === "month",
						onClick: () => setView("month"),
					},
					{
						id: "year",
						label: "Year",
						active: view === "year",
						onClick: () => setView("year"),
					},
				],
			]}
			settings
			onSettings={onSettings}
		/>
	);
}

function SchedulerReadyContent({
	controller,
	localShowCalendar,
	localShowList,
	month,
	mutate,
	mutationError,
	selectedDate,
	setEditing,
	setMonth,
	setSelectedDate,
	setView,
	snapshot,
	view,
	visible,
	year,
}: {
	controller: SchedulerController | null | undefined;
	localShowCalendar: boolean;
	localShowList: boolean;
	month: number;
	mutate(operation: () => Promise<boolean>): Promise<void>;
	mutationError: string | null;
	selectedDate: string | null;
	setEditing(value: ScheduleProjection | "new" | null): void;
	setMonth(value: number): void;
	setSelectedDate(value: string | null): void;
	setView(value: "month" | "year"): void;
	snapshot: SchedulerController["snapshot"];
	view: "month" | "year";
	visible: readonly ScheduleProjection[];
	year: number;
}) {
	return (
		<>
			{mutationError && (
				<p className="scheduler-mutation-error" role="alert">
					{mutationError}
				</p>
			)}
			<div
				className={`scheduler-layout ${localShowList && localShowCalendar ? "is-split" : ""}`}
			>
				{localShowList && (
					<aside className="scheduler-list">
						{selectedDate && (
							<header className="scheduler-list-heading">
								<strong>{selectedDate}</strong>
								<Button size="compact" onClick={() => setSelectedDate(null)}>
									Show all
								</Button>
							</header>
						)}
						<WindowScrollArea
							emptyState={
								visible.length
									? null
									: selectedDate
										? {
												title: "No Schedules on this day",
												description:
													"Choose another day or clear the day filter.",
											}
										: {
												title: "No Schedules",
												description:
													"Create an Interval, Calendar expression, or One-time Schedule.",
											}
							}
						>
							<div className="scheduler-event-list">
								{visible.map((schedule) => (
									<ScheduleCard
										key={schedule.definition.id}
										schedule={schedule}
										canWrite={snapshot.canWrite}
										onEdit={() => setEditing(schedule)}
										onEnabled={(enabled) => {
											if (!controller) return;
											void mutate(() =>
												controller.setEnabled(
													schedule.definition.id,
													schedule.definition.revision,
													enabled,
												),
											);
										}}
										onDuplicate={() => {
											if (!controller) return;
											void mutate(() =>
												controller.duplicate(
													schedule.definition.id,
													schedule.definition.revision,
												),
											);
										}}
										onDelete={() => {
											if (
												!controller ||
												!globalThis.confirm(
													`Delete Schedule “${schedule.definition.name}”?`,
												)
											)
												return;
											void mutate(() =>
												controller.delete(
													schedule.definition.id,
													schedule.definition.revision,
												),
											);
										}}
									/>
								))}
							</div>
						</WindowScrollArea>
					</aside>
				)}
				{localShowCalendar && (
					<main className="scheduler-calendar">
						<Calendar
							view={view}
							year={year}
							month={month}
							markers={markers(snapshot.schedules)}
							selectedDate={selectedDate}
							onDaySelect={setSelectedDate}
							onMonthSelect={(nextMonth) => {
								setMonth(nextMonth);
								setView("month");
							}}
						/>
					</main>
				)}
			</div>
		</>
	);
}

function ScheduleEditor({
	controller,
	schedule,
	onClose,
}: {
	controller: SchedulerController;
	schedule: ScheduleProjection | null;
	onClose(): void;
}) {
	const { snapshot } = controller;
	const sourceDraft = schedule
		? {
				name: schedule.definition.name,
				enabled: schedule.definition.enabled,
				timing: schedule.definition.timing,
				target: schedule.definition.target,
			}
		: null;
	const [tab, setTab] = useState<ScheduleEditorTab>("name");
	const [form, setForm] = useState(() =>
		scheduleEditorForm(
			sourceDraft,
			snapshot.playbackTargets,
			snapshot.serverDate,
		),
	);
	const [preview, setPreview] = useState<
		| { status: "loading"; result: null }
		| { status: "ready"; result: SchedulePreview }
		| { status: "error"; result: null }
	>({ status: "loading", result: null });
	const [saving, setSaving] = useState(false);
	const [saveError, setSaveError] = useState<string | null>(null);
	const draft = useMemo(
		() => draftFromScheduleEditor(form, snapshot.playbackTargets),
		[form, snapshot.playbackTargets],
	);
	useEffect(() => {
		if (!draft) {
			setPreview({
				status: "ready",
				result: {
					status: "invalid",
					occurrences: [],
					message: "Choose a valid Playback target.",
				},
			});
			return;
		}
		const abort = new AbortController();
		setPreview({ status: "loading", result: null });
		const timer = globalThis.setTimeout(() => {
			void controller
				.preview(draft, abort.signal)
				.then((result) => {
					if (!abort.signal.aborted) setPreview({ status: "ready", result });
				})
				.catch(() => {
					if (!abort.signal.aborted)
						setPreview({ status: "error", result: null });
				});
		}, 180);
		return () => {
			globalThis.clearTimeout(timer);
			abort.abort();
		};
	}, [controller, draft]);
	const close = () => {
		const initial = scheduleEditorForm(
			sourceDraft,
			snapshot.playbackTargets,
			snapshot.serverDate,
		);
		if (
			JSON.stringify(initial) !== JSON.stringify(form) &&
			!globalThis.confirm("Discard unsaved Schedule changes?")
		)
			return;
		onClose();
	};
	const save = async () => {
		if (!draft) return;
		setSaving(true);
		setSaveError(null);
		const accepted = schedule
			? await controller.update(
					schedule.definition.id,
					schedule.definition.revision,
					draft,
				)
			: await controller.create(draft);
		setSaving(false);
		if (accepted) onClose();
		else
			setSaveError(
				"The server rejected this Schedule. Review the fields and retry.",
			);
	};
	const previewInvalid =
		preview.status !== "ready" ||
		preview.result.status === "invalid" ||
		!draft?.name;
	return (
		<ModalFrame
			id="scheduler-editor"
			ariaLabel={
				schedule ? `Edit ${schedule.definition.name}` : "Create Schedule"
			}
			dialogClassName="scheduler-editor-modal"
			title={schedule ? "Edit Schedule" : "New Schedule"}
			details={`Times and previews use ${snapshot.timezone || "the server timezone"}`}
			tabs={[
				{ id: "name", label: "Name" },
				{ id: "when", label: "When" },
				{ id: "action", label: "Action" },
			]}
			activeTab={tab}
			onTabChange={(value) => setTab(value as ScheduleEditorTab)}
			actions={
				<Button
					variant="primary"
					disabled={previewInvalid || saving || !snapshot.canWrite}
					onClick={() => void save()}
				>
					{saving ? "Saving…" : schedule ? "Save changes" : "Create Schedule"}
				</Button>
			}
			onClose={close}
		>
			<div className="scheduler-editor">
				{saveError && (
					<p className="scheduler-mutation-error" role="alert">
						{saveError}
					</p>
				)}
				<ScheduleEditorTabs
					activeTab={tab}
					form={form}
					timezone={snapshot.timezone}
					playbackTargets={snapshot.playbackTargets}
					update={(key, value) =>
						setForm((current) => ({ ...current, [key]: value }))
					}
				/>
				<OccurrencePreview preview={preview} />
			</div>
		</ModalFrame>
	);
}

function OccurrencePreview({
	preview,
}: {
	preview:
		| { status: "loading"; result: null }
		| { status: "ready"; result: SchedulePreview }
		| { status: "error"; result: null };
}) {
	if (preview.status === "loading")
		return (
			<aside className="scheduler-preview" aria-live="polite">
				<strong>Upcoming occurrences</strong>
				<span>Loading authoritative preview…</span>
			</aside>
		);
	if (preview.status === "error")
		return (
			<aside className="scheduler-preview is-error" role="alert">
				<strong>Preview unavailable</strong>
				<span>Check the server connection and retry.</span>
			</aside>
		);
	if (preview.result.status === "invalid")
		return (
			<aside className="scheduler-preview is-error" role="alert">
				<strong>Schedule is invalid</strong>
				<span>{preview.result.message}</span>
			</aside>
		);
	return (
		<aside className="scheduler-preview" aria-live="polite">
			<strong>Upcoming occurrences</strong>
			<ol>
				{preview.result.occurrences.map((occurrence) => (
					<OccurrenceRow key={occurrence.id} occurrence={occurrence} />
				))}
			</ol>
		</aside>
	);
}

function OccurrenceRow({ occurrence }: { occurrence: ScheduleOccurrence }) {
	return (
		<li>
			<time dateTime={occurrence.instant}>
				{occurrence.localDate} · {occurrence.localTime}
			</time>
		</li>
	);
}

function capitalize(value: string) {
	return value.charAt(0).toUpperCase() + value.slice(1);
}
