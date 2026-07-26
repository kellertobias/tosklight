import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { CSSProperties } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ICON_CATALOG_GROUPS } from "../common/controls/iconCatalog";
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
		expect(card.querySelector(".pool-card-name")).toHaveTextContent(
			"Front Wash",
		);
		expect(card.querySelector(".pool-card-information")).toHaveTextContent(
			"4 fixtures · ordered",
		);
		expect(card.querySelector(".pool-card-media")).toHaveTextContent("◇");
		expect(card.querySelector(".pool-card-workflow")).toHaveTextContent(
			"Update",
		);
		expect(card).toHaveClass("has-status", "has-media");
		fireEvent.click(card);
		expect(select).toHaveBeenCalledOnce();
	});

	it("maps high-level workflow states to Record, Update, and Set", () => {
		const { rerender } = render(
			<PoolCard
				model={{
					number: 1,
					primary: "Record target",
					states: ["record-target"],
				}}
			/>,
		);
		expect(screen.getByText("Record")).toHaveClass("record");

		rerender(
			<PoolCard
				model={{
					number: 1,
					primary: "Update target",
					states: ["update-target"],
				}}
			/>,
		);
		expect(screen.getByText("Update")).toHaveClass("update");

		rerender(
			<PoolCard
				model={{ number: 1, primary: "Set target", states: ["set-target"] }}
			/>,
		);
		expect(screen.getByText("Set")).toHaveClass("set");
	});

	it("supports independently colored icon foreground and media background", () => {
		render(
			<PoolCard
				model={{
					number: 12,
					primary: "Verylongsinglewordthatneedstobreak",
					secondary: "12 fixtures",
					icon: "★",
					iconColor: "#ff00ff",
					iconBackgroundColor: "#223344",
				}}
			/>,
		);
		const card = screen.getByRole("button", {
			name: /Verylongsinglewordthatneedstobreak/u,
		});
		expect(card).toHaveStyle({
			"--pool-card-icon-color": "#ff00ff",
			"--pool-card-icon-background": "#223344",
		});
		expect(card.querySelector(".pool-card-name")).toBeInTheDocument();
		expect(card.querySelector(".pool-card-media")).toHaveTextContent("★");
	});

	it("renders catalog asset icons as images", () => {
		const icon = ICON_CATALOG_GROUPS.find(
			(group) => group.id === "fixture-type",
		)?.icons[0];
		expect(icon).toBeDefined();
		render(
			<PoolCard
				model={{
					number: 1,
					primary: "Profiles",
					icon: icon?.value,
					kind: "group",
				}}
			/>,
		);
		expect(
			screen
				.getByRole("button", { name: /Profiles/u })
				.querySelector(".pool-card-icon-image"),
		).toHaveAttribute("src", icon?.value);
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

	it("keeps an empty card actionable while applying its vacant visual state", () => {
		const click = vi.fn();
		render(
			<PoolCard
				model={{
					number: 6,
					primary: "Empty",
					kind: "group",
					states: ["empty"],
				}}
				onClick={click}
			/>,
		);
		const card = screen.getByRole("button", { name: /Empty/u });
		expect(card).toHaveClass("empty");
		expect(card.querySelector(".number")).toHaveTextContent("6");
		expect(card).toBeEnabled();
		fireEvent.click(card);
		expect(click).toHaveBeenCalledOnce();
	});

	it("suppresses configured colors on empty cards", () => {
		render(
			<PoolCard
				model={{
					number: 7,
					primary: "Empty",
					color: "#d8ad55",
					iconColor: "#d8ad55",
					iconBackgroundColor: "#332200",
					kind: "group",
					states: ["empty"],
				}}
				style={
					{
						"--pool-card-color": "#ffee00",
						"--pool-card-icon-color": "#ffee00",
						"--pool-card-icon-background": "#443300",
					} as CSSProperties
				}
			/>,
		);
		const card = screen.getByRole("button", { name: /Empty/u });
		expect(card).not.toHaveClass("has-color");
		expect(card.style.getPropertyValue("--pool-card-color")).toBe("");
		expect(card.style.getPropertyValue("--pool-card-icon-color")).toBe("");
		expect(card.style.getPropertyValue("--pool-card-icon-background")).toBe("");
		expect(
			screen.queryByLabelText("Configured color #d8ad55"),
		).not.toBeInTheDocument();
	});
});
