import {
	cleanup,
	fireEvent,
	render as rtlRender,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import type { ReactElement } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PlaybackDefinition } from "../../api/types";
import {
	normalizePlaybackTopology,
	PlaybackConfigurationDialog,
	playbackImageDataUrl,
	withFunctionDefaults,
} from "./PlaybackConfigurationModal";

import { familyFromTarget, playbackLayoutOptions } from "./PlaybackConfigurationTabs";

const render = (ui: ReactElement) => rtlRender(ui, { wrapper: ModalProvider });

const mocks = vi.hoisted(() => ({
	savePlaybackSlot: vi.fn(),
	clearPlaybackSlot: vi.fn(),
	saveCueList: vi.fn(),
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
	] as Array<{
		id: string;
		name: string;
		storageId?: string;
		auto_off_at_zero?: boolean;
		auto_off_flash_release?: boolean;
	}>,
	macros: [{ id: "macro-1", body: { id: "macro-1", number: 2, name: "Open sequence" } }],
	timecodes: [{ id: "timecode-1", body: { id: "timecode-1", number: 3, name: "Opening transport" } }],
	groups: [{ id: "group-1", body: { name: "Front Wash" } }],
	dynamics: [
		{
			id: "dynamic-1",
			body: {
				pool_number: 1,
				name: "Pulse",
				target_binding: { type: "targetless" },
			},
		},
	],
}));

vi.mock("../../features/server/useShowObjectsState", () => ({
	useGroups: () => mocks.groups,
}));
vi.mock("../files/RootConfinedFilePickerButton", () => ({
	RootConfinedFilePickerButton: ({
		label,
		onFiles,
	}: {
		label: string;
		onFiles: (files: File[]) => void | Promise<void>;
	}) => (
		<button
			type="button"
			onClick={() =>
				void onFiles([
					new File(["image"], "blue-wash.png", { type: "image/png" }),
				])
			}
		>
			{label}
		</button>
	),
}));
vi.mock("../../features/showObjects/ShowObjectsState", () => ({
	useMacros: () => mocks.macros,
	useTimecodes: () => mocks.timecodes,
	usePortableGroups: () => mocks.groups,
	useDynamics: () => mocks.dynamics,
	useCueLists: () =>
		mocks.scopedCueLists.map((body, index) => ({
			kind: "cue_list",
			id: body.storageId ?? body.id,
			revision: 1,
			updated_at: "",
			body: { ...body, pool_number: [12, 1000][index] },
		})),
}));
vi.mock("../../features/playbackTopology/useCueListTopologyWriter", () => ({
	cueListWriteBasis: (object: {
		body: { id: string };
		id: string;
		revision: number;
	}) => ({
		cueListId: object.body.id,
		expectedObjectId: object.id,
		expectedRevision: object.revision,
	}),
	useCueListTopologyWriter: () => mocks.saveCueList,
}));

