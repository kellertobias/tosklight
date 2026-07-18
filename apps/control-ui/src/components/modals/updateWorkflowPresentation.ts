import type {
	UpdatePreview,
	UpdatePreviewItem,
	UpdateTargetIdentity,
} from "../../api/types";
import { targetFamilyLabel } from "../control/updateWorkflow";

const changingOutcomes = new Set([
	"change_at_source",
	"change_in_current_cue",
	"add_to_current_cue",
	"add_new_to_current_cue",
	"update_existing",
	"add_new",
]);

export function updatePreviewStats(preview: UpdatePreview) {
	const ignored = preview.items.filter(
		(item) => item.outcome.outcome === "ignored",
	).length;
	const changed = preview.items.filter((item) =>
		changingOutcomes.has(item.outcome.outcome),
	).length;
	const added = preview.items.filter((item) =>
		["add_to_current_cue", "add_new_to_current_cue", "add_new"].includes(
			item.outcome.outcome,
		),
	).length;
	const source = preview.items.filter(
		(item) => item.outcome.outcome === "change_at_source",
	).length;
	const currentCue = preview.items.filter((item) =>
		[
			"change_in_current_cue",
			"add_to_current_cue",
			"add_new_to_current_cue",
		].includes(item.outcome.outcome),
	).length;
	return {
		eligible: preview.items.length - ignored,
		changed,
		added,
		ignored,
		source,
		currentCue,
	};
}

export function updateAddressLabel(item: UpdatePreviewItem) {
	if (item.address.type === "fixture_attribute") {
		return `Fixture ${item.address.fixture_id} · ${item.address.attribute}`;
	}
	if (item.address.type === "group_attribute") {
		return `Group ${item.address.group_id} · ${item.address.attribute}`;
	}
	return `Fixture ${item.address.fixture_id} · Group membership`;
}

export function updateOutcomeLabel(item: UpdatePreviewItem) {
	const outcome = item.outcome;
	if (outcome.outcome === "change_at_source") {
		return `Change at source Cue ${outcome.source.cue_number}`;
	}
	if (outcome.outcome === "change_in_current_cue") {
		return `Change in current Cue ${outcome.cue.cue_number}`;
	}
	if (outcome.outcome === "add_to_current_cue") {
		return `Add to current Cue ${outcome.cue.cue_number}`;
	}
	if (outcome.outcome === "add_new_to_current_cue") {
		return `Add new to current Cue ${outcome.cue.cue_number}`;
	}
	if (outcome.outcome === "update_existing") {
		return "Update existing stored content";
	}
	if (outcome.outcome === "add_new") return "Add new stored content";
	if (outcome.outcome === "unchanged") {
		return outcome.source
			? `Unchanged at Cue ${outcome.source.cue_number}`
			: "Unchanged";
	}
	return {
		new_address: "Ignored · address is new to this target",
		not_in_current_cue: "Ignored · not explicitly stored in the current Cue",
		not_in_active_tracked_state: "Ignored · not in the active tracked state",
		new_group_member: "Ignored · fixture is not an existing Group member",
	}[outcome.reason];
}

export function updateTargetContext(target: UpdateTargetIdentity) {
	const parts = [targetFamilyLabel(target)];
	if (target.playback_number != null) {
		parts.push(`Playback ${target.playback_number}`);
	}
	if (target.cue) parts.push(`Current Cue ${target.cue.number}`);
	return parts.join(" · ");
}
