/**
 * Future Scheduler feature prototype.
 *
 * This window is intentionally Storybook-first and is not wired into the live
 * desktop yet. Keep it during dead-code reviews: it is the interaction and
 * component contract for the planned Scheduler production feature.
 */
import {
	Button,
	Calendar,
	SwitchField,
	WindowHeader,
	WindowScrollArea,
	WindowSettings,
} from "@tosklight/ui";
import { useMemo, useState } from "react";
import "./SchedulerWindow.css";

export type ScheduleTrigger =
	| { type: "macro"; macro: string }
	| {
			type: "playback";
			page: number;
			playback: number;
			action: string;
			master?: number;
			fadeSeconds?: number;
	  };

export interface ScheduleEvent {
	id: string;
	name: string;
	enabled: boolean;
	timing:
		| { type: "fixed"; date: string; time: string }
		| {
				type: "recurring";
				summary: string;
				time: string;
				cron: string;
				occurrences: string[];
		  };
	trigger: ScheduleTrigger;
}

export interface SchedulerWindowProps {
	initialEvents: ScheduleEvent[];
	onCreate?: () => void;
	onEdit?: (event: ScheduleEvent) => void;
}

function triggerLabel(trigger: ScheduleTrigger) {
	if (trigger.type === "macro") return `Macro · ${trigger.macro}`;
	return `Page ${trigger.page} · Playback ${trigger.playback} · ${trigger.action}`;
}

function EventCard({
	event,
	onSelect,
}: {
	event: ScheduleEvent;
	onSelect: () => void;
}) {
	return (
		<Button
			className="scheduler-event-card"
			contentAlign="left"
			onClick={onSelect}
		>
			<span className="scheduler-event-time">
				{event.timing.time}
				<i className={event.timing.type} aria-hidden="true" />
			</span>
			<span className="scheduler-event-copy">
				<strong>{event.name}</strong>
				<small>
					{event.timing.type === "fixed"
						? event.timing.date
						: event.timing.summary}
				</small>
				<small>{triggerLabel(event.trigger)}</small>
			</span>
			<span
				className={`scheduler-event-status ${event.enabled ? "" : "is-off"}`}
			>
				{event.enabled ? "On" : "Off"}
			</span>
		</Button>
	);
}

export function SchedulerWindow({
	initialEvents,
	onCreate,
	onEdit,
}: SchedulerWindowProps) {
	const [view, setView] = useState<"month" | "year">("month");
	const [year, setYear] = useState(2026);
	const [month, setMonth] = useState(6);
	const [selectedDate, setSelectedDate] = useState<string | null>(null);
	const [showList, setShowList] = useState(true);
	const [showCalendar, setShowCalendar] = useState(true);
	const [settingsOpen, setSettingsOpen] = useState(false);
	const markers = useMemo(() => {
		const byDate = new Map<
			string,
			{ date: string; explicit?: number; recurring?: number }
		>();
		for (const event of initialEvents) {
			const dates =
				event.timing.type === "fixed"
					? [event.timing.date]
					: event.timing.occurrences;
			for (const date of dates) {
				const current = byDate.get(date) ?? { date };
				if (event.timing.type === "fixed")
					current.explicit = (current.explicit ?? 0) + 1;
				else current.recurring = (current.recurring ?? 0) + 1;
				byDate.set(date, current);
			}
		}
		return [...byDate.values()];
	}, [initialEvents]);
	const visibleEvents = selectedDate
		? initialEvents.filter((event) =>
				event.timing.type === "fixed"
					? event.timing.date === selectedDate
					: event.timing.occurrences.includes(selectedDate),
			)
		: initialEvents;
	const changePeriod = (direction: -1 | 1) => {
		setSelectedDate(null);
		if (view === "year") {
			setYear((value) => value + direction);
			return;
		}
		const next = new Date(year, month + direction, 1);
		setYear(next.getFullYear());
		setMonth(next.getMonth());
	};
	return (
		<section className="scheduler-window">
			<WindowHeader
				title="Scheduler"
				info={{
					primary: selectedDate
						? `${visibleEvents.length} schedules on ${selectedDate}`
						: `${initialEvents.length} schedules`,
					secondary: (
						<span className="scheduler-inline-legend">
							<span>
								<i className="fixed" /> Fixed date
							</span>
							<span>
								<i className="recurring" /> Repeating rule
							</span>
							{selectedDate && <b>Day filter active</b>}
						</span>
					),
				}}
				actions={[
					[
						{
							id: "create",
							label: "+ Schedule",
							variant: "primary",
							className: "scheduler-create-title-action",
							onClick: () => onCreate?.(),
						},
					],
					[
						{
							id: "previous",
							label: "‹",
							ariaLabel: "Previous period",
							onClick: () => changePeriod(-1),
						},
					],
					[
						{
							id: "today",
							label: "Today",
							onClick: () => {
								setYear(2026);
								setMonth(6);
								setSelectedDate(null);
							},
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
					[
						{
							id: "next",
							label: "›",
							ariaLabel: "Next period",
							onClick: () => changePeriod(1),
						},
					],
				]}
				settings
				onSettings={() => setSettingsOpen(true)}
			/>
			<div
				className={`scheduler-layout ${showList && showCalendar ? "is-split" : ""}`}
			>
				{showList && (
					<aside className="scheduler-list">
						{selectedDate && (
							<header className="scheduler-list-heading">
								<span>
									<strong>{selectedDate}</strong>
									<small>Fixed and repeating events occurring this day</small>
								</span>
								<Button size="compact" onClick={() => setSelectedDate(null)}>
									Show all
								</Button>
							</header>
						)}
						<WindowScrollArea
							emptyState={
								visibleEvents.length
									? null
									: {
											title: "No events on this day",
											description:
												"Choose another day or clear the day filter.",
										}
							}
						>
							<div className="scheduler-event-list">
								{visibleEvents.map((event) => (
									<EventCard
										event={event}
										key={event.id}
										onSelect={() => onEdit?.(event)}
									/>
								))}
							</div>
						</WindowScrollArea>
					</aside>
				)}
				{showCalendar && (
					<main className="scheduler-calendar">
						<div className="scheduler-calendar-body">
							<Calendar
								view={view}
								year={year}
								month={month}
								markers={markers}
								selectedDate={selectedDate}
								onDaySelect={setSelectedDate}
								onMonthSelect={(nextMonth) => {
									setMonth(nextMonth);
									setView("month");
								}}
							/>
						</div>
					</main>
				)}
			</div>
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
									<p>
										Choose which sides are visible in this Scheduler window.
									</p>
									<SwitchField
										label="Schedule list"
										checked={showList}
										disabled={showList && !showCalendar}
										onChange={(event) => setShowList(event.target.checked)}
									/>
									<SwitchField
										label="Calendar"
										checked={showCalendar}
										disabled={showCalendar && !showList}
										onChange={(event) => setShowCalendar(event.target.checked)}
									/>
								</div>
							),
						},
					]}
				/>
			)}
		</section>
	);
}
