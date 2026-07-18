import type {
	ServerEvent,
	SessionResponse,
	UpdateTargetRequest,
} from "../../api/types";
import type { ServerState } from "./useServerState";

function routeDeskAction(event: ServerEvent, session: SessionResponse) {
	if (event.kind !== "desk_action") return;
	const payload = event.payload as {
		action?: string;
		control?: string;
		value?: string;
		session_id?: string;
		desk_id?: string;
		desk_alias?: string;
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
		payload.control.startsWith("encode/")
	) {
		window.dispatchEvent(
			new CustomEvent("light:encoder-action", { detail: payload }),
		);
	}
}

function routeFileEvent(event: ServerEvent, session: SessionResponse) {
	if (event.kind === "file_input_action") {
		const payload = event.payload as {
			action?: string;
			instance_id?: string;
			session_id?: string;
		};
		if (
			payload.action &&
			payload.instance_id &&
			payload.session_id === session.session_id
		)
			window.dispatchEvent(
				new CustomEvent("light:file-manager-input", { detail: payload }),
			);
	}
	if (event.kind === "file_operation_completed")
		window.dispatchEvent(
			new CustomEvent("light:file-operation", { detail: event.payload }),
		);
}

function routeGroupConfiguration(event: ServerEvent, session: SessionResponse) {
	if (event.kind !== "group_configuration_requested") return;
	const payload = event.payload as { group_id?: string; desk_id?: string };
	if (payload.group_id && payload.desk_id === session.desk.id)
		window.dispatchEvent(
			new CustomEvent("light:group-configuration", {
				detail: payload.group_id,
			}),
		);
}

function routeUpdateWorkflow(event: ServerEvent, session: SessionResponse) {
	const kinds = [
		"update_armed",
		"update_target_requested",
		"update_target_rejected",
		"update_targets_requested",
		"update_settings_requested",
	];
	if (!kinds.includes(event.kind)) return;
	const payload = event.payload as {
		armed?: boolean;
		desk_id?: string;
		target?: UpdateTargetRequest;
		error?: string;
	};
	if (payload.desk_id !== session.desk.id) return;
	if (event.kind === "update_armed")
		window.dispatchEvent(
			new CustomEvent("light:update-armed", {
				detail: payload.armed ?? true,
			}),
		);
	if (event.kind === "update_target_requested" && payload.target)
		window.dispatchEvent(
			new CustomEvent("light:update-target", {
				detail: payload.target,
			}),
		);
	if (event.kind === "update_target_rejected")
		window.dispatchEvent(
			new CustomEvent("light:command-error", {
				detail:
					payload.error ?? "This playback is not a recordable Update target.",
			}),
		);
	if (event.kind === "update_targets_requested")
		window.dispatchEvent(new Event("light:update-target-menu"));
	if (event.kind === "update_settings_requested")
		window.dispatchEvent(new Event("light:update-settings"));
}

function refreshCommandHistory(
	event: ServerEvent,
	session: SessionResponse,
	state: ServerState,
) {
	if (
		event.kind !== "command_history" ||
		(event.payload as { desk_id?: string }).desk_id !== session.desk.id
	)
		return;
	void state.client
		.commandHistory()
		.then(state.setCommandHistory)
		.catch(() => undefined);
}

export function routeOperatorEvent(
	event: ServerEvent,
	session: SessionResponse,
	state: ServerState,
) {
	routeDeskAction(event, session);
	routeFileEvent(event, session);
	routeGroupConfiguration(event, session);
	routeUpdateWorkflow(event, session);
	refreshCommandHistory(event, session, state);
}
