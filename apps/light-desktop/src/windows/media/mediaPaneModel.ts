import type { ReactNode } from "react";

export type MediaBrowserMode = "media" | "mask";

export type MediaPreviewState =
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
			detail: string;
	  }
	| {
			kind: "failed_source";
			imageSrc?: string;
			capturedAt?: string;
			source: string;
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
	  };

export type MediaLayerStatus = "online" | "stale" | "failed" | "unsupported";

export interface MediaPaneServer {
	id: string;
	name: string;
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
	thumbnailSrc?: string;
	liveSourceLabel?: string;
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
	disabled?: boolean;
}

interface MediaControlBase {
	id: string;
	label: string;
	description?: string;
	disabled?: boolean;
}

export interface MediaChoiceControl extends MediaControlBase {
	kind: "choice";
	value: string;
	options: Array<{ value: string; label: string; disabled?: boolean }>;
}

export interface MediaValueControl extends MediaControlBase {
	kind: "value";
	value: number;
	minimum?: number;
	maximum?: number;
	step?: number;
	display?: string;
	accentColor?: string;
}

export interface MediaColorControl extends MediaControlBase {
	kind: "color";
	value: string;
}

export interface MediaReadoutControl extends MediaControlBase {
	kind: "readout";
	value: ReactNode;
}

export type MediaSecondaryControl =
	| MediaChoiceControl
	| MediaValueControl
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
	servers: MediaPaneServer[];
	selectedServerId: string;
	selectedLayerId: string | null;
	preview: MediaPreviewState;
	layers: MediaPaneLayer[];
	browserMode: MediaBrowserMode;
	maskBrowser: "supported" | "unsupported" | "hidden";
	libraryPath: string[];
	libraryItems: MediaLibraryItem[];
	liveSelection: MediaBrowseSelection;
	draftSelection: MediaBrowseSelection;
	liveSelectionLabel: string;
	draftSelectionLabel: string;
	controlSections: MediaControlSection[];
	selectedControlSectionId: string;
}

export interface MediaPaneUiCallbacks {
	onSelectServer(serverId: string): void;
	onSelectLayer(layerId: string): void;
	onSelectBrowserMode(mode: MediaBrowserMode): void;
	onBrowseItem(mode: MediaBrowserMode, item: MediaLibraryItem): void;
	onSelectControlSection(sectionId: string): void;
	onChangeControl(controlId: string, value: string | number): void;
	onOpenSettings?(anchor: HTMLElement): void;
}

export interface MediaPaneSurfaceProps extends MediaPaneUiCallbacks {
	model: MediaPaneModel;
	dummyDataBadge: ReactNode;
	compact?: boolean;
}
