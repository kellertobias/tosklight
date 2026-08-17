import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { MediaPaneSurface, SafePreviewImage } from "./MediaPaneSurface";
import type { MediaPaneModel, MediaPaneUiCallbacks } from "./mediaPaneModel";

describe("MediaPaneSurface control state", () => {
	vi.stubGlobal(
		"ResizeObserver",
		class {
			observe() {}
			disconnect() {}
		},
	);

	it("keeps preview feedback below a full-width master picture", () => {
		const view = renderSurface({
			kind: "unsupported",
			capability: "preview",
			detail: "No composite source",
		});
		const master = view.getByRole("button", { name: "Master output" });
		const picture = view.getByTestId("master-output-picture");
		const feedback = view.getByText(
			"preview unsupported · No composite source",
		);

		expect(picture).toHaveClass("media-composite-picture", "is-empty");
		expect(picture).toHaveStyle({ aspectRatio: "16 / 9" });
		expect(master).toHaveAttribute("data-preview-state", "unsupported");
		expect(master).toHaveClass("is-full-width", "media-composite-frame");
		expect(picture.compareDocumentPosition(feedback)).toBe(
			Node.DOCUMENT_POSITION_FOLLOWING,
		);
		expect(
			view.container.querySelector(".media-composite-info"),
		).toContainElement(feedback);
	});

	it("shows a music note for the Internal Audio Player master and its layers", () => {
		const view = renderSurface(
			{ kind: "audio", detail: "003/012.wav" },
			[
				{
					id: "player-head",
					number: "1",
					name: "Player",
					status: "online",
					statusLabel: "Playing",
					audio: { volumeLabel: "42%", sourceLabel: "003 / 012" },
					liveSourceLabel: "003/012.wav",
				},
			],
			{ hasCitpEndpoint: false },
		);
		const surface = within(view.container);
		const picture = surface.getByTestId("master-output-picture");
		expect(
			within(picture).getByRole("img", { name: "Audio output" }),
		).toBeInTheDocument();
		expect(
			within(
				view.container.querySelector(".media-composite-info") as HTMLElement,
			).getByText("003/012.wav"),
		).toBeInTheDocument();
		expect(surface.getByText("Master audio output")).toBeInTheDocument();
		expect(surface.queryByText(/CITP is not configured/)).toBeNull();

		const thumbnail = view.container.querySelector(".media-layer-thumbnail");
		expect(thumbnail).toHaveClass("is-audio");
		const values = within(thumbnail as HTMLElement).getByText("42% · 003 / 012");
		const note = within(thumbnail as HTMLElement).getByRole("img", {
			name: "Audio 42% · 003 / 012",
		});
		expect(values.compareDocumentPosition(note)).toBe(
			Node.DOCUMENT_POSITION_FOLLOWING,
		);
		view.unmount();
	});

	it("projects optional authoritative output facts into the window title", () => {
		const view = renderSurface({ kind: "ready" }, [], {}, vi.fn(), vi.fn(), {
			primary: "192.0.2.10 · DMX U1 A1",
			secondary: "2 library items",
		});

		expect(view.getByText("192.0.2.10 · DMX U1 A1")).toBeInTheDocument();
		expect(view.getByText("2 library items")).toBeInTheDocument();
	});

	it("adapts the master height to the advertised output aspect ratio", () => {
		const view = renderSurface({
			kind: "ready",
			outputSize: { width: 1024, height: 768 },
		});

		expect(
			view.container.querySelector('[data-testid="master-output-picture"]'),
		).toHaveStyle({ aspectRatio: "1024 / 768" });
	});

	it("keeps the loaded CITP frame underneath until its replacement has loaded", () => {
		const failure = vi.fn();
		const view = render(
			<SafePreviewImage
				src="/preview?frame=1"
				alt="Program"
				onFailure={failure}
			/>,
		);
		const firstLoader = view.container.querySelector("img.is-pending");
		expect(firstLoader).toHaveAttribute("hidden");
		fireEvent.load(firstLoader as HTMLImageElement);
		expect(view.container.querySelector("img.is-current")).toHaveAttribute(
			"src",
			"/preview?frame=1",
		);

		view.rerender(
			<SafePreviewImage
				src="/preview?frame=2"
				alt="Program"
				onFailure={failure}
			/>,
		);
		expect(view.container.querySelector("img.is-current")).toHaveAttribute(
			"src",
			"/preview?frame=1",
		);
		const secondLoader = view.container.querySelector("img.is-pending");
		expect(secondLoader).toHaveAttribute("src", "/preview?frame=2");
		expect(secondLoader).toHaveAttribute("hidden");
		view.rerender(
			<SafePreviewImage
				src="/preview?frame=3"
				alt="Program"
				onFailure={failure}
			/>,
		);
		expect(view.container.querySelector("img.is-pending")).toHaveAttribute(
			"src",
			"/preview?frame=2",
		);
		fireEvent.load(secondLoader as HTMLImageElement);
		expect(view.container.querySelector("img.is-pending")).toHaveAttribute(
			"src",
			"/preview?frame=3",
		);
		fireEvent.load(
			view.container.querySelector("img.is-pending") as HTMLImageElement,
		);

		const visibleFrames = view.container.querySelectorAll(
			"img.is-previous, img.is-current",
		);
		expect(visibleFrames).toHaveLength(2);
		expect(visibleFrames[0]).toHaveAttribute("src", "/preview?frame=2");
		expect(visibleFrames[1]).toHaveAttribute("src", "/preview?frame=3");
		expect(visibleFrames[1]).toHaveClass("is-current");
		expect(failure).not.toHaveBeenCalled();

		view.rerender(
			<SafePreviewImage
				src="/preview?frame=4"
				alt="Program"
				onFailure={failure}
			/>,
		);
		fireEvent.error(
			view.container.querySelector("img.is-pending") as HTMLImageElement,
		);
		expect(failure).toHaveBeenCalledOnce();
		expect(
			view.container.querySelectorAll("img.is-previous, img.is-current"),
		).toHaveLength(2);
		expect(view.container.querySelector("img.is-current")).toHaveAttribute(
			"src",
			"/preview?frame=3",
		);
	});

	it("normalizes a stale control-tab ID instead of crashing the surface", () => {
		const view = renderSurface({ kind: "ready" }, [], {
			rightPaneVisible: true,
			selectedControlSectionId: "output",
			controlSections: [{ id: "playback", label: "Playback", controls: [] }],
		});

		expect(
			within(view.container).getByRole("tab", { name: "Playback" }),
		).toHaveAttribute("aria-selected", "true");
	});

	it("shows an honest black empty output instead of a dummy image", () => {
		const view = renderSurface({ kind: "ready" });

		expect(view.container.querySelector("img")).not.toBeInTheDocument();
		expect(
			view.container.querySelector('[data-testid="master-output-picture"]'),
		).toHaveClass("is-empty");
		expect(view.container).toHaveTextContent("No output · black");
	});

	it("marks an unsupported layer preview as an empty placeholder", () => {
		const view = renderSurface({ kind: "ready" }, [
			{
				id: "layer-1",
				number: "1",
				name: "Layer 1",
				status: "unsupported",
			},
		]);

		const thumbnail = view.container.querySelector(".media-layer-thumbnail");
		expect(thumbnail).toHaveClass("is-empty");
		expect(thumbnail?.querySelector("img")).not.toBeInTheDocument();
		expect(view.getByText("Dimmer 100%")).toBeInTheDocument();
	});

	it("keeps live preview transport failures visible on both master and layer", () => {
		const view = renderSurface(
			{ kind: "ready", imageSrc: "/program-preview" },
			[
				{
					id: "layer-1",
					number: "1",
					name: "Layer 1",
					status: "online",
					thumbnailSrc: "/layer-preview",
				},
			],
		);
		const images = view.container.querySelectorAll("img");
		expect([...images].every((image) => image.hidden)).toBe(true);
		fireEvent.error(images[0]);
		fireEvent.error(images[1]);
		const retainedHiddenLoaders = view.container.querySelectorAll("img");
		expect(retainedHiddenLoaders).toHaveLength(2);
		expect([...retainedHiddenLoaders].every((image) => image.hidden)).toBe(
			true,
		);

		expect(
			within(view.container).getByText("Live preview could not be loaded."),
		).toHaveAttribute("role", "alert");
		expect(
			within(view.container).getByText(
				"Live layer preview could not be loaded.",
			),
		).toHaveAttribute("role", "alert");
	});

	it("shows only the no-patch empty state when no media server is patched", () => {
		const onOpenPatch = vi.fn();
		const view = renderSurface(
			{ kind: "missing_patch", detail: "No media server is patched." },
			[],
			{
				hasPatchedServer: false,
				hasCitpEndpoint: false,
				servers: [
					{
						id: "",
						name: "No media server is patched",
						statusLabel: "Missing patch",
						disabled: true,
					},
				],
				selectedServerId: "",
			},
			vi.fn(),
			vi.fn(),
			undefined,
			{ onOpenPatch },
		);
		const surface = within(view.container);

		expect(surface.getByRole("status")).toHaveTextContent(
			"No media server is patched",
		);
		expect(surface.queryByRole("button", { name: "Media server" })).toBeNull();
		expect(surface.queryByLabelText("Media layers")).toBeNull();
		expect(surface.queryByLabelText("Media library browser")).toBeNull();
		const openPatch = surface.getByRole("button", { name: "Open Patch" });
		expect(openPatch.parentElement).toHaveClass("ui-window-action-group");
		fireEvent.click(openPatch);
		expect(onOpenPatch).toHaveBeenCalledOnce();
	});

	it("keeps logical layers selectable when the patch has no CITP endpoint", async () => {
		const onSelectLayer = vi.fn();
		const view = renderSurface(
			{
				kind: "offline",
				detail: "No CITP Media Server is available.",
			},
			[
				{
					id: "layer-1",
					number: "1",
					name: "Layer 1",
					status: "unsupported",
					statusLabel: "No advertised mapping",
				},
			],
			{ hasCitpEndpoint: false },
			vi.fn(),
			onSelectLayer,
		);
		const surface = within(view.container);

		expect(surface.getByRole("status")).toHaveTextContent(
			"CITP is not configured",
		);
		await userEvent.click(
			surface.getByRole("button", {
				name: "Layer 1 Layer 1 · No advertised mapping",
			}),
		);
		expect(onSelectLayer).toHaveBeenCalledWith("layer-1");
	});

	it("shows every patched server in the left selector with its fixture number", async () => {
		const onSelectServer = vi.fn();
		const view = renderSurface(
			{ kind: "unsupported", capability: "preview", detail: "No preview" },
			[],
			{
				servers: [
					{
						id: "video",
						name: "Video Server",
						fixtureLabel: "1001",
						statusLabel: "Online",
					},
					{
						id: "internal",
						name: "Internal Audio",
						fixtureLabel: "1002",
						statusLabel: "Not configured",
					},
				],
				selectedServerId: "video",
			},
			vi.fn(),
			vi.fn(),
			undefined,
			{ onSelectServer },
		);
		const shortcuts = within(view.container).getByLabelText("Media servers");
		expect(shortcuts).toBeInTheDocument();
		expect(shortcuts.querySelector(".pool-window-grid")).toHaveClass(
			"media-server-pool",
		);
		expect(
			within(shortcuts).getByRole("button", {
				name: /1001\s*Video Server/,
			}),
		).toHaveClass("preset-card", "selected");
		expect(
			within(shortcuts).getAllByRole("button", { hidden: true }),
		).toHaveLength(6);
		expect(
			within(shortcuts).getAllByRole("button", { name: /Empty/ }),
		).toHaveLength(4);
		expect(
			within(shortcuts).getAllByRole("button", { name: /Empty/ })[0],
		).toBeDisabled();
		await userEvent.click(
			within(shortcuts).getByRole("button", {
				name: /1002\s*Internal Audio/,
			}),
		);
		expect(onSelectServer).toHaveBeenCalledWith("internal");
	});

	it("keeps a single server in the left selector and removes the title dropdown", () => {
		const view = renderSurface({ kind: "ready" }, [], {
			servers: [
				{
					id: "server",
					name: "ToskLight Media Server",
					fixtureLabel: "1001",
					statusLabel: "Online",
				},
			],
		});
		const selector = within(view.container).getByLabelText("Media servers");
		expect(selector).toHaveTextContent("ToskLight Media Server");
		expect(selector).toHaveTextContent("1001");
		expect(
			within(view.container).queryByRole("button", { name: "Media server" }),
		).toBeNull();
	});

	it("does not invoke a disabled choice control", async () => {
		const onChangeControl = vi.fn();
		const model: MediaPaneModel = {
			hasPatchedServer: true,
			hasCitpEndpoint: true,
			servers: [{ id: "server", name: "Server", statusLabel: "Online" }],
			selectedServerId: "server",
			selectedLayerId: null,
			preview: { kind: "unsupported", capability: "preview", detail: "None" },
			layers: [],
			browserMode: "media",
			showSourceFilters: false,
			maskBrowser: "hidden",
			libraryFolders: [],
			libraryFiles: [],
			draftFolderId: "1",
			draftFileId: null,
			liveSelection: {
				folderId: null,
				fileId: null,
				maskFolderId: null,
				maskFileId: null,
			},
			draftSelection: {
				folderId: null,
				fileId: null,
				maskFolderId: null,
				maskFileId: null,
			},
			liveSelectionLabel: "None",
			draftSelectionLabel: "None",
			controlSections: [
				{
					id: "playback",
					label: "Playback",
					controls: [
						{
							id: "mode",
							kind: "choice",
							label: "Play mode",
							value: "loop",
							options: [
								{ value: "loop", label: "Loop" },
								{ value: "pause", label: "Pause" },
							],
							disabled: true,
						},
					],
				},
			],
			selectedControlSectionId: "playback",
			mainSectionId: "playback",
			rightPaneVisible: false,
		};
		render(
			<MediaPaneSurface
				model={model}
				onSelectServer={vi.fn()}
				onSelectLayer={vi.fn()}
				onSelectBrowserMode={vi.fn()}
				onBrowseItem={vi.fn()}
				onSelectControlSection={vi.fn()}
				onChangeControl={onChangeControl}
				onSetRightPaneVisible={vi.fn()}
			/>,
		);
		const pause = screen.getByRole("radio", { name: "Pause" });
		expect(pause).toBeDisabled();
		await userEvent.click(pause, { pointerEventsCheck: 0 });
		expect(onChangeControl).not.toHaveBeenCalled();
	});

	it("opens play mode in a grouped modal with visible quick actions", async () => {
		const onChangeControl = vi.fn();
		const view = renderSurface(
			{ kind: "ready" },
			[],
			{
				controlSections: [
					{
						id: "playback",
						label: "Playback",
						controls: [
							{
								id: "play-mode",
								kind: "choice",
								label: "Play mode",
								value: "loop",
								options: ["loop", "once", "reverse", "bounce", "stop"].map(
									(value) => ({ value, label: value }),
								),
								quickActions: [
									{ value: "stop", label: "Stop" },
									{ value: "once", label: "Play" },
									{ value: "loop", label: "Play looped" },
								],
							},
						],
					},
				],
				selectedControlSectionId: "playback",
				mainSectionId: "playback",
			},
			onChangeControl,
		);

		const surface = within(view.container);
		expect(surface.queryByRole("radiogroup", { name: "Play mode" })).toBeNull();
		await userEvent.click(surface.getByRole("button", { name: "Play mode" }));
		const dialog = screen.getByRole("dialog", { name: "Choose Play mode" });
		expect(within(dialog).getByText("Continuous playback")).toBeInTheDocument();
		await userEvent.click(
			within(dialog).getByRole("button", { name: /bounce/iu }),
		);
		expect(onChangeControl).toHaveBeenCalledWith("play-mode", "bounce");
		await userEvent.click(surface.getByRole("button", { name: "Stop" }));
		expect(onChangeControl).toHaveBeenCalledWith("play-mode", "stop");
	});

	it("opens speed in a grouped modal and selects an exact DMX band", async () => {
		const onChangeControl = vi.fn();
		const view = renderSurface(
			{ kind: "ready" },
			[],
			{
				controlSections: [
					{
						id: "playback",
						label: "Playback",
						controls: [
							{
								id: "speed",
								kind: "value",
								label: "Speed",
								value: 127,
								display: "1×",
							},
						],
					},
				],
				selectedControlSectionId: "playback",
				mainSectionId: "playback",
			},
			onChangeControl,
		);

		await userEvent.click(
			within(view.container).getByRole("button", { name: "Speed" }),
		);
		const dialog = screen.getByRole("dialog", { name: "Choose Speed" });
		expect(within(dialog).getByText("Slower")).toBeInTheDocument();
		expect(within(dialog).getByText("Faster")).toBeInTheDocument();
		await userEvent.click(within(dialog).getByRole("button", { name: /^2×/u }));
		expect(onChangeControl).toHaveBeenCalledWith("speed", 135);
		expect(screen.queryByRole("dialog", { name: "Choose Speed" })).toBeNull();
	});

	it("offers a reset beside each editable media control", async () => {
		const onResetControl = vi.fn();
		renderSurface(
			{ kind: "ready" },
			[],
			{
				controlSections: [
					{
						id: "frame",
						label: "Frame",
						controls: [
							{
								id: "media.layer.scale.x",
								kind: "value",
								label: "Scale X",
								value: 1,
							},
						],
					},
				],
				selectedControlSectionId: "frame",
				mainSectionId: "frame",
			},
			vi.fn(),
			vi.fn(),
			undefined,
			{ onResetControl },
		);

		await userEvent.click(
			screen.getByRole("button", { name: "Reset Scale X" }),
		);
		expect(onResetControl).toHaveBeenCalledWith("media.layer.scale.x");
	});

	it("groups effect amounts into four selectable effect slots", async () => {
		const onChangeControl = vi.fn();
		const view = renderSurface(
			{ kind: "ready" },
			[],
			{
				controlSections: [
					{
						id: "effects",
						label: "Effects",
						controls: [1, 2, 3, 4].map((slot) => ({
							id: `media.layer.effect.${slot}`,
							kind: "value" as const,
							label: `Effect ${slot}`,
							value: slot * 10,
						})),
					},
				],
				selectedControlSectionId: "effects",
				mainSectionId: "effects",
			},
			onChangeControl,
		);
		const surface = within(view.container);
		const tabs = surface.getByRole("tablist", { name: "Effect slot" });
		expect(within(tabs).getAllByRole("tab")).toHaveLength(4);
		expect(surface.getByText("10%")).toBeInTheDocument();
		expect(surface.queryByText("20%")).toBeNull();

		await userEvent.click(within(tabs).getByRole("tab", { name: "Effect 2" }));
		expect(surface.queryByText("10%")).toBeNull();
		expect(surface.getByText("20%")).toBeInTheDocument();
		expect(surface.getByText("Amount")).toBeInTheDocument();
	});

	it("opens the selected native Media Server controls and content in place", async () => {
		let view = renderSurface({ kind: "ready" }, [], {
			nativeManagementUrl: "http://192.0.2.44:8080",
		});
		await userEvent.click(
			within(view.container).getByRole("button", { name: "Settings" }),
		);
		const showContent = screen
			.getAllByRole("button", { name: "Show content" })
			.at(-1);
		expect(showContent).toBeDefined();
		await userEvent.click(showContent as HTMLElement);
		const content = within(view.container).getByTitle("Media Server content");
		expect(content).toHaveAttribute("src", "http://192.0.2.44:8080/library");

		view.unmount();
		view = renderSurface({ kind: "ready" }, [], {
			nativeManagementUrl: "http://192.0.2.44:8080",
		});
		await userEvent.click(
			within(view.container).getByRole("button", { name: "Settings" }),
		);
		const showControls = screen
			.getAllByRole("button", { name: "Show controls" })
			.at(-1);
		expect(showControls).toBeDefined();
		await userEvent.click(showControls as HTMLElement);
		expect(
			within(view.container).getByTitle("Media Server controls"),
		).toHaveAttribute("src", "http://192.0.2.44:8080/media");
	});

	it("shows native source filters and prevents Master content browsing", async () => {
		const onSelectSourceFilter = vi.fn();
		const onBrowseItem = vi.fn();
		renderSurface(
			{ kind: "ready" },
			[],
			{
				selectedLayerId: "master",
				sourceFilter: "media",
				showSourceFilters: true,
				maskBrowser: "supported",
				rightPaneVisible: true,
				libraryFolders: [{ id: "1", kind: "folder", name: "Looks" }],
				libraryFiles: [{ id: "1", kind: "file", name: "Loop" }],
			},
			vi.fn(),
			vi.fn(),
			undefined,
			{ onSelectSourceFilter, onBrowseItem },
		);
		for (const label of ["Media", "VIS", "Text"])
			expect(screen.getByRole("tab", { name: label })).toBeInTheDocument();
		await userEvent.click(screen.getByRole("tab", { name: "VIS" }));
		expect(onSelectSourceFilter).toHaveBeenCalledWith("visualizers");
		const folder = screen.getByRole("button", { name: /001Looks/iu });
		expect(folder).toBeDisabled();
		await userEvent.click(folder, { pointerEventsCheck: 0 });
		expect(onBrowseItem).not.toHaveBeenCalled();
	});

	it("hides Media, VIS and Text for a non-native media server", () => {
		const view = renderSurface(
			{ kind: "ready" },
			[],
			{ sourceFilter: "media", showSourceFilters: false },
			vi.fn(),
			vi.fn(),
			undefined,
			{ onSelectSourceFilter: vi.fn() },
		);
		const surface = within(view.container);
		for (const label of ["Media", "VIS", "Text"])
			expect(surface.queryByRole("tab", { name: label })).toBeNull();
	});

	it("places source filters before Content and Mask at both pane widths", () => {
		for (const rightPaneVisible of [false, true]) {
			const view = renderSurface(
				{ kind: "ready" },
				[],
				{
					sourceFilter: "media",
					showSourceFilters: true,
					maskBrowser: "supported",
					rightPaneVisible,
					controlSections: [
						{ id: "playback", label: "Playback", controls: [] },
					],
					selectedControlSectionId: "playback",
				},
				vi.fn(),
				vi.fn(),
				undefined,
				{ onSelectSourceFilter: vi.fn() },
			);
			const surface = within(view.container);
			const media = surface.getByRole("tab", { name: "Media" });
			const content = surface.getByRole("tab", { name: "Content" });
			expect(
				media.compareDocumentPosition(content) &
					Node.DOCUMENT_POSITION_FOLLOWING,
			).toBeTruthy();
			view.unmount();
		}
	});

	it("marks only Mask browsing with the shared folder and file accent scope", () => {
		const mask = renderSurface({ kind: "ready" }, [], {
			browserMode: "mask",
			maskBrowser: "supported",
			libraryFolders: [{ id: "1", kind: "folder", name: "Masks" }],
			libraryFiles: [{ id: "1", kind: "file", name: "Circle" }],
		});
		const maskBrowser = within(mask.container).getByLabelText(
			"Media library browser",
		);
		expect(maskBrowser).toHaveClass("is-mask");
		expect(maskBrowser).toHaveAttribute("data-browser-mode", "mask");
		mask.unmount();

		const content = renderSurface({ kind: "ready" });
		const contentBrowser = within(content.container).getByLabelText(
			"Media library browser",
		);
		expect(contentBrowser).toHaveClass("is-content");
		expect(contentBrowser).not.toHaveClass("is-mask");
	});

	it("starts Content and Mask file pools with a selectable 000 clear entry", async () => {
		for (const browserMode of ["media", "mask"] as const) {
			const onBrowseItem = vi.fn();
			const view = renderSurface(
				{ kind: "ready" },
				[],
				{
					browserMode,
					maskBrowser: "supported",
					libraryFolders: [{ id: "1", kind: "folder", name: "Looks" }],
					libraryFiles: [{ id: "1", kind: "file", name: "Loop" }],
				},
				vi.fn(),
				vi.fn(),
				undefined,
				{ onBrowseItem },
			);
			const files = within(
				within(view.container).getByLabelText(
					browserMode === "mask" ? "Mask files" : "Media files",
				),
			);
			const clear = files.getByRole("button", {
				name: /000No file selected/iu,
			});
			expect(clear).toHaveAttribute("data-pool-position", "0");
			expect(clear).not.toHaveClass("empty");
			await userEvent.click(clear);
			expect(onBrowseItem).toHaveBeenCalledWith(
				browserMode,
				expect.objectContaining({ id: "0", kind: "file" }),
			);
			view.unmount();
		}
	});

	it("marks the selected media folder and file with the shared selected state", () => {
		const view = renderSurface({ kind: "ready" }, [], {
			libraryFolders: [{ id: "1", kind: "folder", name: "Looks" }],
			libraryFiles: [{ id: "7", kind: "file", name: "Loop" }],
			draftFolderId: "1",
			draftFileId: "7",
		});

		const browser = within(view.container).getByLabelText(
			"Media library browser",
		);
		expect(
			within(browser).getByRole("button", { name: /001Looks/iu }),
		).toHaveClass("selected");
		expect(
			within(browser).getByRole("button", { name: /007Loop/iu }),
		).toHaveClass("selected");
	});
});

