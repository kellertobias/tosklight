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

	it("shows touch server shortcuts only for multiple patched media servers", async () => {
		const onSelectServer = vi.fn();
		const view = renderSurface(
			{ kind: "unsupported", capability: "preview", detail: "No preview" },
			[],
			{
				servers: [
					{ id: "video", name: "Video Server", statusLabel: "Online" },
					{
						id: "internal",
						name: "Internal Audio",
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
		const shortcuts = within(view.container).getByLabelText(
			"Media server shortcuts",
		);
		expect(shortcuts).toBeInTheDocument();
		expect(
			within(shortcuts).getByRole("button", { name: /Video Server\s*Online/ }),
		).toHaveAttribute("aria-pressed", "true");
		await userEvent.click(
			within(shortcuts).getByRole("button", {
				name: /Internal Audio\s*Not configured/,
			}),
		);
		expect(onSelectServer).toHaveBeenCalledWith("internal");
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

	it("collapses more than four modes into a dropdown with visible quick actions", async () => {
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
		expect(surface.getByRole("button", { name: "loop" })).toHaveAttribute(
			"aria-haspopup",
			"listbox",
		);
		await userEvent.click(surface.getByRole("button", { name: "Stop" }));
		expect(onChangeControl).toHaveBeenCalledWith("play-mode", "stop");
	});

	it("shows source filters and prevents Master content browsing", async () => {
		const onSelectSourceFilter = vi.fn();
		const onBrowseItem = vi.fn();
		renderSurface(
			{ kind: "ready" },
			[],
			{
				selectedLayerId: "master",
				sourceFilter: "media",
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

	it("places source filters before Content and Mask at both pane widths", () => {
		for (const rightPaneVisible of [false, true]) {
			const view = renderSurface(
				{ kind: "ready" },
				[],
				{
					sourceFilter: "media",
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
			onSetRightPaneVisible={vi.fn()}
		/>,
	);
}
