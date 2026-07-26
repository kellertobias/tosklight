import type {
	OperatorNotification,
	RuntimeCapabilityEvent,
	UpdateWorkflowNotification,
	SessionResponse,
	UpdateTargetRequest,
} from "../../api/types";
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
		window.dispatchEvent(
			new CustomEvent("light:desk-action", { detail: payload.action }),
		);
	}
	if (
		payload.control &&
		payload.desk_alias === session.desk.osc_alias &&
		(payload.control.startsWith("encode/") || payload.control === "nav")
	) {
		window.dispatchEvent(new CustomEvent("light:encoder-action", { detail }));
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
			window.dispatchEvent(
				new CustomEvent("light:update-armed", { detail: event.armed }),
			);
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
			window.dispatchEvent(
				new CustomEvent("light:update-target", {
					detail: target,
				}),
			);
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
			window.dispatchEvent(new Event("light:update-target-menu"));
			return;
		case "settings_requested":
			window.dispatchEvent(new Event("light:update-settings"));
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
