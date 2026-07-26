import type { UpdateTargetRequest } from "../../api/types";

export type ControlSurfaceIntent =
	| { type: "set"; source: ControlSurfaceSource }
	| {
			type: "update_target";
			source: ControlSurfaceSource;
			target: UpdateTargetRequest;
	  }
	| { type: "update_armed"; source: ControlSurfaceSource; armed: boolean }
	| { type: "update_settings"; source: ControlSurfaceSource }
	| { type: "update_target_menu"; source: ControlSurfaceSource }
	| {
			type: "file_operation_key";
			source: ControlSurfaceSource;
			action: FileOperationKey;
	  }
	| {
			type: "desk_shortcut";
			source: ControlSurfaceSource;
			action: DeskShortcut;
	  }
	| {
			type: "configure_playback";
			source: "context_menu";
			surfaceId: string;
			slot: number;
	  };

export type FileOperationKey =
	| "rename"
	| "copy"
	| "move"
	| "delete"
	| "escape"
	| "enter";

export type DeskShortcut =
	| "shift_down"
	| "shift_up"
	| "shift_clear"
	| "shift_delete"
	| "shift_0"
	| "shift_1"
	| "shift_2"
	| "shift_3"
	| "shift_4"
	| "shift_5"
	| "shift_6"
	| "shift_7"
	| "shift_8"
	| "shift_9";

export type ControlSurfaceSource =
	| "touch"
	| "mouse"
	| "keyboard"
	| "context_menu"
	| "osc"
	| "hardware"
	| "server";

export interface ControlSurfaceTarget {
	id: string;
	priority: number;
	accepts(intent: ControlSurfaceIntent): boolean;
	handle(intent: ControlSurfaceIntent): void;
}

export type ControlSurfaceRoute =
	| { status: "handled"; targetId: string }
	| { status: "missing" }
	| { status: "ambiguous"; targetIds: string[] };

const targets = new Map<string, ControlSurfaceTarget>();

export function registerControlSurfaceTarget(target: ControlSurfaceTarget) {
	targets.set(target.id, target);
	return () => {
		if (targets.get(target.id) === target) targets.delete(target.id);
	};
}

export function routeControlSurfaceIntent(
	intent: ControlSurfaceIntent,
): ControlSurfaceRoute {
	const candidates = [...targets.values()]
		.filter((target) => target.accepts(intent))
		.sort(
			(left, right) =>
				right.priority - left.priority || left.id.localeCompare(right.id),
		);
	const first = candidates[0];
	if (!first) return { status: "missing" };
	const tied = candidates.filter(
		(target) => target.priority === first.priority,
	);
	if (tied.length > 1)
		return { status: "ambiguous", targetIds: tied.map((target) => target.id) };
	first.handle(intent);
	return { status: "handled", targetId: first.id };
}

export function routeControlSurfaceIntentWithFeedback(
	intent: ControlSurfaceIntent,
) {
	const outcome = routeControlSurfaceIntent(intent);
	if (outcome.status === "handled") return outcome;
	const detail =
		outcome.status === "missing"
			? `No active control surface can handle ${intentLabel(intent)}.`
			: `More than one active control surface can handle ${intentLabel(intent)}. Choose one surface and try again.`;
	window.dispatchEvent(new CustomEvent("light:command-error", { detail }));
	return outcome;
}

function intentLabel(intent: ControlSurfaceIntent) {
	if (intent.type === "set") return "SET";
	if (intent.type === "update_target") return "this Update target";
	if (intent.type === "update_target_menu") return "the Update target menu";
	if (intent.type === "update_settings") return "Update settings";
	if (intent.type === "file_operation_key")
		return `File Manager ${intent.action}`;
	if (intent.type === "desk_shortcut")
		return intent.action.replaceAll("_", " ");
	if (intent.type === "configure_playback") return "playback configuration";
	return "Update";
}

export function resetControlSurfaceTargetsForTests() {
	targets.clear();
}
