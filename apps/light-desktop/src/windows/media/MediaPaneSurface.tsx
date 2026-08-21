import {
	Button,
	ColorPickerField,
	GroupedSelectionField,
	HorizontalFader,
	MultiValueToggle,
	SelectField,
	SwitchField,
} from "@tosklight/ui";
import { PoolGrid, type PoolSlotViewModel } from "@tosklight/ui/pools";
import { WindowFrame } from "@tosklight/ui/window-kit";
import { Fragment, type ReactNode, useEffect, useRef, useState } from "react";
import type {
	MediaBrowserMode,
	MediaControlSection,
	MediaLibraryItem,
	MediaPaneLayer,
	MediaPaneModel,
	MediaPaneSurfaceProps,
	MediaPreviewState,
	MediaSecondaryControl,
	MediaSourceFilter,
} from "./mediaPaneModel";
import {
	MediaCompositePreview,
	MusicNote,
	PreviewStateMessage,
	SafePreviewImage,
} from "./MediaPanePreview";
import "./MediaPaneSurface.css";

export type {
	MediaBrowserMode,
	MediaBrowseSelection,
	MediaControlSection,
	MediaLibraryItem,
	MediaPaneLayer,
	MediaPaneModel,
	MediaPaneServer,
	MediaPaneSurfaceProps,
	MediaPaneUiCallbacks,
	MediaPreviewState,
	MediaSecondaryControl,
	MediaSourceFilter,
} from "./mediaPaneModel";