function renderSurface(
	preview: MediaPaneModel["preview"],
	layers: MediaPaneModel["layers"] = [],
	overrides: Partial<MediaPaneModel> = {},
	onChangeControl = vi.fn(),
	onSelectLayer = vi.fn(),
	info?: { primary: string; secondary?: string },
	callbacks: Partial<MediaPaneUiCallbacks> = {},
) {
	const model: MediaPaneModel = {
		hasPatchedServer: true,
		hasCitpEndpoint: true,
		servers: [{ id: "server", name: "Server", statusLabel: "Online" }],
		selectedServerId: "server",
		selectedLayerId: null,
		preview,
		layers,
		browserMode: "media",
		showSourceFilters: false,
		maskBrowser: "hidden",
		libraryFolders: [],
		libraryFiles: [],
		draftFolderId: "1",
		draftFileId: null,
		liveSelection: {
			folderId: null,
			fileId: null,
			maskFolderId: null,
			maskFileId: null,
		},
		draftSelection: {
			folderId: null,
			fileId: null,
			maskFolderId: null,
			maskFileId: null,
		},
		liveSelectionLabel: "None",
		draftSelectionLabel: "None",
		controlSections: [],
		selectedControlSectionId: "",
		mainSectionId: "content",
		rightPaneVisible: false,
		...overrides,
	};
	return render(
		<MediaPaneSurface
			model={model}
			info={info}
			onOpenPatch={callbacks.onOpenPatch}
			onSelectServer={callbacks.onSelectServer ?? vi.fn()}
			onSelectLayer={onSelectLayer}
			onSelectBrowserMode={callbacks.onSelectBrowserMode ?? vi.fn()}
			onSelectSourceFilter={callbacks.onSelectSourceFilter}
			onBrowseItem={callbacks.onBrowseItem ?? vi.fn()}
			onSelectControlSection={vi.fn()}
			onChangeControl={onChangeControl}
			onResetControl={callbacks.onResetControl}
			onSetRightPaneVisible={vi.fn()}
		/>,
	);
}
