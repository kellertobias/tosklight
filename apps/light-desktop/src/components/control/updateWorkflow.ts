import type {
	CueUpdateMode,
	ExistingContentMode,
	UpdateMode,
	UpdateSettings,
	UpdateTargetIdentity,
	UpdateTargetRequest,
} from "../../api/types";
import { routeControlSurfaceIntentWithFeedback } from "../../features/controlSurfaceInteraction/registry";

export const cueUpdateModes: Array<{ value: CueUpdateMode; label: string }> = [
	{ value: "existing_in_current_cue", label: "Update" },
	{ value: "existing_only", label: "Tracked" },
	{ value: "add_to_current_cue", label: "Known" },
	{ value: "add_new", label: "All" },
];

export const existingContentModes: Array<{
	value: ExistingContentMode;
	label: string;
}> = [
	{ value: "update_existing", label: "Update Existing" },
	{ value: "add_new", label: "Add New" },
];

export const defaultUpdateSettings: UpdateSettings = {
	cue_mode: "existing_in_current_cue",
	preset_mode: "update_existing",
	group_mode: "update_existing",
	show_update_modal_on_touch: true,
};

export function configuredUpdateMode(
	settings: UpdateSettings,
	target: UpdateTargetRequest,
): UpdateMode {
	if (target.family.type === "cue")
		return { target_type: "cue", mode: settings.cue_mode };
	const mode =
		target.family.type === "preset"
			? settings.preset_mode
			: settings.group_mode;
	return { target_type: "existing_content", mode };
}

export function cueUpdateTarget(
	objectId: string,
	playbackNumber?: number,
	cue?: { id: string; number: string } | null,
): UpdateTargetRequest {
	return {
		family: { type: "cue" },
		object_id: objectId,
		...(playbackNumber == null ? {} : { playback_number: playbackNumber }),
		...(cue ? { cue_id: cue.id, cue_number: cue.number } : {}),
		...(playbackNumber != null ? { validate_active_context: true } : {}),
	};
}

export function requestFromUpdateIdentity(
	target: UpdateTargetIdentity,
): UpdateTargetRequest {
	return {
		family: target.family,
		object_id: target.object_id,
		...(target.playback_number == null
			? {}
			: { playback_number: target.playback_number }),
		...(target.cue
			? { cue_id: target.cue.id, cue_number: target.cue.number }
			: {}),
		...(target.playback_number == null
			? {}
			: { validate_active_context: true }),
	};
}

export function requestUpdateTarget(target: UpdateTargetRequest) {
	return routeControlSurfaceIntentWithFeedback({
		type: "update_target",
		source: "touch",
		target,
	});
}

export function openUpdateSettings() {
	return routeControlSurfaceIntentWithFeedback({
		type: "update_settings",
		source: "touch",
	});
}

export function openUpdateTargetMenu() {
	return routeControlSurfaceIntentWithFeedback({
		type: "update_target_menu",
		source: "touch",
	});
}

export function updateTargetKey(target: UpdateTargetIdentity) {
	return [
		target.family.type,
		target.object_id,
		target.playback_number ?? "",
		target.cue?.id ?? "",
	].join(":");
}

export function targetFamilyLabel(
	target: Pick<UpdateTargetIdentity, "family">,
) {
	if (target.family.type === "cue") return "Cuelist";
	if (target.family.type === "preset") return "Preset";
	return "Group";
}

export function modeLabel(mode: UpdateMode) {
	const options =
		mode.target_type === "cue" ? cueUpdateModes : existingContentModes;
	return (
		options.find((candidate) => candidate.value === mode.mode)?.label ??
		mode.mode.replaceAll("_", " ")
	);
}
