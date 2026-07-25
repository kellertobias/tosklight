export enum PaneType {
	Stage = "stage",
	Groups = "groups",
	Fixtures = "fixtures",
	Presets = "presets",
	Cuelists = "cuelists",
	CuelistPool = "cuelist_pool",
	Cues = "cues",
	Playback = "playback",
	PlaybackPool = "playback_pool",
	CueList = "cue_list",
	Dynamics = "dynamics",
	Channels = "channels",
	Dmx = "dmx",
	Patch = "patch",
	Setup = "setup",
	Help = "help",
	Development = "development",
	VirtualPlaybacks = "virtual_playbacks",
	FileManager = "file_manager",
	TextEditor = "text_editor",
}

export type BuiltInPaneType =
	| PaneType.Stage
	| PaneType.Fixtures
	| PaneType.Presets
	| PaneType.Cuelists
	| PaneType.Dynamics
	| PaneType.Channels;

export const builtInPaneTypes: readonly BuiltInPaneType[] = [
	PaneType.Stage,
	PaneType.Fixtures,
	PaneType.Presets,
	PaneType.Cuelists,
	PaneType.Dynamics,
	PaneType.Channels,
];

export const builtInLabels: Readonly<Record<BuiltInPaneType, string>> = {
	[PaneType.Stage]: "Stage",
	[PaneType.Fixtures]: "Fixtures",
	[PaneType.Presets]: "Presets",
	[PaneType.Cuelists]: "Cuelists",
	[PaneType.Dynamics]: "Dynamics",
	[PaneType.Channels]: "Channels",
};

export type OperatorPaneType =
	| PaneType.Stage
	| PaneType.Groups
	| PaneType.Fixtures
	| PaneType.Presets
	| PaneType.Cuelists
	| PaneType.CuelistPool
	| PaneType.Cues
	| PaneType.Dynamics
	| PaneType.Channels
	| PaneType.Dmx
	| PaneType.Help
	| PaneType.VirtualPlaybacks
	| PaneType.FileManager
	| PaneType.TextEditor;

export enum StageView {
	TwoDimensional = "2d",
	ThreeDimensional = "3d",
}

export enum PresetFamily {
	Mixed = "Mixed",
	Dimmer = "Dimmer",
	Position = "Position",
	Color = "Color",
	Beam = "Beam",
	Focus = "Focus",
}

export interface StagePaneConfiguration {
	view?: StageView;
	followPreload?: boolean;
	beamGuides?: boolean;
	showGroupShortcuts?: boolean;
}

export interface FixtureSheetPaneConfiguration {
	showGroupShortcuts?: boolean;
}

export interface PresetPaneConfiguration {
	family?: PresetFamily;
	poolColors?: boolean;
	showGroupShortcuts?: boolean;
}

export interface CuesPaneConfiguration {
	showCueSidebar?: boolean;
}

export interface VirtualPlaybackPaneConfiguration {
	rows?: number;
	columns?: number;
}

export interface TextEditorPaneConfiguration {
	readOnly?: boolean;
	view?: "plain" | "markdown" | "split";
}

export type PaneConfiguration<T extends PaneType> =
	T extends PaneType.Stage ? StagePaneConfiguration
		: T extends PaneType.Fixtures ? FixtureSheetPaneConfiguration
			: T extends PaneType.Presets ? PresetPaneConfiguration
				: T extends PaneType.Cues ? CuesPaneConfiguration
					: T extends PaneType.VirtualPlaybacks ? VirtualPlaybackPaneConfiguration
						: T extends PaneType.TextEditor ? TextEditorPaneConfiguration
							: Record<never, never>;

export const paneLabels: Readonly<Record<PaneType, string>> = {
	[PaneType.Stage]: "Stage",
	[PaneType.Groups]: "Group pool",
	[PaneType.Fixtures]: "Fixture sheet",
	[PaneType.Presets]: "Preset pool",
	[PaneType.Cuelists]: "Cuelists",
	[PaneType.CuelistPool]: "Cuelist Pool",
	[PaneType.Cues]: "Cues · Cuelist",
	[PaneType.Playback]: "Playback",
	[PaneType.PlaybackPool]: "Playback Pool",
	[PaneType.CueList]: "Cuelist",
	[PaneType.Dynamics]: "Dynamics",
	[PaneType.Channels]: "Channels",
	[PaneType.Dmx]: "DMX output",
	[PaneType.Patch]: "Patch",
	[PaneType.Setup]: "Setup",
	[PaneType.Help]: "Help",
	[PaneType.Development]: "Development",
	[PaneType.VirtualPlaybacks]: "Virtual Playbacks",
	[PaneType.FileManager]: "File Manager",
	[PaneType.TextEditor]: "Text Editor",
};
