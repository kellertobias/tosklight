import {
	Button,
	ColorPickerField,
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
	onSetRightPaneVisible,
}: MediaPaneSurfaceProps) {
	const hasPatchedServer = model.servers.some(
		(server) => server.id.trim() !== "",
	);
	const patchedServers = model.servers.filter(
		(server) => server.id.trim() !== "" && !server.disabled,
	);
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
									<SwitchField
										label="Right pane"
										offLabel="Hidden"
										onLabel="Visible"
										checked={model.rightPaneVisible}
										onChange={(event) =>
											onSetRightPaneVisible(event.target.checked)
										}
									/>
								),
							},
						]
					: []
			}
		>
			{!hasPatchedServer ? (
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
							{patchedServers.map((server) => (
								<Button
									key={server.id}
									className={
										server.id === model.selectedServerId
											? "selected"
											: undefined
									}
									aria-pressed={server.id === model.selectedServerId}
									onClick={() => onSelectServer(server.id)}
								>
									<span className="media-server-card-heading">
										<strong>{server.name}</strong>
										<b>{server.fixtureLabel}</b>
									</span>
									<small>{server.statusLabel}</small>
								</Button>
							))}
						</section>
					) : null}
					<section className="media-pane-overview" aria-label="Media output">
						{!model.hasCitpEndpoint && (
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

function MediaCompositePreview({
	preview,
	selected,
	onSelect,
}: {
	preview: MediaPreviewState;
	selected: boolean;
	onSelect(): void;
}) {
	const imageSrc = "imageSrc" in preview ? preview.imageSrc : undefined;
	const [imageFailed, setImageFailed] = useState(false);
	useEffect(() => setImageFailed(false), [imageSrc]);
	const outputAspectRatio = preview.outputSize
		? `${preview.outputSize.width} / ${preview.outputSize.height}`
		: "16 / 9";
	return (
		<Button
			type="button"
			fullWidth
			className={`media-composite-frame state-${preview.kind} ${selected ? "selected" : ""}`}
			data-preview-state={preview.kind}
			aria-label={`Master output ${selected ? "selected" : ""}`.trim()}
			aria-pressed={selected}
			onClick={onSelect}
		>
			<span
				className={`media-composite-picture ${imageSrc ? "has-image" : "is-empty"}`}
				data-testid="master-output-picture"
				style={{ aspectRatio: outputAspectRatio }}
			>
				{imageSrc && (
					<SafePreviewImage
						src={imageSrc}
						alt="Master output preview"
						onFailure={() => setImageFailed(true)}
					/>
				)}
				<span className="media-composite-safe-area" aria-hidden="true" />
			</span>
			<span className="media-composite-info">
				<strong>Master output live preview</strong>
				{imageFailed ? (
					<span className="danger" role="alert">
						Live preview could not be loaded.
					</span>
				) : null}
				<PreviewStateMessage preview={preview} />
			</span>
		</Button>
	);
}

export function SafePreviewImage({
	src,
	alt,
	onFailure,
}: {
	src: string;
	alt: string;
	onFailure(): void;
}) {
	const [loadedFrames, setLoadedFrames] = useState<string[]>([]);
	const currentSrc = loadedFrames.at(-1);
	const desiredSrc = useRef(src);
	desiredSrc.current = src;
	const [pendingSrc, setPendingSrc] = useState<string | null>(src);
	useEffect(() => {
		if (currentSrc !== src && pendingSrc === null) setPendingSrc(src);
	}, [currentSrc, pendingSrc, src]);
	const finishPending = (loaded: boolean) => {
		if (pendingSrc === null) return;
		const completed = pendingSrc;
		if (loaded)
			setLoadedFrames((frames) =>
				frames.at(-1) === completed ? frames : [...frames.slice(-1), completed],
			);
		else onFailure();
		setPendingSrc(desiredSrc.current === completed ? null : desiredSrc.current);
	};
	return (
		<>
			{loadedFrames.map((frame, index) => {
				const current = index === loadedFrames.length - 1;
				return (
					<img
						key={frame}
						className={current ? "is-current" : "is-previous"}
						aria-hidden={current ? undefined : true}
						alt={current ? alt : ""}
						src={frame}
						onError={onFailure}
					/>
				);
			})}
			{pendingSrc && (
				<img
					key={pendingSrc}
					hidden
					className="is-pending"
					aria-hidden="true"
					alt=""
					src={pendingSrc}
					onLoad={() => finishPending(true)}
					onError={() => finishPending(false)}
				/>
			)}
		</>
	);
}

function PreviewStateMessage({ preview }: { preview: MediaPreviewState }) {
	switch (preview.kind) {
		case "ready":
			if (preview.imageSrc) return null;
			return (
				<span>
					No output · black
					{preview.capturedAt ? ` · ${preview.capturedAt}` : ""}
				</span>
			);
		case "stale":
			return (
				<span className="warning">
					Stale preview · {preview.capturedAt}
					{preview.detail ? ` · ${preview.detail}` : ""}
				</span>
			);
		case "offline":
			return (
				<span className="danger">
					Offline
					{preview.imageSrc ? " · showing last preview" : " · black output"} ·{" "}
					{preview.detail}
				</span>
			);
		case "failed_source":
			return (
				<span className="danger">
					Source {preview.source} failed · {preview.detail}
				</span>
			);
		case "missing_patch":
			return <span className="danger">Missing patch · {preview.detail}</span>;
		case "unsupported":
			return (
				<span className="warning">
					{preview.capability} unsupported · {preview.detail}
				</span>
			);
	}
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
									className={`media-layer-thumbnail ${layer.thumbnailSrc ? "has-image" : "is-empty"}`}
								>
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
}: {
	sections: MediaControlSection[];
	selectedSectionId: string;
	onChange(controlId: string, value: string | number): void;
}) {
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
						<div className="media-control-grid">
							{section.controls.map((control, index) => (
								<Fragment key={control.id}>
									{control.group &&
										control.group !== section.controls[index - 1]?.group && (
											<h3>{control.group}</h3>
										)}
									<MediaControl
										control={control}
										sectionDisabled={section.capability === "unsupported"}
										onChange={onChange}
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

function MediaControl({
	control,
	sectionDisabled,
	onChange,
}: {
	control: MediaSecondaryControl;
	sectionDisabled: boolean;
	onChange(controlId: string, value: string | number): void;
}) {
	const disabled = sectionDisabled || control.disabled;
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
	return (
		<div className={`media-control-readout ${disabled ? "disabled" : ""}`}>
			<span>{control.label}</span>
			<strong>{control.value}</strong>
			{control.description && <small>{control.description}</small>}
		</div>
	);
}
