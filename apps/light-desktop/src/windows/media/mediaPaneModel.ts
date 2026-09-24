import type { ReactNode } from "react";

export type MediaBrowserMode = "media" | "mask";
export type MediaSourceFilter = "media" | "visualizers" | "text";

export type MediaPreviewState = {
	outputSize?: { width: number; height: number };
} & (
	| {
			kind: "ready";
			imageSrc?: string;
			capturedAt?: string;
	  }
	| {
			kind: "stale";
			imageSrc?: string;
			capturedAt: string;
			detail?: string;
	  }
	| {
			kind: "offline";
			imageSrc?: string;
			capturedAt?: string;
			/** Short operator-facing reason. Never the raw CITP protocol text. */
			detail: string;
			/** The underlying protocol error, for diagnostics rather than the pane headline. */
			diagnostic?: string;
	  }
	| {
			kind: "failed_source";
			imageSrc?: string;
			capturedAt?: string;
			source: string;
			detail: string;
	  }
	| {
			kind: "audio";
			detail: string;
	  }
	| {
			kind: "missing_patch";
			detail: string;
	  }
	| {
			kind: "unsupported";
			capability: string;
			detail: string;
	  }
);

export type MediaLayerStatus = "online" | "stale" | "failed" | "unsupported";

export interface MediaPaneServer {
	id: string;
	name: string;
	fixtureLabel?: string;
	detail?: string;
	statusLabel: string;
	disabled?: boolean;
}

export interface MediaPaneLayer {
	id: string;
	number: string;
	name: string;
	status: MediaLayerStatus;
	statusLabel?: string;
	errorDetail?: string;
	thumbnailSrc?: string;
	/** Internal Audio Player voice: a music note with its live volume and source above it. */
	audio?: { volumeLabel: string; sourceLabel: string };
	liveSourceLabel?: string;
	opacityPercent?: number;
	maskLabel?: string;
	colorValue?: string;
	grayscalePercent?: number;
	effectLabel?: string;
}

export interface MediaBrowseSelection {
	folderId: string | null;
	fileId: string | null;
	maskFolderId: string | null;
	maskFileId: string | null;
}

export interface MediaLibraryItem {
	id: string;
	kind: "folder" | "file";
	name: string;
	detail?: string;
	thumbnailSrc?: string;
	empty?: boolean;
	disabled?: boolean;
}

/** Native Pixel state used to name a numbered effect preset without moving its settings into the show. */
export interface MediaEffectLibrarySlot {
	slot: number;
	name?: string;
	status: "assigned" | "unassigned" | "unsupported";
	detail?: string;
}

interface MediaControlBase {
	id: string;
	label: string;
	group?: string;
	description?: string;
	disabled?: boolean;
}

export interface MediaChoiceControl extends MediaControlBase {
	kind: "choice";
	value: string;
	options: Array<{ value: string; label: string; disabled?: boolean }>;
	quickActions?: Array<{ value: string; label: string }>;
}

export interface MediaValueControl extends MediaControlBase {
	kind: "value";
	value: number;
	minimum?: number;
	maximum?: number;
	step?: number;
	display?: string;
	/**
	 * How the fader writes the value it currently holds. A control that sets this reads its own
	 * position while the operator drags, instead of a number the server last confirmed.
	 */
	displayFormat?: "percent" | "decimal" | "integer";
	accentColor?: string;
}

/**
 * A Media In or Out point: a 16-bit frame count shown and entered as `mm:ss.ff` at the Media
 * Server's frame rate.
 */
export interface MediaPointTimeControl extends MediaControlBase {
	kind: "point-time";
	/** The DMX frame count. */
	value: number;
	/** The In point counts from the clip's start, the Out point back from its end. */
	reference: "start" | "end";
	/** Null while the server's rate is unknown; entry then takes a frame count. */
	framesPerSecond: number | null;
	display: string;
	/** Why the rate is unknown and what the operator can do about it. */
	rateNotice?: string;
	/** Asks the Media Server for its frame rate again; offered with a notice. */
	onRetryFrameRate?(): void;
}

/**
 * What the desk knows about the rate a Media Server's In and Out points count in. The desk never
 * assumes one: an unknown rate is shown and the points fall back to frame counts.
 */
export type MediaPointFrameRate =
	| { kind: "known"; framesPerSecond: number }
	| { kind: "loading" }
	| {
			kind: "unknown";
			detail: string;
			/** Whether asking the server again can help. */
			retryable?: boolean;
	  };

export interface MediaColorControl extends MediaControlBase {
	kind: "color";
	value: string;
}

export interface MediaReadoutControl extends MediaControlBase {
	kind: "readout";
	value: ReactNode;
	/**
	 * One action at the row's right edge, such as clearing the range the row heads: the changes
	 * it sends, each as the control it writes and the value, all at once.
	 */
	action?: {
		label: string;
		disabled?: boolean;
		changes: ReadonlyArray<{ controlId: string; value: number }>;
	};
}

export type MediaSecondaryControl =
	| MediaChoiceControl
	| MediaValueControl
	| MediaPointTimeControl
	| MediaColorControl
	| MediaReadoutControl;

export interface MediaControlSection {
	id: string;
	label: string;
	capability?: "supported" | "unsupported";
	unsupportedDetail?: string;
	controls: MediaSecondaryControl[];
}

export interface MediaPaneModel {
	hasPatchedServer: boolean;
	hasCitpEndpoint: boolean;
	servers: MediaPaneServer[];
	selectedServerId: string;
	selectedLayerId: string | null;
	preview: MediaPreviewState;
	layers: MediaPaneLayer[];
	browserMode: MediaBrowserMode;
	sourceFilter?: MediaSourceFilter;
	showSourceFilters: boolean;
	maskBrowser: "supported" | "unsupported" | "hidden";
	libraryFolders: MediaLibraryItem[];
	libraryFiles: MediaLibraryItem[];
	draftFolderId: string;
	draftFileId: string | null;
	liveSelection: MediaBrowseSelection;
	draftSelection: MediaBrowseSelection;
	liveSelectionLabel: string;
	draftSelectionLabel: string;
	controlSections: MediaControlSection[];
	selectedControlSectionId: string;
	mainSectionId: string;
	rightPaneVisible: boolean;
	nativeManagementUrl?: string;
}

export interface MediaPaneUiCallbacks {
	onOpenPatch?(): void;
	onSelectServer(serverId: string): void;
	onSelectLayer(layerId: string): void;
	onSelectBrowserMode(mode: MediaBrowserMode): void;
	onSelectSourceFilter?(filter: MediaSourceFilter): void;
	onBrowseItem(mode: MediaBrowserMode, item: MediaLibraryItem): void;
	onSelectControlSection(sectionId: string): void;
	onChangeControl(controlId: string, value: string | number): void;
	onResetControl?(controlId: string): void;
	onSetRightPaneVisible(visible: boolean): void;
}

export interface MediaPaneSurfaceProps extends MediaPaneUiCallbacks {
	model: MediaPaneModel;
	compact?: boolean;
	title?: ReactNode;
	info?: { primary: ReactNode; secondary?: ReactNode };
	headerAction?: ReactNode;
}
