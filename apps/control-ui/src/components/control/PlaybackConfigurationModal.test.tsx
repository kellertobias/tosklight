import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PlaybackDefinition } from "../../api/types";
import {
	normalizePlaybackTopology,
	PlaybackConfigurationDialog,
	PlaybackConfigurationModal,
	withFunctionDefaults,
} from "./PlaybackConfigurationModal";

const mocks = vi.hoisted(() => ({
	savePlaybackSlot: vi.fn(),
	clearPlaybackSlot: vi.fn(),
	error: null as string | null,
	playbacks: {
		desk: { buttons: 3 },
		cue_lists: [
			{ id: "cue-1", name: "Main sequence" },
			{ id: "cue-2", name: "Encore" },
		],
	},
	scopedCueLists: [
		{ id: "cue-1", name: "Main sequence" },
		{ id: "cue-2", name: "Encore" },
	] as Array<{ id: string; name: string; storageId?: string }>,
	groups: [{ id: "group-1", body: { name: "Front Wash" } }],
}));

vi.mock("../../api/ServerContext", () => ({
	useServer: () => ({
		...mocks,
		playbacks: mocks.playbacks,
		groups: mocks.groups,
	}),
}));
vi.mock("../../features/server/useShowObjectsState", () => ({
	useGroups: () => mocks.groups,
}));
vi.mock("../../features/showObjects/ShowObjectsState", () => ({
	usePortableGroups: () => mocks.groups,
	useCueLists: () =>
		mocks.scopedCueLists.map((body) => ({
			kind: "cue_list",
			id: body.storageId ?? body.id,
			revision: 1,
			updated_at: "",
			body: { id: body.id, name: body.name },
		})),
}));

const base: PlaybackDefinition = {
	number: 7,
	name: "Configured Playback",
	target: { type: "cue_list", cue_list_id: "cue-1" },
	buttons: ["go_minus", "go", "flash"],
	button_count: 3,
	fader: "master",
	has_fader: true,
	go_activates: true,
	auto_off: true,
	xfade_millis: 0,
	color: "#20c997",
	flash_release: "release_all",
	protect_from_swap: false,
};

afterEach(cleanup);
beforeEach(() => {
	mocks.savePlaybackSlot.mockReset().mockResolvedValue(true);
	mocks.clearPlaybackSlot.mockReset().mockResolvedValue(true);
	mocks.error = null;
	mocks.playbacks.cue_lists = [
		{ id: "cue-1", name: "Main sequence" },
		{ id: "cue-2", name: "Encore" },
	];
	mocks.scopedCueLists = [
		{ id: "cue-1", name: "Main sequence" },
		{ id: "cue-2", name: "Encore" },
	];
});

function show(
	playback: PlaybackDefinition = base,
	props: { empty?: boolean; virtual?: boolean } = {},
) {
	const close = vi.fn();
	render(
		<PlaybackConfigurationModal
			playback={playback}
			page={2}
			slot={4}
			onClose={close}
			{...props}
		/>,
	);
	return close;
}
function selectTrigger(label: string) {
	return screen
		.getByText(label, { selector: "label", exact: true })
		.closest(".ui-form-field")!
		.querySelector(".ui-select-trigger") as HTMLButtonElement;
}
function colorTrigger(label: string) {
	return screen
		.getByText(label, { selector: "label", exact: true })
		.closest(".ui-form-field")!
		.querySelector(".ui-color-input-trigger") as HTMLButtonElement;
}
function choose(label: string, option: string) {
	fireEvent.click(selectTrigger(label));
	fireEvent.click(screen.getByRole("option", { name: option }));
}