const base: PlaybackDefinition = {
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
	mocks.saveCueList.mockReset().mockImplementation(async (_basis, body) => ({
		id: body.id,
		revision: 2,
		body,
	}));
	mocks.error = null;
	mocks.dynamics.splice(1);
	Object.assign(mocks.dynamics[0].body.target_binding, {
		type: "live_group",
		group_id: "group-1",
	});
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
	props: { empty?: boolean } = {},
) {
	const close = vi.fn();
	render(
		<PlaybackConfigurationDialog
			playback={playback}
			page={2}
			slot={4}
			fallbackButtons={playback.button_count ?? 3}
			save={mocks.savePlaybackSlot}
			clear={mocks.clearPlaybackSlot}
			error={mocks.error}
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
	it("persists shared Cuelist Auto-off options independently", async () => {
		mocks.scopedCueLists[0] = {
			...mocks.scopedCueLists[0],
			auto_off_at_zero: false,
			auto_off_flash_release: false,
		};
		show();
		fireEvent.click(screen.getByRole("button", { name: "Behavior" }));
		const group = screen.getByRole("group", { name: "Auto-off" });
		fireEvent.click(
			within(
				within(group).getByRole("radiogroup", {
					name: "When fader reaches zero",
				}),
			).getByRole("radio", { name: "Release All" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		await waitFor(() => expect(mocks.saveCueList).toHaveBeenCalledTimes(1));
		expect(mocks.saveCueList.mock.calls[0][1]).toMatchObject({
			auto_off_at_zero: true,
			auto_off_flash_release: false,
		});
	});
	it("preserves a zero Group Master seed for fader and button-only presentation", async () => {
		for (const has_fader of [false, true]) {
			show({
				...base,
				has_fader,
				target: {
					type: "group",
					group_id: "group-1",
					initial_master: 0,
				},
			});
			fireEvent.change(screen.getByLabelText("Playback name"), {
				target: { value: has_fader ? "Fader zero" : "Buttons zero" },
			});
			fireEvent.click(screen.getByRole("button", { name: "Apply" }));
			await waitFor(() => expect(mocks.savePlaybackSlot).toHaveBeenCalled());
			expect(mocks.savePlaybackSlot).toHaveBeenLastCalledWith(
				2,
				4,
				expect.objectContaining({
					target: {
						type: "group",
						group_id: "group-1",
						initial_master: 0,
					},
				}),
			);
			cleanup();
			mocks.savePlaybackSlot.mockClear();
		}
	});

	it("uses a Cuelist semantic ID instead of its legacy storage key", async () => {
		const cueListId = "11111111-1111-4111-8111-111111111111";
		mocks.scopedCueLists = [
			{ id: cueListId, name: "Legacy Main", storageId: "main" },
		];
		show({ ...base, target: { type: "cue_list", cue_list_id: cueListId } });

		const option = screen.getByRole("radio", { name: "12 Legacy Main" });
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
			screen.getByRole("radio", { name: "12 Main sequence" }),
		).toBeInTheDocument();
		expect(screen.getByText("12")).toHaveClass(
			"playback-numbered-object-number",
		);
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
	it("shows one exact shared policy for Flash and Swap release", async () => {
		show();
		fireEvent.click(screen.getByRole("button", { name: "Behavior" }));
		const releasePolicy = screen.getByRole("radiogroup", {
			name: "When Flash or Swap is released",
		});
		expect(
			within(releasePolicy).getByRole("radio", { name: "Keep Running" }),
		).toHaveAttribute("aria-checked", "true");
		expect(screen.queryByText("Intensity only")).not.toBeInTheDocument();
		fireEvent.click(
			within(releasePolicy).getByRole("radio", { name: "Release All" }),
		);
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
					flash_release: "release_all",
					auto_off: false,
					protect_from_swap: true,
				}),
			),
		);
		expect(mocks.saveCueList.mock.calls[0][1]).toMatchObject({
			auto_off_flash_release: true,
		});
	});

	it("resets incompatible mappings and explains choices in an additional modal", async () => {
		show({
			...base,
			buttons: ["swap", "select_contents", "fast_forward"],
			fader: "x_fade",
		});
		fireEvent.click(screen.getByRole("radio", { name: "Group Master" }));
		fireEvent.click(screen.getByRole("radio", { name: "1 Front Wash" }));
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
			screen.getByRole("radio", { name: "12 Scoped authority" }),
		).toBeInTheDocument();
		expect(screen.queryByRole("radio", { name: "Legacy" })).toBeNull();
	});

	it.each([
		true,
		false,
	])("persists presentation and expanded footprint for canonical fader=%s", async (has_fader) => {
		show({ ...base, button_count: 1, has_fader });
		choose("Presentation", "Icon");
		fireEvent.change(screen.getByLabelText("Icon"), { target: { value: "★" } });
		fireEvent.click(screen.getByRole("button", { name: "Layout" }));
		fireEvent.click(screen.getByRole("radio", { name: "Taller" }));
		fireEvent.click(selectTrigger("Additional upper button"));
		fireEvent.click(
			within(
				screen.getByRole("dialog", {
					name: "Choose Additional upper button function",
				}),
			).getByRole("button", { name: /^PausePauses/ }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		await waitFor(() =>
			expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(
				2,
				4,
				expect.objectContaining({
					button_count: 1,
					has_fader,
					presentation_icon: "★",
					presentation_image: undefined,
					footprint: { type: "taller", upper_button: "pause" },
				}),
			),
		);
	});

	it("renders exactly one control and no fader for a canonical button-only topology", () => {
		show({
			...base,
			buttons: ["toggle", "none", "none"],
			button_count: 1,
			has_fader: false,
		});
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
			<PlaybackConfigurationDialog {...props} error={null} />,
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
			/>,
		);

		await screen.findByText("stale Playback revision");
	});
});

