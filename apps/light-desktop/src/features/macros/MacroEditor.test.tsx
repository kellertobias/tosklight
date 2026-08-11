import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { MacrosApiClient, MacroValidation } from "../../api/client/macros";
import { MacroEditor } from "./MacroEditor";

afterEach(() => {
	cleanup();
	vi.useRealTimers();
});

const macro = {
	kind: "macro" as const,
	id: "00000000-0000-4000-8000-000000000160",
	revision: 4,
	updated_at: "2026-08-11T12:00:00Z",
	body: {
		id: "00000000-0000-4000-8000-000000000160",
		number: 160,
		name: "Front wash",
		source: "F",
		presentation: { color: "#315cab", icon: "play" },
	},
};

function api(validation: MacroValidation) {
	return {
		validate: vi.fn().mockResolvedValue(validation),
		create: vi.fn(),
		update: vi.fn(),
		delete: vi.fn(),
		run: vi.fn().mockResolvedValue({
			execution_id: "execution-160",
			state: "succeeded",
			message: "Macro completed",
		}),
		runLine: vi.fn(),
		undoRunLine: vi.fn(),
		execution: vi.fn(),
	} as unknown as MacrosApiClient;
}

const valid = (overrides: Partial<MacroValidation> = {}): MacroValidation => ({
	valid: true,
	diagnostics: [
		{
			line: 1,
			status: "valid",
			message: "Valid command",
			tokens: [],
		},
	],
	suggestions: [],
	...overrides,
});

describe("MacroEditor", () => {
	it("keeps identity controls in Macro Settings, runs the saved Macro, and removes editor copy/save undo", async () => {
		const client = api(valid());
		const onClose = vi.fn();
		vi.spyOn(window, "confirm").mockReturnValue(true);
		render(
			<MacroEditor
				showId="show-a"
				macro={macro}
				api={client}
				onClose={onClose}
				onSaved={vi.fn()}
			/>,
		);

		expect(screen.getByText("Macro")).toBeInTheDocument();
		expect(screen.queryByText("Macro 160")).not.toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Run Macro" })).toHaveTextContent(
			"▶ Run Macro",
		);
		fireEvent.click(screen.getByRole("button", { name: "Run Macro" }));
		await waitFor(() =>
			expect(client.run).toHaveBeenCalledWith("show-a", macro.id, {
				source_revision: 4,
				trigger: { type: "editor" },
			}),
		);
		expect(screen.queryByRole("button", { name: /copy/i })).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: /undo.*save/i }),
		).not.toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		expect(screen.getByRole("dialog", { name: "Macro Settings" })).toBeInTheDocument();
		expect(screen.getByRole("textbox", { name: "Name" })).toHaveValue(
			"Front wash",
		);
		fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
			target: { value: "Front wash revised" },
		});
		expect(screen.getByRole("textbox", { name: "Name" })).toHaveValue(
			"Front wash revised",
		);
		expect(screen.getByText("Icon")).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Delete Macro" }));
		await waitFor(() =>
			expect(client.delete).toHaveBeenCalledWith("show-a", macro.id, 4),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Close Macro Settings" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "← Macros" }));
		expect(onClose).toHaveBeenCalledOnce();
	});

	it("inserts the authoritative completion and exposes DEFINE expansion on hover", async () => {
		vi.useFakeTimers();
		const macroWithDefinition = {
			...macro,
			body: {
				...macro.body,
				source: "F\nDEFINE _front FIXTURE 1\n_front",
			},
		};
		const client = api(
			valid({
				diagnostics: [
					{
						line: 1,
						status: "valid",
						message: "Valid command",
						tokens: [],
					},
					{
						line: 2,
						status: "valid",
						message: "Valid command",
						tokens: [],
					},
					{
						line: 3,
						status: "valid",
						message: "Valid command",
						tokens: [
							{
								start: 0,
								end: 6,
								kind: "definition",
								expansion: "FIXTURE 1",
							},
						],
					},
				],
				suggestions: [
					{
						label: "FIXTURE",
						insertText: "FIXTURE ",
						detail: "Select fixtures by number or range",
						replaceStart: 0,
						replaceEnd: 1,
					},
				],
			}),
		);
		render(
			<MacroEditor
				showId="show-a"
				macro={macroWithDefinition}
				api={client}
				onClose={vi.fn()}
				onSaved={vi.fn()}
			/>,
		);

		await act(async () => {
			vi.advanceTimersByTime(180);
			await Promise.resolve();
		});
		expect(client.validate).toHaveBeenCalledWith(
			"show-a",
			"F\nDEFINE _front FIXTURE 1\n_front",
			0,
		);
		expect(screen.getByTitle("_front → FIXTURE 1")).toBeInTheDocument();
		const source = screen.getByRole("textbox", { name: "Macro command lines" });
		expect(source).toHaveAttribute("aria-activedescendant");
		fireEvent.keyDown(source, { key: "Enter" });
		expect(source).toHaveValue(
			"FIXTURE \nDEFINE _front FIXTURE 1\n_front",
		);
	});

	it("discards a late IntelliCode response for an older source revision", async () => {
		vi.useFakeTimers();
		let resolveOld: (validation: MacroValidation) => void = () => undefined;
		const client = api(valid());
		vi.mocked(client.validate)
			.mockImplementationOnce(
				() =>
					new Promise((resolve) => {
						resolveOld = resolve;
					}),
			)
			.mockResolvedValueOnce(
				valid({
					suggestions: [
						{
							label: "FIXTURE",
							insertText: "FIXTURE ",
							detail: "Current source",
							replaceStart: 0,
							replaceEnd: 2,
						},
					],
				}),
			);
		render(
			<MacroEditor
				showId="show-a"
				macro={macro}
				api={client}
				onClose={vi.fn()}
				onSaved={vi.fn()}
			/>,
		);
		await act(async () => vi.advanceTimersByTime(180));
		const source = screen.getByRole("textbox", { name: "Macro command lines" });
		fireEvent.change(source, { target: { value: "FI" } });
		await act(async () => vi.advanceTimersByTime(180));
		expect(screen.getByRole("option", { name: /FIXTURE/ })).toBeInTheDocument();

		await act(async () =>
			resolveOld(
				valid({
					suggestions: [
						{
							label: "FULL",
							insertText: "FULL ",
							detail: "Stale source",
							replaceStart: 0,
							replaceEnd: 1,
						},
					],
				}),
			),
		);
		expect(screen.queryByRole("option", { name: /FULL/ })).not.toBeInTheDocument();
	});
});