describe("PlaybackConfigurationModal function and behavior", () => {
	it("uses a Cuelist semantic ID instead of its legacy storage key", async () => {
		const cueListId = "11111111-1111-4111-8111-111111111111";
		mocks.scopedCueLists = [
			{ id: cueListId, name: "Legacy Main", storageId: "main" },
		];
		show({ ...base, target: { type: "cue_list", cue_list_id: cueListId } });

		const option = screen.getByRole("radio", { name: "Legacy Main" });
		expect(option).toHaveAttribute("aria-checked", "true");
		fireEvent.click(option);
		fireEvent.change(screen.getByLabelText("Playback name"), {
			target: { value: "Changed" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));

		await waitFor(() =>
			expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(
				2,
				4,
				expect.objectContaining({
					target: { type: "cue_list", cue_list_id: cueListId },
				}),
			),
		);
	});

	it("uses three tabs, title-bar Apply, Close, and no footer Cancel", () => {
		show();
		for (const tab of ["Function", "Behavior", "Layout"])
			expect(screen.getByRole("button", { name: tab })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Apply" })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Apply" })).toBeDisabled();
		expect(
			screen.getByRole("button", { name: "Close playback configuration" }),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Cancel" }),
		).not.toBeInTheDocument();
		expect(screen.queryByText("Page 2 · Playback 4")).not.toBeInTheDocument();
		expect(screen.queryByText("3 buttons · fader")).not.toBeInTheDocument();
	});

	it("shows scrollable two-column lists and groups the persisted special targets", () => {
		show();
		expect(
			screen.getByRole("radiogroup", { name: "Playback function" }),
		).toBeInTheDocument();
		expect(document.querySelector(".ui-selection-tree")).toBeInTheDocument();
		expect(document.querySelectorAll(".ui-selection-list-scroll")).toHaveLength(
			2,
		);
		expect(
			screen.getByRole("radiogroup", { name: "Cue List options" }),
		).toBeInTheDocument();
		expect(
			screen.queryByText("Cue List", { selector: "label" }),
		).not.toBeInTheDocument();
		fireEvent.click(screen.getByRole("radio", { name: "Speed Master" }));
		expect(
			screen.getByRole("radiogroup", { name: "Speed Group options" }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("radio", { name: "Speed Group E" }),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("radio", { name: "Special" }));
		expect(
			screen.getByRole("radiogroup", { name: "Special options" }),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("radio", { name: "Grand Master" }));
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		return waitFor(() =>
			expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(
				2,
				4,
				expect.objectContaining({ target: { type: "grand_master" } }),
			),
		);
	});

	it("keeps name and the responsive color dropdown on Function only", async () => {
		show();
		expect(screen.getByLabelText("Playback name")).toBeInTheDocument();
		const modal = screen.getByRole("dialog", {
			name: "Playback Configuration",
		});
		fireEvent.click(colorTrigger("Playback color"));
		expect(
			screen.getAllByRole("option", { name: /^Use color #/ }),
		).toHaveLength(16);
		expect(
			document
				.querySelector(".ui-color-dropdown-panel")
				?.closest(".playback-configuration-modal"),
		).toBeNull();
		expect(modal.querySelector(".ui-color-dropdown-panel")).toBeNull();
		fireEvent.click(screen.getByRole("option", { name: "Use color #8b5cf6" }));
		fireEvent.click(screen.getByRole("button", { name: "Layout" }));
		expect(screen.queryByLabelText("Playback name")).not.toBeInTheDocument();
		expect(
			screen.queryByText("Playback color", { selector: "label" }),
		).not.toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		await waitFor(() =>
			expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(
				2,
				4,
				expect.objectContaining({ color: "#8b5cf6" }),
			),
		);
	});
});

describe("PlaybackConfigurationModal behavior compatibility", () => {
	it("uses the stage-style toggle for cue-list flash release and persists Behavior", async () => {
		show();
		fireEvent.click(screen.getByRole("button", { name: "Behavior" }));
		expect(
			screen.getByRole("radiogroup", {
				name: "When Flash or Swap is released",
			}),
		).toBeInTheDocument();
		expect(
			screen.getByText(/leaves this Cue List active at zero intensity/),
		).toBeInTheDocument();
		expect(screen.getByRole("radio", { name: "Release all" })).toHaveAttribute(
			"aria-checked",
			"true",
		);
		fireEvent.click(screen.getByRole("radio", { name: "Intensity only" }));
		fireEvent.click(
			screen.getByRole("switch", {
				name: "Turn off when other playbacks take full control",
			}),
		);
		fireEvent.click(screen.getByRole("switch", { name: "Protect from Swap" }));
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		await waitFor(() =>
			expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(
				2,
				4,
				expect.objectContaining({
					flash_release: "release_intensity_only",
					auto_off: false,
					protect_from_swap: true,
				}),
			),
		);
	});

	it("resets incompatible mappings and explains choices in an additional modal", async () => {
		show({
			...base,
			buttons: ["swap", "select_contents", "fast_forward"],
			fader: "x_fade",
		});
		fireEvent.click(screen.getByRole("radio", { name: "Group Master" }));
		fireEvent.click(screen.getByRole("radio", { name: "Front Wash" }));
		fireEvent.click(screen.getByRole("button", { name: "Layout" }));
		expect(selectTrigger("Top button")).toHaveTextContent("Select");
		expect(selectTrigger("Middle button")).toHaveTextContent(
			"Select dereferenced",
		);
		expect(selectTrigger("Bottom button")).toHaveTextContent("Flash");
		fireEvent.click(selectTrigger("Top button"));
		const dialog = screen.getByRole("dialog", {
			name: "Choose Top button function",
		});
		expect(
			within(dialog).getByRole("heading", { name: "Selection" }),
		).toBeInTheDocument();
		expect(
			within(dialog).getByRole("heading", { name: "Temporary State" }),
		).toBeInTheDocument();
		expect(
			within(dialog).getByRole("button", { name: "Empty Button" }),
		).toBeInTheDocument();
		expect(
			within(dialog).getByText(
				"Selects this playback or its live Group reference without executing it.",
			),
		).toBeInTheDocument();
		fireEvent.click(
			within(dialog).getByRole("button", {
				name: "Close Top button function choices",
			}),
		);
		expect(selectTrigger("Fader")).toBeDisabled();
		expect(selectTrigger("Fader")).toHaveTextContent("Group intensity master");
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		await waitFor(() =>
			expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(
				2,
				4,
				expect.objectContaining({
					target: { type: "group", group_id: "group-1" },
					buttons: ["select", "select_dereferenced", "flash"],
					fader: "master",
				}),
			),
		);
	});
});

describe("PlaybackConfigurationModal layout and persistence", () => {
	it("groups button and fader functions and moves Empty Button to the title", () => {
		show();
		fireEvent.click(screen.getByRole("button", { name: "Layout" }));
		expect(selectTrigger("Top button")).toHaveTextContent("GO −");
		expect(selectTrigger("Middle button")).toHaveTextContent("GO +");
		fireEvent.click(selectTrigger("Top button"));
		let dialog = screen.getByRole("dialog", {
			name: "Choose Top button function",
		});
		for (const heading of [
			"Step Control",
			"Permanent State",
			"Temporary State",
			"Selection",
		])
			expect(
				within(dialog).getByRole("heading", { name: heading }),
			).toBeInTheDocument();
		expect(
			within(dialog).getByRole("button", { name: /^FFW/ }),
		).toBeInTheDocument();
		expect(
			within(dialog).getByRole("button", { name: /^FRW/ }),
		).toBeInTheDocument();
		expect(
			within(dialog).queryByRole("button", { name: "Disabled" }),
		).not.toBeInTheDocument();
		fireEvent.click(
			within(dialog).getByRole("button", { name: "Empty Button" }),
		);
		expect(selectTrigger("Top button")).toHaveTextContent("Empty Button");
		expect(selectTrigger("Top button")).toHaveClass("is-empty");
		fireEvent.click(selectTrigger("Fader"));
		dialog = screen.getByRole("dialog", { name: "Choose Fader function" });
		expect(
			within(dialog).getByRole("heading", { name: "Level Control" }),
		).toBeInTheDocument();
		expect(
			within(dialog).getByRole("heading", { name: "Cue Transition" }),
		).toBeInTheDocument();
		expect(
			within(dialog).queryByRole("button", { name: "Empty Button" }),
		).not.toBeInTheDocument();
	});

	it("previews None as inactive and clears only when Apply is pressed", async () => {
		const close = show();
		fireEvent.click(screen.getByRole("radio", { name: "None" }));
		expect(screen.getByText("Playback will be cleared")).toBeInTheDocument();
		expect(mocks.clearPlaybackSlot).not.toHaveBeenCalled();
		fireEvent.click(
			screen.getByRole("button", { name: "Close playback configuration" }),
		);
		expect(close).toHaveBeenCalledOnce();
		expect(mocks.clearPlaybackSlot).not.toHaveBeenCalled();
		cleanup();
		show();
		fireEvent.click(screen.getByRole("radio", { name: "None" }));
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		await waitFor(() =>
			expect(mocks.clearPlaybackSlot).toHaveBeenCalledWith(2, 4),
		);
	});

	it("keeps Apply disabled until the normalized draft actually differs", () => {
		show();
		const apply = screen.getByRole("button", { name: "Apply" });
		const name = screen.getByLabelText("Playback name");
		expect(apply).toBeDisabled();
		fireEvent.change(name, { target: { value: "Changed" } });
		expect(apply).toBeEnabled();
		fireEvent.change(name, { target: { value: base.name } });
		expect(apply).toBeDisabled();
		fireEvent.click(screen.getByRole("radio", { name: "None" }));
		expect(apply).toBeEnabled();
	});

	it("treats None as unchanged for an already empty slot", () => {
		show(base, { empty: true });
		const apply = screen.getByRole("button", { name: "Apply" });
		expect(apply).toBeDisabled();
		fireEvent.click(screen.getByRole("radio", { name: "None" }));
		expect(apply).toBeDisabled();
	});

	it("uses the same reusable row geometry for an empty option list", () => {
		mocks.scopedCueLists = [];
		show();
		const empty = screen.getByRole("status");
		const option = screen.getByRole("radio", { name: "Cue List" });
		expect(empty).toHaveTextContent("No options are available");
		expect(empty).toHaveClass("ui-selection-list-option");
		expect(option).toHaveClass("ui-selection-list-option");
	});

	it("lists scoped Cuelists when the legacy Playback snapshot is stale", () => {
		mocks.playbacks.cue_lists = [{ id: "legacy", name: "Legacy" }];
		mocks.scopedCueLists = [{ id: "scoped", name: "Scoped authority" }];

		show();

		expect(
			screen.getByRole("radio", { name: "Scoped authority" }),
		).toBeInTheDocument();
		expect(screen.queryByRole("radio", { name: "Legacy" })).toBeNull();
	});

	it("renders exactly one control and no fader for a virtual topology", () => {
		show(
			{
				...base,
				buttons: ["toggle", "none", "none"],
				button_count: 1,
				has_fader: false,
			},
			{ virtual: true },
		);
		expect(
			screen.getByRole("dialog", { name: "Playback Configuration" }),
		).toHaveAttribute("data-topology", "1 button · faderless");
		expect(selectTrigger("Presentation")).toHaveTextContent("Label");
		fireEvent.click(screen.getByRole("button", { name: "Layout" }));
		expect(selectTrigger("Top button")).toHaveTextContent("Toggle");
		expect(
			screen.queryByText("Middle button", { selector: "label", exact: true }),
		).not.toBeInTheDocument();
		expect(screen.getByText("No fader on this playback.")).toBeInTheDocument();
	});

	it("replaces a generic failed Apply message with the scoped action error", async () => {
		const save = vi.fn().mockResolvedValue(false);
		const props = {
			playback: base,
			page: 2,
			slot: 4,
			fallbackButtons: 1,
			save,
			clear: vi.fn().mockResolvedValue(false),
			onClose: vi.fn(),
		};
		const rendered = render(
			<PlaybackConfigurationDialog {...props} error={null} virtual />,
		);
		fireEvent.change(screen.getByLabelText("Playback name"), {
			target: { value: "Changed" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		await screen.findByText("Playback configuration could not be saved.");

		rendered.rerender(
			<PlaybackConfigurationDialog
				{...props}
				error="stale Playback revision"
				virtual
			/>,
		);

		await screen.findByText("stale Playback revision");
	});
});

describe("PlaybackConfigurationModal topology defaults", () => {
	it.each([
		"programmer_fade",
		"cue_fade",
	] as const)("applies Double, Half, and Off immediately for the %s time master", (type) => {
		show(withFunctionDefaults(base, type, "cue-1", "group-1"));
		fireEvent.click(screen.getByRole("button", { name: "Layout" }));
		expect(selectTrigger("Top button")).toHaveTextContent("Double");
		expect(selectTrigger("Middle button")).toHaveTextContent("Half");
		expect(selectTrigger("Bottom button")).toHaveTextContent("Off");
		for (const label of ["Top button", "Middle button", "Bottom button"])
			expect(selectTrigger(label)).toBeEnabled();
		fireEvent.click(selectTrigger("Top button"));
		expect(
			within(
				screen.getByRole("dialog", { name: "Choose Top button function" }),
			).getByRole("heading", { name: "Time Control" }),
		).toBeInTheDocument();
		expect(selectTrigger("Fader")).toBeDisabled();
		expect(selectTrigger("Fader")).toHaveTextContent(
			type === "programmer_fade" ? "Programmer Fade time" : "Cue Fade time",
		);
	});

	it("applies the Grand Master default order immediately", () => {
		show();
		fireEvent.click(screen.getByRole("radio", { name: "Special" }));
		fireEvent.click(screen.getByRole("radio", { name: "Grand Master" }));
		fireEvent.click(screen.getByRole("button", { name: "Layout" }));
		expect(selectTrigger("Top button")).toHaveTextContent("Blackout");
		expect(selectTrigger("Middle button")).toHaveTextContent("Pause Dynamics");
		expect(selectTrigger("Bottom button")).toHaveTextContent("Flash");
	});

	it("persists mutually exclusive virtual presentation", async () => {
		show({ ...base, button_count: 1, has_fader: false }, { virtual: true });
		choose("Presentation", "Image background");
		fireEvent.change(screen.getByLabelText("Image background"), {
			target: { value: "show://images/blue-wash.png" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		await waitFor(() =>
			expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(
				2,
				4,
				expect.objectContaining({
					presentation_image: "show://images/blue-wash.png",
					presentation_icon: undefined,
				}),
			),
		);
	});

	it("migrates legacy topology deterministically and clears hidden actions", () => {
		expect(
			normalizePlaybackTopology(
				{ ...base, button_count: undefined, has_fader: undefined },
				2,
				false,
			),
		).toMatchObject({
			button_count: 2,
			has_fader: false,
			buttons: ["go_minus", "go", "none"],
		});
	});
});
