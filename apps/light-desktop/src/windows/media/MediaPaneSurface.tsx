import {
	Button,
	ColorPickerField,
	HorizontalFader,
	MultiValueToggle,
	SelectField,
} from "@tosklight/ui";
import { WindowHeader, WindowScrollArea } from "@tosklight/ui/window-kit";
import type {
	MediaBrowserMode,
	MediaControlSection,
	MediaLibraryItem,
	MediaPaneLayer,
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
	dummyDataBadge,
	compact = false,
	onSelectServer,
	onSelectLayer,
	onSelectBrowserMode,
	onBrowseItem,
	onSelectControlSection,
	onChangeControl,
	onOpenSettings,
}: MediaPaneSurfaceProps) {
	const server = model.servers.find(
		(candidate) => candidate.id === model.selectedServerId,
	);
	return (
		<section
			className={`media-pane-surface ${compact ? "compact" : ""}`}
			aria-label="Media"
		>
			<WindowHeader
				title="Media"
				info={{
					primary: server?.name ?? "No patched media server",
					secondary: server?.statusLabel ?? "Missing patch",
				}}
				toolbar={
					<strong className="media-pane-dummy-badge" role="status">
						{dummyDataBadge}
					</strong>
				}
				settings={Boolean(onOpenSettings)}
				onSettings={onOpenSettings}
			/>
			<div className="media-pane-server-bar">
				<SelectField
					label="Media server"
					value={model.selectedServerId}
					options={model.servers.map((candidate) => ({
						value: candidate.id,
						label: candidate.name,
						disabled: candidate.disabled,
					}))}
					onChange={onSelectServer}
				/>
				<span>{server?.detail ?? "Select a patched media-server master."}</span>
			</div>
			<div className="media-pane-body">
				<section className="media-pane-overview" aria-label="Media output">
					<MediaCompositePreview preview={model.preview} />
					<MediaLayerStrip
						layers={model.layers}
						selectedLayerId={model.selectedLayerId}
						onSelectLayer={onSelectLayer}
					/>
				</section>
				<section className="media-pane-workspace">
					<MediaLibraryBrowser
						mode={model.browserMode}
						maskBrowser={model.maskBrowser}
						path={model.libraryPath}
						items={model.libraryItems}
						liveLabel={model.liveSelectionLabel}
						draftLabel={model.draftSelectionLabel}
						onSelectMode={onSelectBrowserMode}
						onBrowseItem={onBrowseItem}
					/>
					<MediaSecondaryControls
						sections={model.controlSections}
						selectedSectionId={model.selectedControlSectionId}
						onSelectSection={onSelectControlSection}
						onChange={onChangeControl}
					/>
				</section>
			</div>
		</section>
	);
}

function MediaCompositePreview({ preview }: { preview: MediaPreviewState }) {
	const imageSrc = "imageSrc" in preview ? preview.imageSrc : undefined;
	return (
		<figure
			className={`media-composite-frame state-${preview.kind}`}
			data-preview-state={preview.kind}
			aria-label="Program composite preview"
		>
			{imageSrc && <img src={imageSrc} alt="Mock program composite" />}
			<div className="media-composite-safe-area" aria-hidden="true" />
			<figcaption>
				<strong>PROGRAM / COMPOSITE</strong>
				<PreviewStateMessage preview={preview} />
			</figcaption>
		</figure>
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
			<header>
				<strong>Layers</strong>
				<small className="media-layer-count">{layers.length} mock layers</small>
			</header>
			<ul className="media-layer-strip" aria-label="Media layers">
				{layers.map((layer) => (
					<li key={layer.id}>
						<Button
							className={`media-layer-tile status-${layer.status}`}
							active={layer.id === selectedLayerId}
							aria-pressed={layer.id === selectedLayerId}
							aria-label={`Layer ${layer.number} ${layer.name} · ${layer.statusLabel ?? layer.status}`}
							onClick={() => onSelectLayer(layer.id)}
						>
							<span className="media-layer-thumbnail">
								{layer.thumbnailSrc && <img src={layer.thumbnailSrc} alt="" />}
							</span>
							<span>
								<b>
									{layer.number} · {layer.name}
								</b>
								<small>{layer.liveSourceLabel ?? "No live source"}</small>
							</span>
							<i aria-hidden="true" />
						</Button>
					</li>
				))}
			</ul>
		</div>
	);
}

