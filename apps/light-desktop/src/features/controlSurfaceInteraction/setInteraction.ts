import type {
	ControlSurfaceInteractionScope,
	ControlSurfaceSource,
	GroupInteractionIdentity,
	PlaybackInteractionIdentity,
	SetInteractionTerminalIntent,
} from "./contracts";

export type SetInteractionState =
	| { phase: "idle"; scope: ControlSurfaceInteractionScope }
	| { phase: "set_armed"; scope: ControlSurfaceInteractionScope }
	| {
			phase: "group_source_pending";
			scope: ControlSurfaceInteractionScope;
			group: GroupInteractionIdentity;
	  };

interface ScopedEvent {
	scope: ControlSurfaceInteractionScope;
}

interface SourcedEvent extends ScopedEvent {
	source: ControlSurfaceSource;
}

export type SetInteractionEvent =
	| (SourcedEvent & { type: "arm_set" })
	| (SourcedEvent & {
			type: "choose_group";
			group: GroupInteractionIdentity;
	  })
	| (SourcedEvent & {
			type: "choose_playback";
			playback: PlaybackInteractionIdentity;
	  })
	| (SourcedEvent & { type: "enter" })
	| (SourcedEvent & {
			type: "select_group_live" | "select_group_frozen" | "open_group_settings";
			group: GroupInteractionIdentity;
	  })
	| (ScopedEvent & { type: "cancel" | "clear" })
	| { type: "replace_scope"; scope: ControlSurfaceInteractionScope };

export interface SetInteractionTransition {
	state: SetInteractionState;
	intent: SetInteractionTerminalIntent | null;
	status: "transitioned" | "invalid_transition" | "scope_mismatch";
}

export function initialSetInteractionState(
	scope: ControlSurfaceInteractionScope,
): SetInteractionState {
	return { phase: "idle", scope };
}

/**
 * Pure desk/show/surface-scoped SET state machine. It carries only explicit object
 * identities and cannot inspect Programmer selection or any incidental UI state.
 */
export function transitionSetInteraction(
	state: SetInteractionState,
	event: SetInteractionEvent,
): SetInteractionTransition {
	if (event.type === "replace_scope")
		return transitioned({ phase: "idle", scope: event.scope });
	if (!sameScope(state.scope, event.scope))
		return { state, intent: null, status: "scope_mismatch" };

	if (event.type === "cancel" || event.type === "clear")
		return transitioned(idle(state.scope));

	if (
		event.type === "select_group_live" ||
		event.type === "select_group_frozen" ||
		event.type === "open_group_settings"
	)
		return transitioned(idle(state.scope), {
			type: event.type,
			source: event.source,
			scope: state.scope,
			group: event.group,
		});

	if (event.type === "arm_set")
		return transitioned({ phase: "set_armed", scope: state.scope });

	if (event.type === "choose_group") {
		if (state.phase !== "set_armed") return invalid(state);
		return transitioned(
			{
				phase: "group_source_pending",
				scope: state.scope,
				group: event.group,
			},
			{
				type: "choose_group_master_source",
				source: event.source,
				scope: state.scope,
				group: event.group,
			},
		);
	}

	if (event.type === "choose_playback") {
		if (state.phase === "set_armed")
			return transitioned(idle(state.scope), {
				type: "open_playback_settings",
				source: event.source,
				scope: state.scope,
				playback: event.playback,
			});
		if (state.phase === "group_source_pending")
			return transitioned(idle(state.scope), {
				type: "assign_group_master",
				source: event.source,
				scope: state.scope,
				group: state.group,
				playback: event.playback,
			});
		return invalid(state);
	}

	if (event.type === "enter") {
		if (state.phase === "group_source_pending")
			return transitioned(idle(state.scope), {
				type: "open_group_settings",
				source: event.source,
				scope: state.scope,
				group: state.group,
			});
		if (state.phase === "set_armed") return transitioned(idle(state.scope));
		return invalid(state);
	}

	return invalid(state);
}

function sameScope(
	left: ControlSurfaceInteractionScope,
	right: ControlSurfaceInteractionScope,
) {
	return (
		left.deskId === right.deskId &&
		left.showId === right.showId &&
		left.surfaceId === right.surfaceId
	);
}

function idle(scope: ControlSurfaceInteractionScope): SetInteractionState {
	return { phase: "idle", scope };
}

function transitioned(
	state: SetInteractionState,
	intent: SetInteractionTerminalIntent | null = null,
): SetInteractionTransition {
	return { state, intent, status: "transitioned" };
}

function invalid(state: SetInteractionState): SetInteractionTransition {
	return { state, intent: null, status: "invalid_transition" };
}
