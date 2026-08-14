// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { RunningRow } from "./model";
import { RunningPane } from "./RunningPane";

vi.mock("@tosklight/ui/window-kit", () => ({
	WindowScrollArea: ({ children }: { children: ReactNode }) => children,
	WindowHeader: ({
		title,
		groups,
	}: {
		title: string;
		groups: Array<{
			activeId: string;
			onActiveChange(id: string): void;
			actions: Array<{ id: string; label: string }>;
		}>;
	}) => (
		<header>
			<h1>{title}</h1>
			{groups.flatMap((group) => group.actions.map((action) => ({ action, group }))).map(({ action, group }) => (
				<button
					key={action.id}
					type="button"
					aria-pressed={group.activeId === action.id}
					onClick={() => group.onActiveChange(action.id)}
				>
					{action.label}
				</button>
			))}
		</header>
	),
}));

afterEach(cleanup);

function row(
	kind: RunningRow["kind"],
	number: number,
	name: string,
	off = vi.fn(),
): RunningRow {
	return {
		key: `${kind}:${number}`,
		kind,
		number,
		name,
		status: "Running",
		cueNumber: kind === "cue_list" ? 4 : null,
		off,
		...(kind === "cue_list" ? { source: {} } : {}),
		...(kind === "dynamic" ? { controller: {} } : {}),
		...(kind === "timecode" ? { snapshot: {} } : {}),
		...(kind === "macro" ? { execution: {} } : {}),
	} as unknown as RunningRow;
}

describe("RunningPane", () => {
	it("defaults to all four kinds with literal identity and Cue labels", () => {
		render(
			<RunningPane
				rows={[
					row("cue_list", 2, "Main"),
					row("dynamic", 4, "Circle"),
					row("timecode", 6, "Intro"),
					row("macro", 8, "Reset"),
				]}
			/>,
		);
		expect(screen.getAllByRole("article")).toHaveLength(4);
		expect(screen.getByText("Cuelist · Cue 4 · Running")).toBeInTheDocument();
		expect(screen.getByText("Dynamic · Cue — · Running")).toBeInTheDocument();
		expect(screen.getByText("Timecode · Cue — · Running")).toBeInTheDocument();
		expect(screen.getByText("Macro · Cue — · Running")).toBeInTheDocument();
		expect(screen.getAllByRole("article")[0]).toHaveStyle({
			"--running-kind-color": "#93cc55",
		});
		expect(screen.getAllByRole("article")[1]).toHaveStyle({
			"--running-kind-color": "#3bbdce",
		});
		expect(screen.getAllByRole("article")[2]).toHaveStyle({
			"--running-kind-color": "#48c0ff",
		});
		expect(screen.getAllByRole("article")[3]).toHaveStyle({
			"--running-kind-color": "#8f3541",
		});
	});

	it("filters to one kind and explains the empty filtered view", () => {
		render(<RunningPane rows={[row("cue_list", 2, "Main")]} />);
		fireEvent.click(screen.getByRole("button", { name: "Macros" }));
		expect(screen.queryByRole("article")).not.toBeInTheDocument();
		expect(screen.getByText("No Macros are running.")).toBeInTheDocument();
	});

	it("routes one exact Off and blocks duplicate clicks while it is pending", () => {
		let finish: (() => void) | undefined;
		const off = vi.fn(
			() =>
				new Promise<void>((resolve) => {
					finish = resolve;
				}),
		);
		render(<RunningPane rows={[row("macro", 8, "Reset", off)]} />);
		const button = screen.getByRole("button", {
			name: "Turn off Macro 8 Reset",
		});
		fireEvent.click(button);
		fireEvent.click(button);
		expect(off).toHaveBeenCalledTimes(1);
		expect(button).toBeDisabled();
		finish?.();
	});

	it("reports an actionable row-specific error when authoritative Off is rejected", async () => {
		const off = vi.fn().mockResolvedValue(null);
		render(<RunningPane rows={[row("cue_list", 4, "Act One", off)]} />);

		fireEvent.click(
			screen.getByRole("button", { name: "Turn off Cuelist 4 Act One" }),
		);

		expect(
			await screen.findByText(
				/Could not turn off Cuelist 4 · Act One:.*check the desk connection and try again/,
			),
		).toBeInTheDocument();
	});
});