export function MediaPaneSurface({
	model,
	compact = false,
	title = "Media",
	info,
	headerAction,
	onOpenPatch,
	onSelectServer,
	onSelectLayer,
	onSelectBrowserMode,
	onSelectSourceFilter,
	onBrowseItem,
	onSelectControlSection,
	onChangeControl,
	onResetControl,
	onSetRightPaneVisible,
}: MediaPaneSurfaceProps) {
	const [nativeManagementView, setNativeManagementView] = useState<
		"controls" | "content" | null
	>(null);
	useEffect(() => setNativeManagementView(null), [model.nativeManagementUrl]);
	const hasPatchedServer = model.servers.some(
		(server) => server.id.trim() !== "",
	);
	const patchedServers = model.servers.filter(
		(server) => server.id.trim() !== "" && !server.disabled,
	);
	const serverPoolSlots: PoolSlotViewModel<string>[] = [
		...patchedServers.map<PoolSlotViewModel<string>>((server, position) => ({
			id: server.id,
			position,
			card: {
				number: server.fixtureLabel ?? "—",
				primary: server.name,
				kind: "preset" as const,
				states: server.id === model.selectedServerId ? ["selected"] : [],
			},
		})),
		...Array.from(
			{ length: Math.max(0, 6 - patchedServers.length) },
			(_, emptyIndex): PoolSlotViewModel<string> => {
				const position = patchedServers.length + emptyIndex;
				return {
					id: `empty-server-${position}`,
					position,
					card: {
						number: position + 1,
						primary: "Empty",
						kind: "preset" as const,
						states: ["empty", "disabled"],
					},
				};
			},
		),
	];
	const mainShowsBrowser =
		model.rightPaneVisible ||
		model.mainSectionId === "content" ||
		model.mainSectionId === "mask";
	const controlSectionId = validActiveId(
		model.selectedControlSectionId,
		model.controlSections.map((section) => section.id),
	);
	const mainSectionId = validActiveId(
		model.mainSectionId,
		mainSectionOptions(model).map((option) => option.value),
	);
	const sourceFilterGroup =
		model.showSourceFilters && model.sourceFilter && onSelectSourceFilter
			? {
					id: "media-source-filter",
					kind: "tabs" as const,
					activeId: model.sourceFilter,
					onActiveChange: (id: string) =>
						onSelectSourceFilter(id as MediaSourceFilter),
					actions: [
						{ id: "media", label: "Media" },
						{ id: "visualizers", label: "VIS" },
						{ id: "text", label: "Text" },
					],
				}
			: undefined;
	const titleGroups = !hasPatchedServer
		? []
		: model.rightPaneVisible
			? [
					...(sourceFilterGroup ? [sourceFilterGroup] : []),
					{
						id: "media-browser-mode",
						kind: "tabs" as const,
						activeId: model.browserMode,
						onActiveChange: (id: string) =>
							onSelectBrowserMode(id as MediaBrowserMode),
						actions: browserOptions(model.maskBrowser).map((option) => ({
							id: option.value,
							label: option.label,
							disabled: option.disabled,
						})),
					},
					{
						id: "media-control-section",
						kind: "tabs" as const,
						activeId: controlSectionId,
						onActiveChange: onSelectControlSection,
						actions: model.controlSections.map((section) => ({
							id: section.id,
							label: section.label,
						})),
					},
				]
			: [
					...(sourceFilterGroup && ["content", "mask"].includes(mainSectionId)
						? [sourceFilterGroup]
						: []),
					{
						id: "media-window-section",
						kind: "tabs" as const,
						activeId: mainSectionId,
						onActiveChange: (sectionId: string) => {
							if (sectionId === "content") onSelectBrowserMode("media");
							else if (sectionId === "mask") onSelectBrowserMode("mask");
							else onSelectControlSection(sectionId);
						},
						actions: mainSectionOptions(model).map((option) => ({
							id: option.value,
							label: option.label,
							disabled: option.disabled,
						})),
					},
				];
	return (
		<WindowFrame
			className={`media-pane-surface ${compact ? "compact" : ""}`}
			title={title}
			info={info}
			groups={titleGroups}
			toolbar={headerAction}
			settingsTitle="Media pane settings"
			settingsTabs={
				hasPatchedServer
					? [
							{
								id: "pane",
								label: "Pane",
								content: (
									<div className="media-pane-settings">
										<SwitchField
											label="Right pane"
											offLabel="Hidden"
											onLabel="Visible"
											checked={model.rightPaneVisible}
											onChange={(event) =>
												onSetRightPaneVisible(event.target.checked)
											}
										/>
										{model.nativeManagementUrl ? (
											<div className="ui-window-action-group">
												<Button
													onClick={() => setNativeManagementView("controls")}
												>
													Show controls
												</Button>
												<Button
													onClick={() => setNativeManagementView("content")}
												>
													Show content
												</Button>
											</div>
										) : null}
									</div>
								),
							},
						]
					: []
			}
		>
			{nativeManagementView && model.nativeManagementUrl ? (
				<section className="media-native-management">
					<header>
						<strong>
							{nativeManagementView === "content"
								? "Media Server content"
								: "Media Server controls"}
						</strong>
						<Button onClick={() => setNativeManagementView(null)}>Close</Button>
					</header>
					<iframe
						title={`Media Server ${nativeManagementView}`}
						src={`${model.nativeManagementUrl}${
							nativeManagementView === "content" ? "/library" : "/media"
						}`}
					/>
				</section>
			) : !hasPatchedServer ? (
				<div className="media-pane-empty" role="status">
					<strong>No media server is patched</strong>
					{onOpenPatch ? (
						<div className="ui-window-action-group media-pane-empty-action">
							<Button onClick={onOpenPatch}>Open Patch</Button>
						</div>
					) : null}
				</div>
			) : (
				<div
					className={`media-pane-body ${patchedServers.length ? "has-server-shortcuts" : ""}`}
				>
					{patchedServers.length ? (
						<section
							className="media-server-shortcuts"
							aria-label="Media servers"
						>
							<PoolGrid
								className="media-server-pool"
								columns={1}
								fillEmptySlots={false}
								minimumCardWidth={100}
								slots={serverPoolSlots}
								emptySlot={(position) => ({
									id: `empty-${position}`,
									position,
									card: {
										number: position + 1,
										primary: "Empty",
										states: ["empty"],
									},
								})}
								onSlotClick={(serverId) => {
									if (patchedServers.some((server) => server.id === serverId))
										onSelectServer(serverId);
								}}
							/>
						</section>
					) : null}
					<section className="media-pane-overview" aria-label="Media output">
						{!model.hasCitpEndpoint && model.preview.kind !== "audio" && (
							<p className="media-pane-citp-note" role="status">
								CITP is not configured. Layers and manual folder/file values
								remain available; previews and advertised library names are
								unavailable.
							</p>
						)}
						<MediaCompositePreview
							preview={model.preview}
							selected={model.selectedLayerId === "master"}
							onSelect={() => onSelectLayer("master")}
						/>
						<MediaLayerStrip
							layers={model.layers}
							selectedLayerId={model.selectedLayerId}
							onSelectLayer={onSelectLayer}
						/>
					</section>
					<section className="media-pane-workspace">
						{mainShowsBrowser && (
							<MediaLibraryBrowser
								mode={model.browserMode}
								folders={model.libraryFolders.map((item) => ({
									...item,
									disabled:
										item.disabled ||
										(model.selectedLayerId === "master" &&
											model.browserMode === "media"),
								}))}
								files={model.libraryFiles.map((item) => ({
									...item,
									disabled:
										item.disabled ||
										(model.selectedLayerId === "master" &&
											model.browserMode === "media"),
								}))}
								draftFolderId={model.draftFolderId}
								draftFileId={model.draftFileId}
								onBrowseItem={onBrowseItem}
							/>
						)}
						{(model.rightPaneVisible || !mainShowsBrowser) && (
							<MediaSecondaryControls
								sections={model.controlSections}
								selectedSectionId={
									model.rightPaneVisible ? controlSectionId : mainSectionId
								}
								onChange={onChangeControl}
								onReset={onResetControl}
							/>
						)}
					</section>
				</div>
			)}
		</WindowFrame>
	);
}

