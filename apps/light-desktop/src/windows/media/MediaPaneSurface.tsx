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
} from "./mediaPaneModel";

export function MediaPaneSurface({
	model,
	compact = false,
	title = "Media",
	headerAction,
	onOpenPatch,
	onSelectServer,
	onSelectLayer,
	onSelectBrowserMode,
	onBrowseItem,
	onSelectControlSection,
	onChangeControl,
	onSetRightPaneVisible,
}: MediaPaneSurfaceProps) {
	const server = model.servers.find(
		(candidate) => candidate.id === model.selectedServerId,
	);
	const hasPatchedServer = model.servers.some((candidate) => candidate.id.trim() !== "");
	const mainShowsBrowser =
		model.rightPaneVisible ||
		model.mainSectionId === "content" ||
		model.mainSectionId === "mask";
	return (
		<WindowFrame
			className={`media-pane-surface ${compact ? "compact" : ""}`}
			title={title}
			toolbar={
				<div className="media-pane-header-tools">
					{model.rightPaneVisible ? (
						<>
							<MultiValueToggle
								ariaLabel="Content or Mask browser"
								value={model.browserMode}
								options={browserOptions(model.maskBrowser)}
								onChange={onSelectBrowserMode}
							/>
							<MultiValueToggle
								ariaLabel="Media control section"
								value={model.selectedControlSectionId}
								options={model.controlSections.map((section) => ({
									value: section.id,
									label: section.label,
								}))}
								onChange={onSelectControlSection}
							/>
						</>
					) : (
						<MultiValueToggle
							ariaLabel="Media window section"
							value={model.mainSectionId}
							options={mainSectionOptions(model)}
							onChange={(sectionId) => {
								if (sectionId === "content") onSelectBrowserMode("media");
								else if (sectionId === "mask") onSelectBrowserMode("mask");
								else onSelectControlSection(sectionId);
							}}
						/>
					)}
					<SelectField
						className="media-pane-server-select"
						label="Server"
						ariaLabel="Media server"
						size="compact"
						value={model.selectedServerId}
						options={model.servers.map((candidate) => ({
							value: candidate.id,
							label: candidate.name,
							disabled: candidate.disabled,
						}))}
						onChange={onSelectServer}
					/>
					{headerAction}
				</div>
			}
			settingsTitle="Media pane settings"
			settingsTabs={[
				{
					id: "pane",
					label: "Pane",
					content: (
						<SwitchField
							label="Right pane"
							offLabel="Hidden"
							onLabel="Visible"
							checked={model.rightPaneVisible}
							onChange={(event) => onSetRightPaneVisible(event.target.checked)}
						/>
					),
				},
			]}
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
				<div className="media-pane-body">
				<section className="media-pane-overview" aria-label="Media output">
					<MediaCompositePreview
						preview={model.preview}
						selected={model.selectedLayerId === "master"}
						onSelect={() => onSelectLayer("master")}
						serverLabel={server?.name ?? "No patched media server"}
						statusLabel={server?.statusLabel ?? "Missing patch"}
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
							folders={model.libraryFolders}
							files={model.libraryFiles}
							draftFolderId={model.draftFolderId}
							draftFileId={model.draftFileId}
							onBrowseItem={onBrowseItem}
						/>
					)}
					{(model.rightPaneVisible || !mainShowsBrowser) && (
						<MediaSecondaryControls
							sections={model.controlSections}
							selectedSectionId={
								model.rightPaneVisible
									? model.selectedControlSectionId
									: model.mainSectionId
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
	serverLabel,
	statusLabel,
}: {
	preview: MediaPreviewState;
	selected: boolean;
	onSelect(): void;
	serverLabel: string;
	statusLabel: string;
}) {
	const imageSrc = "imageSrc" in preview ? preview.imageSrc : undefined;
	return (
		<Button
			type="button"
			className={`media-composite-frame state-${preview.kind} ${selected ? "selected" : ""}`}
			data-preview-state={preview.kind}
			aria-label={`Master output ${selected ? "selected" : ""}`.trim()}
			aria-pressed={selected}
			onClick={onSelect}
		>
			{imageSrc && <img src={imageSrc} alt="Mock program composite" />}
			<div className="media-composite-safe-area" aria-hidden="true" />
			<span className="media-composite-caption">
				<strong>MASTER OUTPUT</strong>
				<small>
					{serverLabel} · {statusLabel}
				</small>
				<PreviewStateMessage preview={preview} />
			</span>
		</Button>
	);
}

function PreviewStateMessage({ preview }: { preview: MediaPreviewState }) {
	switch (preview.kind) {
		case "ready":
			return (
				<span>
					Mock preview
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
					{preview.imageSrc ? " · showing mock last preview" : ""} ·{" "}
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
	return (
		<div className="media-layer-region">
			<ul className="media-layer-strip" aria-label="Media layers">
				{layers.map((layer) => {
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
								<span className="media-layer-thumbnail">
									{layer.thumbnailSrc && (
										<img src={layer.thumbnailSrc} alt="" />
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
	const fileSlots = withMediaFilePlaceholders(
		mediaPoolSlots(files, draftFileId, true),
		40,
	);
	return (
		<section
			className="media-library-browser"
			aria-label="Media library browser"
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
						if (item) onBrowseItem(mode, item);
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
				{files.length === 0 ? (
					<div className="media-file-empty">
						<strong>This folder is empty</strong>
						<span>
							The live selection stays unchanged until a file is chosen.
						</span>
					</div>
				) : (
					<PoolGrid
						className="media-file-pool-grid"
						slots={fileSlots}
						fillEmptySlots={false}
						emptySlot={emptyMediaPoolSlot}
						minimumCardWidth={112}
						onSlotClick={(id) => {
							const item = files.find((candidate) => candidate.id === id);
							if (item) onBrowseItem(mode, item);
						}}
					/>
				)}
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
