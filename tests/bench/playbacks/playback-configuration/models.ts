export type PlaybackTarget =
	| { type: "cue_list"; cue_list_id: string }
	| { type: "group"; group_id: string }
	| { type: "speed_group"; group: string }
	| { type: "programmer_fade" }
	| { type: "cue_fade" }
	| { type: "grand_master" };

export interface PlaybackDefinition {
	number: number;
	name: string;
	target: PlaybackTarget;
	buttons: [string, string, string];
	button_count: number;
	fader: string;
	has_fader: boolean;
	go_activates: boolean;
	auto_off: boolean;
	xfade_millis: number;
	color: string;
	flash_release: string;
	protect_from_swap: boolean;
	presentation_icon?: string;
	presentation_image?: string;
}

export interface PreparedShow {
	showId: string;
	cueListId: string;
	fixtures: Record<number, string>;
}

export interface PlaybackConfigurationObservation {
	page: number;
	slot: number;
	number: number;
	targetType: PlaybackTarget["type"];
	targetMatchesExpected: boolean;
	buttons: string[];
	buttonCount: number;
	fader: string;
	hasFader: boolean;
	color: string;
}

export interface PlaybackCheckpoint {
	cue: number;
	position: number;
	progress: number;
	direction: string;
	intensity: number;
}