function validActiveId(activeId: string, ids: string[]) {
	return ids.includes(activeId) ? activeId : (ids[0] ?? "");
}

function mainSectionOptions(model: MediaPaneModel) {
	return [
		{ value: "content", label: "Content" },
		...(model.maskBrowser === "hidden"
			? []
			: [
					{
						value: "mask",
						label:
							model.maskBrowser === "unsupported"
								? "Mask · Unsupported"
								: "Mask",
						disabled: model.maskBrowser === "unsupported",
					},
				]),
		...model.controlSections.map((section) => ({
			value: section.id,
			label: section.label,
		})),
	];
}

function browserOptions(maskBrowser: "supported" | "unsupported" | "hidden") {
	return [
		{ value: "media" as const, label: "Content" },
		...(maskBrowser === "hidden"
			? []
			: [
					{
						value: "mask" as const,
						label:
							maskBrowser === "unsupported" ? "Mask · Unsupported" : "Mask",
						disabled: maskBrowser === "unsupported",
					},
				]),
	];
}


function MediaLayerStrip({
	layers,
	selectedLayerId,
	onSelectLayer,
}: {
	layers: MediaPaneLayer[];
	selectedLayerId: string | null;
	onSelectLayer(layerId: string): void;
}) {
	const [failedPreviews, setFailedPreviews] = useState<Map<string, string>>(
		new Map(),
	);
	const previewFailed = (id: string, src: string) =>
		setFailedPreviews((current) => new Map(current).set(id, src));
	return (
		<div className="media-layer-region">
			<ul className="media-layer-strip" aria-label="Media layers">
				{layers.map((layer) => {
					const currentPreviewFailed =
						layer.thumbnailSrc !== undefined &&
						failedPreviews.get(layer.id) === layer.thumbnailSrc;
					const opacity = Math.max(
						0,
						Math.min(100, layer.opacityPercent ?? 100),
					);
					return (
						<li key={layer.id}>
							<Button
								className={`media-layer-tile status-${layer.status}`}
								active={layer.id === selectedLayerId}
								aria-pressed={layer.id === selectedLayerId}
								aria-label={`Layer ${layer.number} ${layer.name} · ${layer.statusLabel ?? layer.status}`}
								onClick={() => onSelectLayer(layer.id)}
							>
								<span
									className={`media-layer-thumbnail ${layer.thumbnailSrc ? "has-image" : "is-empty"} ${layer.audio ? "is-audio" : ""}`}
								>
									{layer.audio && (
										<span className="media-layer-audio">
											<small className="media-layer-audio-values">
												{layer.audio.volumeLabel} · {layer.audio.sourceLabel}
											</small>
											<MusicNote
												className="media-layer-note"
												label={`Audio ${layer.audio.volumeLabel} · ${layer.audio.sourceLabel}`}
											/>
										</span>
									)}
									{layer.thumbnailSrc && (
										<SafePreviewImage
											src={layer.thumbnailSrc}
											alt=""
											onFailure={() =>
												previewFailed(layer.id, layer.thumbnailSrc ?? "")
											}
										/>
									)}
								</span>
								<span className="media-layer-copy">
									<b className="media-layer-title">
										<span>Layer {layer.number}</span>
										<span className="media-layer-name">{layer.name}</span>
									</b>
									<small className="media-layer-source">
										{layer.liveSourceLabel ?? layer.name}
									</small>
									{layer.errorDetail || currentPreviewFailed ? (
										<small className="media-layer-error" role="alert">
											{layer.errorDetail ??
												"Live layer preview could not be loaded."}
										</small>
									) : null}
									<span
										className="media-layer-opacity"
										role="img"
										aria-label={`Opacity ${opacity}%`}
										title={`Opacity ${opacity}%`}
									>
										<i aria-hidden="true" style={{ opacity: opacity / 100 }} />
										<small>Dimmer {opacity}%</small>
									</span>
									<small className="media-layer-mask">
										<strong>Mask:</strong> {layer.maskLabel ?? "None / None"}
									</small>
									<span className="media-layer-color">
										<i
											aria-hidden="true"
											style={{ backgroundColor: layer.colorValue ?? "#ffffff" }}
										/>
										<small>Grey {layer.grayscalePercent ?? 100}%</small>
									</span>
									<small className="media-layer-effect">
										<strong>Effect:</strong> {layer.effectLabel ?? "None"}
									</small>
								</span>
								<i aria-hidden="true" />
							</Button>
						</li>
					);
				})}
			</ul>
		</div>
	);
}

