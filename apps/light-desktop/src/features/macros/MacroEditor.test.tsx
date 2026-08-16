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
import { MACRO_HELP_COMMANDS, MacroEditor } from "./MacroEditor";

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
		claimEditorInput: vi.fn().mockResolvedValue(undefined),
		releaseEditorInput: vi.fn().mockResolvedValue(undefined),
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
	it("claims focused desk input, inserts attached keypad text at the selection, and releases on blur", async () => {
		const client = api(valid());
		render(
			<MacroEditor
				showId="show-a"
				macro={macro}
				api={client}
				onClose={vi.fn()}
				onSaved={vi.fn()}
			/>,
		);

		const source = screen.getByRole("textbox", {
			name: "Macro command lines",
		}) as HTMLTextAreaElement;
		act(() => source.focus());
		await waitFor(() => expect(client.claimEditorInput).toHaveBeenCalledOnce());
		const instanceId = vi.mocked(client.claimEditorInput).mock.calls[0]?.[0];
		source.setSelectionRange(0, 1);
		act(() =>
			window.dispatchEvent(
				new CustomEvent("light:macro-editor-input", {
					detail: { instance_id: instanceId, action: "group" },
				}),
			),
		);
		expect(source).toHaveValue("GROUP ");

		source.setSelectionRange(6, 6);
		act(() =>
			window.dispatchEvent(
				new CustomEvent("light:macro-editor-input", {
					detail: { instance_id: instanceId, action: "digit-7" },
				}),
			),
		);
		expect(source).toHaveValue("GROUP 7");

		act(() => source.blur());
		expect(client.releaseEditorInput).toHaveBeenCalledWith(instanceId);
	});

	it("keeps identity controls in Macro Settings, runs the saved Macro, and removes editor copy/save undo", async () => {
		const client = api(valid());
		const onClose = vi.fn();
		vi.spyOn(window, "confirm").mockReturnValue(true);
		const { container } = render(
			<MacroEditor
				showId="show-a"
				macro={macro}
				api={client}
				onClose={onClose}
				onSaved={vi.fn()}
			/>,
		);

		expect(container.querySelectorAll(".ui-window-header")).toHaveLength(1);
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
		expect(
			screen.queryByRole("button", { name: /copy/i }),
		).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: /undo.*save/i }),
		).not.toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		expect(
			screen.getByRole("dialog", { name: "Macro Settings" }),
		).toBeInTheDocument();
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
		expect(onClose).toHaveBeenCalledOnce();
	});

	it("hides an invalid caret line until the caret moves to the next line", async () => {
		vi.useFakeTimers();
		const incomplete = valid({
			valid: false,
			diagnostics: [
				{
					line: 1,
					status: "invalid",
					message: "Fixture selection needs a number",
					tokens: [],
				},
			],
		});
		const client = api(valid());
		vi.mocked(client.validate)
			.mockResolvedValueOnce(valid())
			.mockResolvedValueOnce(incomplete)
			.mockResolvedValueOnce(incomplete)
			.mockResolvedValueOnce(valid());
		vi.mocked(client.update).mockResolvedValue({
			request_id: "request-1",
			replayed: false,
			show_id: "show-a",
			show_revision: 8,
			object: {
				kind: "macro",
				id: macro.id,
				revision: 5,
				updated_at: "2026-08-12T02:00:00Z",
				body: { ...macro.body, source: "FIXTURE 1" },
			},
		});
		render(
			<MacroEditor
				showId="show-a"
				macro={macro}
				api={client}
				onClose={vi.fn()}
				onSaved={vi.fn().mockResolvedValue(undefined)}
			/>,
		);

		await act(async () => vi.advanceTimersByTime(180));
		const source = screen.getByRole("textbox", { name: "Macro command lines" });
		fireEvent.change(source, { target: { value: "FIXTURE" } });
		expect(screen.getByRole("status")).toHaveTextContent(
			"Checking command line",
		);
		expect(screen.queryByText(/needs a number/)).not.toBeInTheDocument();

		await act(async () => vi.advanceTimersByTime(180));
		expect(screen.getByRole("status")).toHaveTextContent(
			"Editing current line",
		);
		expect(screen.queryByText(/needs a number/)).not.toBeInTheDocument();
		expect(client.update).not.toHaveBeenCalled();

		fireEvent.change(source, { target: { value: "FIXTURE\n" } });
		(source as HTMLTextAreaElement).setSelectionRange(8, 8);
		fireEvent.select(source);
		await act(async () => vi.advanceTimersByTime(180));
		expect(screen.getByRole("alert")).toHaveTextContent(
			"Command line needs attention",
		);
		expect(screen.getByText(/needs a number/)).toBeInTheDocument();

		fireEvent.change(source, { target: { value: "FIXTURE 1" } });
		expect(screen.queryByText(/needs a number/)).not.toBeInTheDocument();
		await act(async () => vi.advanceTimersByTime(180));
		expect(screen.getByRole("status")).toHaveTextContent("Autosave pending");
		await act(async () => vi.advanceTimersByTime(499));
		expect(client.update).not.toHaveBeenCalled();
		await act(async () => {
			vi.advanceTimersByTime(1);
			await Promise.resolve();
		});
		expect(client.update).toHaveBeenCalledWith("show-a", macro.id, 4, {
			number: 160,
			name: "Front wash",
			source: "FIXTURE 1",
			presentation: macro.body.presentation,
		});
		expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
		expect(screen.getByRole("status")).toHaveTextContent("Saved");
	});

	it("toggles persistent in-editor help with the authoritative Macro subset", async () => {
		const client = api(valid());
		render(
			<MacroEditor
				showId="show-a"
				macro={macro}
				api={client}
				onClose={vi.fn()}
				onSaved={vi.fn()}
			/>,
		);

		const toggle = screen.getByRole("button", { name: "Toggle Macro help" });
		expect(
			screen.queryByRole("complementary", { name: "Macro Editor help" }),
		).toBeNull();
		fireEvent.click(toggle);
		const help = screen.getByRole("complementary", {
			name: "Macro Editor help",
		});
		expect(toggle).toHaveClass("is-active");
		expect(help).toHaveTextContent("one desk command per line");
		expect(help).toHaveTextContent("unfinished current line stays neutral");
		expect(help).toHaveTextContent("initiating selection");
		for (const [command] of MACRO_HELP_COMMANDS)
			expect(help).toHaveTextContent(command);

		fireEvent.change(
			screen.getByRole("textbox", { name: "Macro command lines" }),
			{
				target: { value: "FIXTURE 1" },
			},
		);
		expect(
			screen.getByRole("complementary", { name: "Macro Editor help" }),
		).toBeInTheDocument();
		fireEvent.click(toggle);
		expect(
			screen.queryByRole("complementary", { name: "Macro Editor help" }),
		).toBeNull();
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
		expect(source).toHaveValue("FIXTURE \nDEFINE _front FIXTURE 1\n_front");
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
		expect(
			screen.queryByRole("option", { name: /FULL/ }),
		).not.toBeInTheDocument();
	});
});