describe("PlaybackConfigurationModal topology defaults", () => {
	it("persists the two Dynamic Auto-off options independently", async () => {
		show(
			withFunctionDefaults(
				base,
				"dynamic",
				"cue-1",
				"group-1",
				mocks.dynamics[0] as never,
			),
		);
		fireEvent.click(screen.getByRole("button", { name: "Behavior" }));

		const group = screen.getByRole("group", { name: "Auto-off" });
		const atZero = within(group).getByRole("radiogroup", {
			name: "When fader reaches zero",
		});
		const afterFlash = within(group).getByRole("radiogroup", {
			name: "When Flash or Swap is released",
		});
		expect(
			within(atZero).getByRole("radio", { name: "Keep Running" }),
		).toHaveAttribute("aria-checked", "true");
		expect(
			within(afterFlash).getByRole("radio", { name: "Keep Running" }),
		).toHaveAttribute("aria-checked", "true");

		fireEvent.click(within(atZero).getByRole("radio", { name: "Release All" }));
		expect(
			within(atZero).getByRole("radio", { name: "Release All" }),
		).toHaveAttribute("aria-checked", "true");
		expect(
			within(afterFlash).getByRole("radio", { name: "Keep Running" }),
		).toHaveAttribute("aria-checked", "true");
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));

		await waitFor(() => expect(mocks.savePlaybackSlot).toHaveBeenCalledOnce());
		expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(
			2,
			4,
			expect.objectContaining({
				target: expect.objectContaining({
					type: "dynamic",
					assignment: expect.objectContaining({
						auto_off_at_zero: true,
						auto_off_flash_release: false,
					}),
				}),
			}),
		);
	});

	it("uses the server Dynamic Playback defaults", () => {
		show(
			withFunctionDefaults(
				base,
				"dynamic",
				"cue-1",
				"group-1",
				mocks.dynamics[0] as never,
			),
		);
		fireEvent.click(screen.getByRole("button", { name: "Layout" }));
		expect(selectTrigger("Top button")).toHaveTextContent("Off");
		expect(selectTrigger("Middle button")).toHaveTextContent("Pause");
		expect(selectTrigger("Bottom button")).toHaveTextContent("Flash");
	});

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

	it("persists mutually exclusive Playback presentation", async () => {
		show({ ...base, button_count: 1, has_fader: false });
		choose("Presentation", "Image background");
		fireEvent.click(screen.getByRole("button", { name: "Choose image" }));
		await screen.findByAltText("Selected playback background");
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		await waitFor(() =>
			expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(
				2,
				4,
				expect.objectContaining({
					presentation_image: "data:image/png;base64,aW1hZ2U=",
					presentation_icon: undefined,
				}),
			),
		);
	});

	it("rejects oversized playback images before storing them", async () => {
		const file = new File([new Uint8Array(400 * 1024 + 1)], "large.png", {
			type: "image/png",
		});
		await expect(playbackImageDataUrl(file)).rejects.toThrow(
			"400 KB or smaller",
		);
	});

	it("applies explicit topology defaults and clears hidden actions", () => {
		expect(
			normalizePlaybackTopology(
				{ ...base, button_count: undefined, has_fader: undefined },
				2,
				false,
			),
		).toMatchObject({
			footprint: { type: "normal" },
			button_count: 2,
			has_fader: false,
			buttons: ["go_minus", "go", "none"],
		});
	});

	it("preserves dormant expanded controls when the ordinary surface is smaller", () => {
		const footprint = {
			type: "wider" as const,
			right_buttons: ["go", "pause", "flash"] as PlaybackDefinition["buttons"],
			right_fader: "x_fade" as const,
		};
		expect(
			normalizePlaybackTopology(
				{ ...base, footprint, button_count: undefined, has_fader: undefined },
				1,
				false,
			),
		).toMatchObject({ footprint });
	});

	it("authors every control in a mutually exclusive wider footprint", () => {
		show();
		fireEvent.click(screen.getByRole("button", { name: "Layout" }));
		fireEvent.click(screen.getByRole("radio", { name: "Wider" }));

		expect(selectTrigger("Right top button")).toHaveTextContent("GO −");
		expect(selectTrigger("Right middle button")).toHaveTextContent("GO +");
		expect(selectTrigger("Right bottom button")).toHaveTextContent("Flash");
		expect(selectTrigger("Right fader")).toHaveTextContent("Master");
		expect(
			screen.queryByText("Additional upper button"),
		).not.toBeInTheDocument();
	});

	it.each([
		[1, ["Right top button"]],
		[2, ["Right top button", "Right middle button"]],
	] as const)("offers only %i wider button control(s) for that Playback topology", (buttonCount, visibleLabels) => {
		show({ ...base, button_count: buttonCount });
		fireEvent.click(screen.getByRole("button", { name: "Layout" }));
		fireEvent.click(screen.getByRole("radio", { name: "Wider" }));

		for (const label of visibleLabels)
			expect(selectTrigger(label)).toBeInTheDocument();
		expect(
			screen.queryByLabelText("Right bottom button"),
		).not.toBeInTheDocument();
		if (buttonCount === 1)
			expect(
				screen.queryByLabelText("Right middle button"),
			).not.toBeInTheDocument();
	});
});