function MediaLibraryBrowser({
	mode,
	folders,
	files,
	draftFolderId,
	draftFileId,
	onBrowseItem,
}: {
	mode: MediaBrowserMode;
	folders: MediaLibraryItem[];
	files: MediaLibraryItem[];
	draftFolderId: string;
	draftFileId: string | null;
	onBrowseItem(mode: MediaBrowserMode, item: MediaLibraryItem): void;
}) {
	const folderSlots = mediaPoolSlots(folders, draftFolderId, false);
	const clearFile: MediaLibraryItem = {
		id: "0",
		kind: "file",
		name: "No file selected",
		detail: "Clear the current file",
		disabled:
			files.length > 0
				? files.every((item) => item.disabled)
				: folders.length > 0 && folders.every((item) => item.disabled),
	};
	const displayedFiles = [clearFile, ...files];
	const fileSlots = withMediaFilePlaceholders(
		mediaPoolSlots(displayedFiles, draftFileId, true),
		40,
	);
	return (
		<section
			className={`media-library-browser ${mode === "mask" ? "is-mask" : "is-content"}`}
			aria-label="Media library browser"
			data-browser-mode={mode}
		>
			<HorizontalMediaPool
				label={`${mode === "mask" ? "Mask" : "Media"} folders`}
				className="media-folder-pool"
			>
				<PoolGrid
					className="media-folder-pool-grid"
					slots={folderSlots}
					fillEmptySlots={false}
					emptySlot={emptyMediaPoolSlot}
					minimumCardWidth={104}
					onSlotClick={(id) => {
						const item = folders.find((candidate) => candidate.id === id);
						if (item && !item.disabled) onBrowseItem(mode, item);
					}}
				/>
			</HorizontalMediaPool>
			<header className="media-library-title">
				<strong>{mode === "mask" ? "Mask File" : "Media File"}</strong>
			</header>
			<section
				className="media-file-pool"
				aria-label={`${mode === "mask" ? "Mask" : "Media"} files`}
			>
				<PoolGrid
					className="media-file-pool-grid"
					slots={fileSlots}
					fillEmptySlots={false}
					emptySlot={emptyMediaPoolSlot}
					minimumCardWidth={112}
					onSlotClick={(id) => {
						const item = displayedFiles.find(
							(candidate) => candidate.id === id,
						);
						if (item && !item.disabled) onBrowseItem(mode, item);
					}}
				/>
			</section>
		</section>
	);
}

