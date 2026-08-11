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
		actions,
	}: {
		title: string;
		actions: Array<
			Array<{ id: string; label: string; onClick(): void; active?: boolean }>
		>;
	}) => (
		<header>
			<h1>{title}</h1>
			{actions.flat().map((action) => (
				<button
					key={action.id}
					type="button"
					aria-pressed={action.active}
					onClick={action.onClick}
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
});