describe("canonical Dynamic assignment choices", () => {
	it("offers bound Dynamics and writes only their canonical binding", async () => {
		mocks.dynamics.push({
			id: "targetless",
			body: {
				pool_number: 2,
				name: "Unbound template",
				target_binding: { type: "targetless" },
			},
		});
		show();
		fireEvent.click(screen.getByRole("radio", { name: "Dynamic" }));
		expect(screen.getByRole("radio", { name: "1 Pulse" })).toBeInTheDocument();
		expect(screen.queryByText("Unbound template")).not.toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Behavior" }));
		expect(screen.queryByLabelText("Target scope")).not.toBeInTheDocument();
		expect(screen.getByText("Live Group group-1")).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		await waitFor(() => expect(mocks.savePlaybackSlot).toHaveBeenCalledOnce());
		expect(
			mocks.savePlaybackSlot.mock.calls[0][2].target.assignment,
		).toMatchObject({
			dynamic_id: "dynamic-1",
			target_scope: null,
			embedded_fallback: {
				target_binding: { type: "live_group", group_id: "group-1" },
			},
		});
	});
	it("does not offer targetless Dynamics and explains how to bind them", () => {
		mocks.dynamics[0].body.target_binding.type = "targetless";
		show();
		expect(
			screen.queryByRole("radio", { name: "Dynamic" }),
		).not.toBeInTheDocument();
		expect(
			screen.getByText(
				"Bind targets in a Dynamic before assigning it to a Playback.",
			),
		).toBeInTheDocument();
		expect(mocks.savePlaybackSlot).not.toHaveBeenCalled();
	});
});

describe("canonical Macro and Timecode Playback functions", () => {
	it.each([
		["macro", ["go", "none", "none"]],
		["timecode", ["go", "pause", "off"]],
	] as const)("keeps the %s family and normal button defaults", (type, buttons) => {
		const target =
			type === "macro"
				? ({ type, macro_id: "macro-1" } as const)
				: ({ type, timecode_id: "timecode-1" } as const);
		const configured = {
			...base,
			target,
			buttons: [...buttons] as PlaybackDefinition["buttons"],
		};
		expect(familyFromTarget(type)).toBe(type);
		expect(
			playbackLayoutOptions(configured).map((option) => option.value),
		).toEqual(type === "macro" ? ["go"] : ["go", "pause", "off"]);
	});
});