function withMediaFilePlaceholders(
	slots: PoolSlotViewModel<string>[],
	minimumCount: number,
): PoolSlotViewModel<string>[] {
	return [
		...slots,
		...Array.from(
			{ length: Math.max(0, minimumCount - slots.length) },
			(_, index) => {
				const position = slots.length + index;
				return {
					id: `media-unavailable-${position + 1}`,
					position,
					card: {
						number: String(position + 1).padStart(3, "0"),
						primary: "",
						states: ["empty" as const],
					},
				};
			},
		),
	];
}

function emptyMediaPoolSlot(index: number): PoolSlotViewModel<string> {
	return {
		id: `empty-${index}`,
		position: index,
		card: { number: index + 1, primary: "Empty", states: ["empty"] },
	};
}

function mediaPoolSlots(
	items: MediaLibraryItem[],
	selectedId: string | null,
	withImage: boolean,
): PoolSlotViewModel<string>[] {
	return items.map((item, index) => ({
		id: item.id,
		position: index,
		card: {
			number: Number.isInteger(Number(item.id))
				? String(Number(item.id)).padStart(3, "0")
				: String(index + 1).padStart(3, "0"),
			primary: item.name,
			secondary: item.detail,
			kind: "generic",
			icon: withImage ? undefined : "▰",
			image:
				withImage && item.thumbnailSrc
					? { src: item.thumbnailSrc, alt: "" }
					: undefined,
			states: [
				...(item.empty ? (["empty"] as const) : []),
				...(item.id === selectedId ? (["selected"] as const) : []),
				...(item.disabled ? (["disabled"] as const) : []),
			],
		},
	}));
}

function HorizontalMediaPool({
	label,
	className,
	children,
}: {
	label: string;
	className: string;
	children: ReactNode;
}) {
	const ref = useRef<HTMLElement>(null);
	const [edges, setEdges] = useState({ left: false, right: true });
	useEffect(() => {
		const element = ref.current;
		if (!element) return;
		const update = () =>
			setEdges({
				left: element.scrollLeft > 1,
				right:
					element.scrollLeft + element.clientWidth < element.scrollWidth - 1,
			});
		update();
		element.addEventListener("scroll", update, { passive: true });
		const observer = new ResizeObserver(update);
		observer.observe(element);
		return () => {
			element.removeEventListener("scroll", update);
			observer.disconnect();
		};
	}, []);
	return (
		<section
			ref={ref}
			className={`${className} fade-${edges.left ? "left" : "none"}-${edges.right ? "right" : "none"}`}
			aria-label={label}
		>
			{children}
		</section>
	);
}

