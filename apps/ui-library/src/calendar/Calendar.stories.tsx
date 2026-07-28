/**
 * Intentional product-design surface for the future Scheduler feature.
 * Keep this story until the production Scheduler adopts the shared calendar.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Calendar, type CalendarView } from "./Calendar";

const markers = [
	{ date: "2026-07-03", recurring: 1 },
	{ date: "2026-07-10", recurring: 1 },
	{ date: "2026-07-17", recurring: 1 },
	{ date: "2026-07-24", recurring: 1 },
	{ date: "2026-07-28", explicit: 2, recurring: 1 },
	{ date: "2026-08-01", explicit: 1 },
];

const meta = {
	title: "Controls/Calendar",
	component: Calendar,
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof Calendar>;

export default meta;
type Story = StoryObj<typeof meta>;

function CalendarHarness({ initialView }: { initialView: CalendarView }) {
	const [view, setView] = useState(initialView);
	const [month, setMonth] = useState(6);
	const [selectedDate, setSelectedDate] = useState<string | null>("2026-07-28");
	return (
		<div style={{ minHeight: "100vh", padding: 24, background: "var(--bg)" }}>
			<Calendar
				view={view}
				year={2026}
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
	);
}

export const Month: Story = {
	args: { view: "month", year: 2026 },
	render: () => <CalendarHarness initialView="month" />,
};

export const Year: Story = {
	args: { view: "year", year: 2026 },
	render: () => <CalendarHarness initialView="year" />,
};
