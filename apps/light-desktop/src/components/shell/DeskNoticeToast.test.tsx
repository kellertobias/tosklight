import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	DESK_NOTICE_DURATION_MS,
	reportDeskNotice,
} from "../../features/deskNotice/deskNotice";
import { DeskNoticeToast } from "./DeskNoticeToast";

afterEach(() => {
	cleanup();
	vi.useRealTimers();
});

describe("DeskNoticeToast", () => {
	it("announces a no-op politely without an alert or a Dismiss requirement", () => {
		render(<DeskNoticeToast />);
		const lane = screen.getByRole("status");
		expect(lane).toHaveAttribute("aria-live", "polite");
		expect(lane).toBeEmptyDOMElement();

		act(() => reportDeskNotice("No fixtures selected. Nothing changed."));

		expect(screen.getByLabelText("Desk notice")).toHaveTextContent(
			"No fixtures selected. Nothing changed.",
		);
		expect(screen.queryByRole("alert")).toBeNull();
		expect(screen.queryByText("Desk needs attention")).toBeNull();
	});

	it("expires on its own", () => {
		vi.useFakeTimers();
		render(<DeskNoticeToast />);
		act(() => reportDeskNotice("Nothing changed."));
		act(() => vi.advanceTimersByTime(DESK_NOTICE_DURATION_MS - 1));
		expect(screen.getByLabelText("Desk notice")).toBeInTheDocument();
		act(() => vi.advanceTimersByTime(1));
		expect(screen.queryByLabelText("Desk notice")).toBeNull();
	});

	it("restarts its lifetime for a repeated notice and can be dismissed early", () => {
		vi.useFakeTimers();
		render(<DeskNoticeToast />);
		act(() => reportDeskNotice("Nothing changed."));
		act(() => vi.advanceTimersByTime(DESK_NOTICE_DURATION_MS - 100));
		act(() => reportDeskNotice("Nothing changed."));
		act(() => vi.advanceTimersByTime(200));
		expect(screen.getByLabelText("Desk notice")).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "Dismiss notice" }));
		expect(screen.queryByLabelText("Desk notice")).toBeNull();
	});
});