function MediaSecondaryControls({
	sections,
	selectedSectionId,
	onChange,
	onReset,
}: {
	sections: MediaControlSection[];
	selectedSectionId: string;
	onChange(controlId: string, value: string | number): void;
	onReset?(controlId: string): void;
}) {
	const [effectSlot, setEffectSlot] = useState(0);
	const section =
		sections.find((candidate) => candidate.id === selectedSectionId) ??
		sections[0];
	return (
		<section
			className="media-secondary-controls"
			aria-label="Media secondary controls"
		>
			<div className="media-control-scroll">
				{section && (
					<div
						className="media-control-panel"
						role="tabpanel"
						aria-label={`${section.label} controls`}
					>
						<header>
							<strong>{section.label}</strong>
							{section.capability === "unsupported" && (
								<span role="status">
									Not supported by this layer
									{section.unsupportedDetail
										? ` · ${section.unsupportedDetail}`
										: ""}
								</span>
							)}
						</header>
						{section.id === "effects" ? (
							<div
								className="media-effect-tabs"
								role="tablist"
								aria-label="Effect slot"
							>
								{[0, 1, 2, 3].map((slot) => (
									<Button
										key={slot}
										role="tab"
										aria-selected={effectSlot === slot}
										active={effectSlot === slot}
										onClick={() => setEffectSlot(slot)}
									>
										Effect {slot + 1}
									</Button>
								))}
							</div>
						) : null}
						<div className="media-control-grid">
							{section.controls
								.filter(
									(control) =>
										section.id !== "effects" ||
										effectControlBelongsToSlot(control.id, effectSlot),
								)
								.map((control, index, visibleControls) => (
									<Fragment key={control.id}>
										{control.group &&
											control.group !== visibleControls[index - 1]?.group && (
												<h3>{control.group}</h3>
											)}
										<MediaControl
											control={
												section.id === "effects" &&
												control.id === `media.layer.effect.${effectSlot + 1}`
													? { ...control, label: "Amount" }
													: control
											}
											sectionDisabled={section.capability === "unsupported"}
											onChange={onChange}
											onReset={onReset}
										/>
									</Fragment>
								))}
						</div>
					</div>
				)}
			</div>
		</section>
	);
}

function effectControlBelongsToSlot(controlId: string, slot: number) {
	return (
		controlId === "native-effects-error" ||
		controlId === `media.layer.effect.${slot + 1}` ||
		controlId.startsWith(`effect-${slot}-`) ||
		(slot === 0 && controlId.startsWith("visualizer-"))
	);
}

function MediaControl({
	control,
	sectionDisabled,
	onChange,
	onReset,
}: {
	control: MediaSecondaryControl;
	sectionDisabled: boolean;
	onChange(controlId: string, value: string | number): void;
	onReset?(controlId: string): void;
}) {
	const disabled = sectionDisabled || control.disabled;
	if (control.kind === "readout")
		return (
			<div className={`media-control-readout ${disabled ? "disabled" : ""}`}>
				<span>{control.label}</span>
				<strong>{control.value}</strong>
				{control.description && <small>{control.description}</small>}
			</div>
		);
	return (
		<div className="media-control-with-reset">
			<MediaControlEditor
				control={control}
				disabled={Boolean(disabled)}
				onChange={onChange}
			/>
			{onReset ? (
				<Button
					className="media-control-reset"
					disabled={disabled}
					aria-label={`Reset ${control.label}`}
					onClick={() => onReset(control.id)}
				>
					Reset
				</Button>
			) : null}
		</div>
	);
}