function MediaLibraryBrowser({
	mode,
	maskBrowser,
	path,
	items,
	liveLabel,
	draftLabel,
	onSelectMode,
	onBrowseItem,
}: {
	mode: MediaBrowserMode;
	maskBrowser: "supported" | "unsupported" | "hidden";
	path: string[];
	items: MediaLibraryItem[];
	liveLabel: string;
	draftLabel: string;
	onSelectMode(mode: MediaBrowserMode): void;
	onBrowseItem(mode: MediaBrowserMode, item: MediaLibraryItem): void;
}) {
	const browserOptions = [
		{ value: "media" as const, label: "Media" },
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
	return (
		<section
			className="media-library-browser"
			aria-label="Media library browser"
		>
			<header>
				<MultiValueToggle
					ariaLabel="Media or Mask browser"
					value={mode}
					options={browserOptions}
					onChange={onSelectMode}
				/>
			</header>
			<section
				className="media-library-authority"
				aria-label="Live and browsing selections"
			>
				<span>
					<b>LIVE</b> {liveLabel}
				</span>
				<span className="draft">
					<b>BROWSING DRAFT</b> {draftLabel} · Not sent
				</span>
			</section>
			<nav className="media-library-path" aria-label="Library path">
				{path.map((part, index) => (
					<span key={`${part}-${index}`}>
						{index > 0 && <i aria-hidden="true">/</i>}
						{part}
					</span>
				))}
			</nav>
			<WindowScrollArea
				emptyState={
					items.length === 0
						? {
								title: "No mock media in this folder",
								description: "The live source remains unchanged.",
							}
						: null
				}
			>
				<div className="media-library-grid">
					{items.map((item) => (
						<Button
							key={item.id}
							className={`media-library-item kind-${item.kind}`}
							disabled={item.disabled}
							aria-label={`${mode === "mask" ? "Mask" : "Media"} ${item.kind} ${item.name}`}
							onClick={() => onBrowseItem(mode, item)}
						>
							<span className="media-library-thumbnail">
								{item.thumbnailSrc ? (
									<img src={item.thumbnailSrc} alt="" />
								) : (
									<span aria-hidden="true">
										{item.kind === "folder" ? "▰" : "▶"}
									</span>
								)}
							</span>
							<b>{item.name}</b>
							<small>{item.detail ?? item.kind}</small>
						</Button>
					))}
				</div>
			</WindowScrollArea>
			<footer>
				<Button disabled fullWidth>
					Program draft · unavailable in Storybook
				</Button>
			</footer>
		</section>
	);
}

function MediaSecondaryControls({
	sections,
	selectedSectionId,
	onSelectSection,
	onChange,
}: {
	sections: MediaControlSection[];
	selectedSectionId: string;
	onSelectSection(sectionId: string): void;
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
			<div
				className="media-control-tabs"
				role="tablist"
				aria-label="Control groups"
			>
				{sections.map((candidate) => (
					<Button
						key={candidate.id}
						role="tab"
						aria-selected={candidate.id === section?.id}
						active={candidate.id === section?.id}
						onClick={() => onSelectSection(candidate.id)}
					>
						{candidate.label}
					</Button>
				))}
			</div>
			<WindowScrollArea>
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
							{section.controls.map((control) => (
								<MediaControl
									key={control.id}
									control={control}
									sectionDisabled={section.capability === "unsupported"}
									onChange={onChange}
								/>
							))}
						</div>
					</div>
				)}
			</WindowScrollArea>
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
			<SelectField
				label={control.label}
				description={control.description}
				disabled={disabled}
				value={control.value}
				options={control.options}
				onChange={(value) => onChange(control.id, value)}
			/>
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
