import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PoolCard } from "./PoolCard";

afterEach(() => {
	cleanup();
	vi.useRealTimers();
});

describe("PoolCard", () => {
	it("preserves ordered state, warnings, color, icon, and callbacks", () => {
		const select = vi.fn();
		render(
			<PoolCard
				model={{
					number: 4,
					primary: "Front Wash",
					secondary: "4 fixtures · ordered",
					details: ["⚠ 1 missing", "2 portable attributes"],
					icon: "◇",
					color: "#1bd6ec",
					kind: "group",
					states: ["selected", "update-target"],
					derived: true,
					frozen: true,
				}}
				onClick={select}
			/>,
		);
		const card = screen.getByRole("button", { name: /Front Wash/ });
		expect(card).toHaveClass(
			"group-card",
			"selected",
			"update-target",
			"has-color",
		);
		expect(card).toHaveStyle({ "--pool-card-color": "#1bd6ec" });
		expect(
			screen.getByLabelText("Configured color #1bd6ec"),
		).toBeInTheDocument();
		expect(screen.getByLabelText("Derived state")).toHaveTextContent("Derived");
		expect(screen.getByLabelText("Frozen state")).toHaveTextContent("Frozen");
		fireEvent.click(card);
		expect(select).toHaveBeenCalledOnce();
	});

	it("separates a press-and-hold callback from the following click", () => {
		vi.useFakeTimers();
		const click = vi.fn();
		const hold = vi.fn();
		render(
			<PoolCard
				model={{ number: 1, primary: "All", kind: "group" }}
				onClick={click}
				onPressHold={hold}
			/>,
		);
		const card = screen.getByRole("button", { name: /All/ });
		fireEvent.pointerDown(card);
		vi.advanceTimersByTime(650);
		fireEvent.pointerUp(card);
		fireEvent.click(card);
		expect(hold).toHaveBeenCalledOnce();
		expect(click).not.toHaveBeenCalled();
	});

	it("cancels a pending hold and keeps a short press as a click", () => {
		vi.useFakeTimers();
		const click = vi.fn();
		const hold = vi.fn();
		render(
			<PoolCard
				model={{ number: 2, primary: "Front", kind: "group" }}
				onClick={click}
				onPressHold={hold}
			/>,
		);
		const card = screen.getByRole("button", { name: /Front/ });
		fireEvent.pointerDown(card);
		vi.advanceTimersByTime(200);
		fireEvent.pointerUp(card);
		fireEvent.click(card);
		expect(hold).not.toHaveBeenCalled();
		expect(click).toHaveBeenCalledOnce();
	});
});