function MediaControlEditor({
	control,
	disabled,
	onChange,
}: {
	control: Exclude<MediaSecondaryControl, { kind: "readout" }>;
	disabled: boolean;
	onChange(controlId: string, value: string | number): void;
}) {
	if (control.kind === "choice" && isPlayModeControl(control))
		return (
			<div className={`media-choice-control ${disabled ? "disabled" : ""}`}>
				{control.quickActions?.length ? (
					<div
						className="media-choice-quick-actions"
						role="toolbar"
						aria-label="Play mode quick actions"
					>
						{control.quickActions.map((action) => (
							<Button
								key={action.value}
								disabled={disabled}
								active={control.value === action.value}
								onClick={() => onChange(control.id, action.value)}
							>
								{action.label}
							</Button>
						))}
					</div>
				) : null}
				<GroupedSelectionField
					label={control.label}
					ariaLabel={control.label}
					dialogTitle={`Choose ${control.label}`}
					value={playModeRepresentative(control)}
					groups={playModeGroups(control)}
					disabled={disabled}
					onChange={(value) => onChange(control.id, value)}
				/>
				{control.description && <small>{control.description}</small>}
			</div>
		);
	if (isSpeedControl(control)) {
		const options = speedOptions(control);
		return (
			<div className={`media-choice-control ${disabled ? "disabled" : ""}`}>
				<GroupedSelectionField
					label={control.label}
					ariaLabel={control.label}
					dialogTitle={`Choose ${control.label}`}
					value={speedRepresentative(control, options)}
					groups={speedGroups(options)}
					disabled={disabled}
					onChange={(value) => onChange(control.id, Number(value))}
				/>
				{control.description && <small>{control.description}</small>}
			</div>
		);
	}
	if (control.kind === "choice" && control.options.length > 4)
		return (
			<div className={`media-choice-control ${disabled ? "disabled" : ""}`}>
				{control.quickActions?.length ? (
					<div
						className="media-choice-quick-actions"
						role="toolbar"
						aria-label="Play mode quick actions"
					>
						{control.quickActions.map((action) => (
							<Button
								key={action.value}
								disabled={disabled}
								active={control.value === action.value}
								onClick={() => onChange(control.id, action.value)}
							>
								{action.label}
							</Button>
						))}
					</div>
				) : null}
				<SelectField
					label={control.label}
					value={control.value}
					options={control.options}
					disabled={disabled}
					onChange={(value) => onChange(control.id, value)}
				/>
				{control.description && <small>{control.description}</small>}
			</div>
		);
	if (control.kind === "choice")
		return (
			<div className={`media-choice-control ${disabled ? "disabled" : ""}`}>
				<strong>{control.label}</strong>
				<MultiValueToggle
					ariaLabel={control.label}
					value={control.value}
					options={control.options.map((option) => ({
						...option,
						disabled: disabled || option.disabled,
					}))}
					onChange={(value) => onChange(control.id, value)}
				/>
				{control.description && <small>{control.description}</small>}
			</div>
		);
	if (control.kind === "value")
		return (
			<div className="media-value-control">
				<HorizontalFader
					label={control.label}
					disabled={disabled}
					value={control.value}
					minimum={control.minimum}
					maximum={control.maximum}
					step={control.step}
					display={control.display}
					displayFormat={control.displayFormat}
					accentColor={control.accentColor}
					onChange={(value) => onChange(control.id, value)}
				/>
				{control.description && <small>{control.description}</small>}
			</div>
		);
	if (control.kind === "color")
		return (
			<ColorPickerField
				label={control.label}
				description={control.description}
				disabled={disabled}
				value={control.value}
				onChange={(value) => onChange(control.id, value)}
			/>
		);
	return null;
}

const SPEED_SELECTION_OPTIONS = [
	...Array.from({ length: 15 }, (_, index) => ({
		value: String(index * 8),
		label: `/${16 - index}`,
	})),
	{ value: "127", label: "1×" },
	...Array.from({ length: 15 }, (_, index) => ({
		value: String(Math.ceil(135 + (index * 121) / 15)),
		label: `${index + 2}×`,
	})),
];

function isPlayModeControl(
	control: Exclude<MediaSecondaryControl, { kind: "readout" }>,
) {
	return (
		control.kind === "choice" &&
		["play-mode", "media.play_mode", "media.layer.play.mode"].includes(
			control.id,
		)
	);
}

function isSpeedControl(
	control: Exclude<MediaSecondaryControl, { kind: "readout" }>,
) {
	return [
		"speed",
		"media.playback_speed",
		"media.layer.speed.multiplier",
	].includes(control.id);
}