it.each([
	"macro",
	"timecode",
] as const)("chooses an authoritative %s object and saves exact canonical defaults", async (type) => {
	show();
	fireEvent.click(
		screen.getByRole("radio", {
			name: type === "macro" ? "Macro" : "Timecode",
		}),
	);
	fireEvent.click(
		screen.getByRole("radio", {
			name: type === "macro" ? "2 Open sequence" : "3 Opening transport",
		}),
	);
	fireEvent.click(screen.getByRole("button", { name: "Apply" }));
	await waitFor(() =>
		expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(
			2,
			4,
			expect.objectContaining({
				target:
					type === "macro"
						? { type, macro_id: "macro-1" }
						: { type, timecode_id: "timecode-1" },
				buttons:
					type === "macro" ? ["go", "none", "none"] : ["go", "pause", "off"],
			}),
		),
	);
});
it("prevents saving a Timecode target removed from the authoritative options", () => {
	const original = mocks.timecodes;
	try {
		mocks.timecodes = [];
		show({
			...base,
			target: { type: "timecode", timecode_id: "removed" },
			buttons: ["go", "pause", "off"],
		});
		fireEvent.change(screen.getByLabelText("Playback name"), {
			target: { value: "Changed" },
		});
		expect(screen.getByRole("button", { name: "Apply" })).toBeDisabled();
	} finally {
		mocks.timecodes = original;
	}
});
it("masks hidden Timecode buttons on both halves when reducing button count", () => {
	const normalized = normalizePlaybackTopology(
		{
			...base,
			target: { type: "timecode", timecode_id: "timecode-1" },
			buttons: ["go", "pause", "off"],
			button_count: 1,
			footprint: {
				type: "wider",
				right_buttons: ["go", "pause", "off"],
				right_fader: "master",
			},
		},
		3,
		true,
	);
	expect(normalized.buttons).toEqual(["go", "none", "none"]);
	expect(normalized.footprint).toMatchObject({
		right_buttons: ["go", "none", "none"],
	});
});

it.each(["macro", "timecode"] as const)(
	"removes an inherited Cue fader and preserves the %s Wider buttons",
	(type) => {
		const configured = withFunctionDefaults(
			{
				...base,
				footprint: {
					type: "wider",
					right_buttons: base.buttons,
					right_fader: "master",
				},
			},
			type,
			"cue-1",
			"group-1",
			undefined,
			{ macroId: "macro-1", timecodeId: "timecode-1" },
		);
		expect(configured.has_fader).toBe(false);
		expect(configured.footprint).toMatchObject({
			type: "wider",
			right_buttons: configured.buttons,
		});
	},
);

it.each(["macro", "timecode"] as const)(
	"shows only button choices for an expanded %s layout",
	(type) => {
		show({
			...base,
			target:
				type === "macro"
					? { type, macro_id: "macro-1" }
					: { type, timecode_id: "timecode-1" },
			buttons:
				type === "macro" ? ["go", "none", "none"] : ["go", "pause", "off"],
			footprint: {
				type: "wider",
				right_buttons: ["go", "none", "none"],
				right_fader: "master",
			},
		});
		fireEvent.click(screen.getByRole("button", { name: "Layout" }));
		expect(
			screen.queryByText("Fader", { selector: "label", exact: true }),
		).not.toBeInTheDocument();
		expect(
			screen.queryByText("Right fader", { selector: "label", exact: true }),
		).not.toBeInTheDocument();
		expect(
			screen.getByText("Macro and Timecode Playbacks use buttons only."),
		).toBeInTheDocument();
		expect(selectTrigger("Top button")).toHaveTextContent("GO");
		expect(selectTrigger("Right top button")).toHaveTextContent("GO");
	},
);
