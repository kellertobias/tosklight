/**
 * Future Scheduler feature prototype.
 * This reusable calendar is exercised by Storybook before Scheduler production
 * wiring exists and must not be removed as unused/dead application code.
 */
import { Button } from "../common";

export type CalendarView = "month" | "year";

export interface CalendarDayMarker {
	date: string;
	explicit?: number;
	recurring?: number;
}

export interface CalendarProps {
	view: CalendarView;
	year: number;
	month?: number;
	markers?: CalendarDayMarker[];
	selectedDate?: string | null;
	onDaySelect?: (date: string) => void;
	onMonthSelect?: (month: number) => void;
	className?: string;
}

const monthNames = [
	"January",
	"February",
	"March",
	"April",
	"May",
	"June",
	"July",
	"August",
	"September",
	"October",
	"November",
	"December",
];
const weekdays = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

function dateKey(year: number, month: number, day: number) {
	return `${year}-${String(month + 1).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
}

function monthCells(year: number, month: number) {
	const firstWeekday = (new Date(year, month, 1).getDay() + 6) % 7;
	const days = new Date(year, month + 1, 0).getDate();
	return [
		...Array.from({ length: firstWeekday }, (_, index) => ({
			key: `padding-${index + 1}`,
			day: null,
		})),
		...Array.from({ length: days }, (_, index) => ({
			key: `day-${index + 1}`,
			day: index + 1,
		})),
	];
}

function MarkerDots({ marker }: { marker?: CalendarDayMarker }) {
	if (!marker?.explicit && !marker?.recurring) return null;
	return (
		<span className="ui-calendar-markers" aria-hidden="true">
			{Boolean(marker.explicit) && <i className="is-explicit" />}
			{Boolean(marker.recurring) && <i className="is-recurring" />}
		</span>
	);
}

function MonthGrid({
	year,
	month,
	markers,
	selectedDate,
	compact,
	onDaySelect,
}: {
	year: number;
	month: number;
	markers: Map<string, CalendarDayMarker>;
	selectedDate?: string | null;
	compact?: boolean;
	onDaySelect?: (date: string) => void;
}) {
	return (
		<div className={`ui-calendar-month-grid ${compact ? "is-compact" : ""}`}>
			{weekdays.map((weekday) => (
				<span className="ui-calendar-weekday" key={weekday}>
					{compact ? weekday.slice(0, 1) : weekday}
				</span>
			))}
			{monthCells(year, month).map((cell) => {
				const { day } = cell;
				if (day === null)
					return <span className="ui-calendar-day is-empty" key={cell.key} />;
				const date = dateKey(year, month, day);
				const marker = markers.get(date);
				const description = [
					marker?.explicit
						? `${marker.explicit} fixed ${marker.explicit === 1 ? "event" : "events"}`
						: "",
					marker?.recurring
						? `${marker.recurring} repeating ${marker.recurring === 1 ? "event" : "events"}`
						: "",
				]
					.filter(Boolean)
					.join(", ");
				return (
					<button
						type="button"
						className={`ui-calendar-day ${selectedDate === date ? "is-selected" : ""}`}
						key={cell.key}
						aria-label={`${monthNames[month]} ${day}, ${year}${description ? `, ${description}` : ""}`}
						onClick={() => onDaySelect?.(date)}
					>
						<span>{day}</span>
						<MarkerDots marker={marker} />
					</button>
				);
			})}
		</div>
	);
}

export function Calendar({
	view,
	year,
	month = 0,
	markers = [],
	selectedDate,
	onDaySelect,
	onMonthSelect,
	className = "",
}: CalendarProps) {
	const markerMap = new Map(markers.map((marker) => [marker.date, marker]));
	if (view === "year") {
		return (
			<div className={`ui-calendar ui-calendar-year ${className}`.trim()}>
				{monthNames.map((name, monthIndex) => (
					<section className="ui-calendar-mini-month" key={name}>
						<Button
							variant="ghost"
							className="ui-calendar-month-link"
							onClick={() => onMonthSelect?.(monthIndex)}
						>
							{name}
						</Button>
						<MonthGrid
							compact
							year={year}
							month={monthIndex}
							markers={markerMap}
							selectedDate={selectedDate}
							onDaySelect={onDaySelect}
						/>
					</section>
				))}
			</div>
		);
	}
	return (
		<section
			className={`ui-calendar ui-calendar-month ${className}`.trim()}
			aria-label={`${monthNames[month]} ${year}`}
		>
			<MonthGrid
				year={year}
				month={month}
				markers={markerMap}
				selectedDate={selectedDate}
				onDaySelect={onDaySelect}
			/>
		</section>
	);
}

export { monthNames as calendarMonthNames };