function playModeRepresentative(
	control: Extract<MediaSecondaryControl, { kind: "choice" }>,
) {
	const raw = Number(control.value);
	if (!Number.isFinite(raw)) return control.value;
	return (
		[...control.options]
			.filter((option) => Number(option.value) <= raw)
			.sort((left, right) => Number(right.value) - Number(left.value))[0]
			?.value ??
		control.options[0]?.value ??
		control.value
	);
}

function playModeGroups(
	control: Extract<MediaSecondaryControl, { kind: "choice" }>,
) {
	const option = (candidate: (typeof control.options)[number]) => ({
		...candidate,
		description: playModeDescription(candidate.label),
	});
	const continuous = control.options.filter(
		(candidate) =>
			!["Once", "Synced", "Stop", "Pause"].some((word) =>
				candidate.label.includes(word),
			),
	);
	const synced = control.options.filter(
		(candidate) =>
			candidate.label.includes("Synced") && !candidate.label.includes("Once"),
	);
	const once = control.options.filter(
		(candidate) =>
			candidate.label.includes("Once") && !candidate.label.includes("Synced"),
	);
	const onceSynced = control.options.filter(
		(candidate) =>
			candidate.label.includes("Once") && candidate.label.includes("Synced"),
	);
	const transport = control.options.filter((candidate) =>
		["Stop", "Pause"].includes(candidate.label),
	);
	return [
		{ label: "Continuous playback", options: continuous.map(option) },
		{ label: "Synchronized playback", options: synced.map(option) },
		{ label: "Play once", options: once.map(option) },
		{ label: "Synchronized play once", options: onceSynced.map(option) },
		{ label: "Transport", options: transport.map(option) },
	].filter((group) => group.options.length > 0);
}

function playModeDescription(label: string) {
	if (label === "Stop") return "Stop playback and return to the start.";
	if (label === "Pause") return "Hold the current playback frame.";
	if (label.includes("Transparent"))
		return "Finish this pass, then make the layer transparent.";
	if (label.includes("Black")) return "Finish this pass, then output black.";
	if (label.includes("Hold"))
		return "Finish this pass, then hold the final frame.";
	if (label.includes("Bounce"))
		return "Alternate forward and reverse playback.";
	if (label.includes("Reverse")) return "Play in reverse.";
	return "Play continuously from the beginning after each pass.";
}

function speedOptions(
	control: Exclude<MediaSecondaryControl, { kind: "readout" }>,
) {
	return control.kind === "choice" ? control.options : SPEED_SELECTION_OPTIONS;
}

function speedRepresentative(
	control: Exclude<MediaSecondaryControl, { kind: "readout" }>,
	options: Array<{ value: string; label: string; disabled?: boolean }>,
) {
	const raw = Number(control.value);
	const label =
		raw <= 119
			? `/${16 - Math.floor(Math.max(0, raw) / 8)}`
			: raw < 135
				? "1×"
				: `${Math.min(16, Math.floor(((raw - 135) * 15) / 121) + 2)}×`;
	return options.find((option) => option.label === label)?.value ?? "127";
}

function speedGroups(
	options: Array<{ value: string; label: string; disabled?: boolean }>,
) {
	const describe = (candidate: (typeof options)[number]) => ({
		...candidate,
		description: candidate.label.startsWith("/")
			? `Play at one ${candidate.label.slice(1)}th of normal speed.`
			: candidate.label === "1×"
				? "Play at normal speed."
				: `Play at ${candidate.label} normal speed.`,
	});
	return [
		{
			label: "Slower",
			options: options
				.filter((option) => option.label.startsWith("/"))
				.map(describe),
		},
		{
			label: "Normal",
			options: options.filter((option) => option.label === "1×").map(describe),
		},
		{
			label: "Faster",
			options: options
				.filter((option) => option.label.endsWith("×") && option.label !== "1×")
				.map(describe),
		},
	];
}
