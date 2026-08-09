import type {
	OperatorNotification,
	RuntimeCapabilityEvent,
	SessionResponse,
	UpdateTargetRequest,
	UpdateWorkflowNotification,
} from "../../api/types";
import { routeControlSurfaceIntentWithFeedback } from "../controlSurfaceInteraction/registry";
import type { ServerState } from "./useServerState";

function routeDeskAction(
	event: Extract<OperatorNotification, { type: "desk_action" }>,
	session: SessionResponse,
) {
	const payload = event.notification;
	const detail = {
		...(payload.action == null ? {} : { action: payload.action }),
		...(payload.control == null ? {} : { control: payload.control }),
		...(payload.value == null ? {} : { value: payload.value }),
		...(payload.request_id == null ? {} : { request_id: payload.request_id }),
		...(payload.session_id == null ? {} : { session_id: payload.session_id }),
		...(payload.desk_id == null ? {} : { desk_id: payload.desk_id }),
		...(payload.desk_alias == null ? {} : { desk_alias: payload.desk_alias }),
	};
	if (
		payload.action &&
		((!payload.session_id && !payload.desk_id) ||
			payload.session_id === session.session_id ||
			payload.desk_id === session.desk.id)
	) {
		if (payload.action === "align") {
			window.dispatchEvent(new CustomEvent("light:align-action", { detail }));
			return;
		} else if (payload.action === "set")
			routeControlSurfaceIntentWithFeedback({ type: "set", source: "osc" });
		else {
			const command = deskCommand(payload.action);
			if (command) {
				routeControlSurfaceIntentWithFeedback({
					type: "desk_command",
					source: "hardware",
					command,
				});
				return;
			}
			const fileAction = fileOperationKey(payload.action);
			if (fileAction)
				routeControlSurfaceIntentWithFeedback({
					type: "file_operation_key",
					source: "osc",
					action: fileAction,
				});
			else {
				const shortcut = deskShortcut(payload.action);
				if (shortcut)
					routeControlSurfaceIntentWithFeedback({
						type: "desk_shortcut",
						source: "osc",
						action: shortcut,
					});
			}
		}
	}
	if (
		payload.control &&
		payload.desk_alias === session.desk.osc_alias &&
		((!payload.session_id && !payload.desk_id) ||
			payload.session_id === session.session_id ||
			payload.desk_id === session.desk.id) &&
		(payload.control.startsWith("encode/") || payload.control === "nav")
	) {
		if (
			payload.control === "nav" &&
			(payload.value === "page-up" || payload.value === "page-down")
		) {
			window.dispatchEvent(
				new CustomEvent("light:playback-page-step", {
					detail: payload.value === "page-up" ? 1 : -1,
				}),
			);
			return;
		}
		window.dispatchEvent(new CustomEvent("light:encoder-action", { detail }));
	}
}

function deskCommand(action: string) {
	const normalized = action.toLowerCase();
	if (normalized === "menu") return "menu" as const;
	if (!normalized.startsWith("desk-")) return null;
	const command = normalized.slice(5);
	if (
		[
			"home",
			"stage",
			"fixtures",
			"channels",
			"groups",
			"presets",
			"cues",
			"playbacks",
			"setup",
			"help",
		].includes(command)
	)
		return command as import("../controlSurfaceInteraction/registry").DeskCommand;
	return null;
}

function deskShortcut(action: string) {
	const normalized = action.toLowerCase().replaceAll("-", "_");
	if (normalized === "shift_del") return "shift_delete" as const;
	if (
		[
			"shift_down",
			"shift_up",
			"shift_clear",
			"shift_delete",
			"shift_0",
			"shift_1",
			"shift_2",
			"shift_3",
			"shift_4",
			"shift_5",
			"shift_6",
			"shift_7",
			"shift_8",
			"shift_9",
		].includes(normalized)
	)
		return normalized as import("../controlSurfaceInteraction/registry").DeskShortcut;
	return null;
}

function fileOperationKey(action: string) {
	switch (action.toLowerCase()) {
		case "copy":
		case "cpy":
			return "copy" as const;
		case "move":
		case "mov":
			return "move" as const;
		case "delete":
		case "del":
			return "delete" as const;
		case "escape":
		case "esc":
			return "escape" as const;
		case "enter":
		case "ent":
			return "enter" as const;
		default:
			return null;
	}
}

function routeFileEvent(
	event: Extract<
		OperatorNotification,
		{ type: "file_input" | "file_operation" }
	>,
	session: SessionResponse,
) {
	if (event.type === "file_input") {
		const payload = event.notification;
		if (payload.session_id === session.session_id)
			window.dispatchEvent(
				new CustomEvent("light:file-manager-input", { detail: payload }),
			);
		return;
	}
	window.dispatchEvent(
		new CustomEvent("light:file-operation", { detail: event.notification }),
	);
}

function routeGroupConfiguration(
	event: Extract<OperatorNotification, { type: "group_configuration" }>,
	session: SessionResponse,
) {
	const payload = event.notification;
	if (payload.desk_id === session.desk.id)
		window.dispatchEvent(
			new CustomEvent("light:group-configuration", {
				detail: payload.group_id,
			}),
		);
}

function routeUpdateWorkflow(
	event: UpdateWorkflowNotification,
	session: SessionResponse,
) {
	if (event.desk_id !== session.desk.id) return;
	switch (event.type) {
		case "armed":
			routeControlSurfaceIntentWithFeedback({
				type: "update_armed",
				source: "osc",
				armed: event.armed,
			});
			return;
		case "target_requested": {
			const target: UpdateTargetRequest = {
				family: { type: event.target.family },
				object_id: event.target.object_id,
				...(event.target.playback_number == null
					? {}
					: { playback_number: event.target.playback_number }),
				...(event.target.cue_id == null ? {} : { cue_id: event.target.cue_id }),
				...(event.target.cue_number == null
					? {}
					: { cue_number: event.target.cue_number }),
				...(event.target.validate_active_context == null
					? {}
					: {
							validate_active_context: event.target.validate_active_context,
						}),
			};
			routeControlSurfaceIntentWithFeedback({
				type: "update_target",
				source: "osc",
				target,
			});
			return;
		}
		case "target_rejected":
			window.dispatchEvent(
				new CustomEvent("light:command-error", {
					detail:
						event.error ?? "This playback is not a recordable Update target.",
				}),
			);
			return;
		case "targets_requested":
			routeControlSurfaceIntentWithFeedback({
				type: "update_target_menu",
				source: "osc",
			});
			return;
		case "settings_requested":
			routeControlSurfaceIntentWithFeedback({
				type: "update_settings",
				source: "osc",
			});
	}
}

function refreshCommandHistory(
	event: Extract<OperatorNotification, { type: "command_history_changed" }>,
	session: SessionResponse,
	state: ServerState,
) {
	if (event.desk_id !== session.desk.id) return;
	void state.api.desk
		.commandHistory()
		.then(state.setCommandHistory)
		.catch(() => undefined);
}

export function routeOperatorEvent(
	event: RuntimeCapabilityEvent,
	session: SessionResponse,
	state: ServerState,
) {
	if (event.type !== "operator_notification") return;
	const notification = event.notification;
	switch (notification.type) {
		case "desk_action":
			routeDeskAction(notification, session);
			return;
		case "file_input":
		case "file_operation":
			routeFileEvent(notification, session);
			return;
		case "group_configuration":
			routeGroupConfiguration(notification, session);
			return;
		case "update_workflow":
			routeUpdateWorkflow(notification.notification, session);
			return;
		case "command_history_changed":
			refreshCommandHistory(notification, session, state);
	}
}
