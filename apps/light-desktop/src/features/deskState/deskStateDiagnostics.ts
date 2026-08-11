import { useSyncExternalStore } from "react";

export type SystemControlsTab = "running" | "desk-state";

let requestedTab: SystemControlsTab = "running";
const listeners = new Set<() => void>();

export function requestSystemControlsTab(tab: SystemControlsTab) {
	if (requestedTab === tab) return;
	requestedTab = tab;
	for (const listener of listeners) listener();
}

export function useRequestedSystemControlsTab(): SystemControlsTab {
	return useSyncExternalStore(
		(listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
		() => requestedTab,
		() => "running",
	);
}

export interface DeskStateDiagnostic {
	id: string;
	title: string;
	summary: string;
	action: string;
	detail?: string;
}

export function deskStateDiagnostic(message: string): DeskStateDiagnostic {
	const fixtureDuplicate = message.match(
		/Fixture (.+?) has more than one Programmer value for (.+?)\./u,
	);
	if (fixtureDuplicate) {
		const [, fixtureId, attribute] = fixtureDuplicate;
		return {
			id: `programmer-fixture-${slug(fixtureId)}-${slug(attribute)}`,
			title: `Programmer conflict · Fixture ${fixtureId} · ${attribute}`,
			summary: `Fixture ${fixtureId} has two Programmer values for ${attribute}. The desk cannot choose one authoritative value until the conflict is repaired. This is not a DMX patch overlap.`,
			action: `Reload the desk state. If the conflict returns, inspect Fixture ${fixtureId}'s profile, logical heads, and multipatch data.`,
		};
	}

	const groupDuplicate = message.match(
		/Group (.+?) has more than one Programmer value for (.+?)\./u,
	);
	if (groupDuplicate) {
		const [, groupId, attribute] = groupDuplicate;
		return {
			id: `programmer-group-${slug(groupId)}-${slug(attribute)}`,
			title: `Programmer conflict · Group ${groupId} · ${attribute}`,
			summary: `Group ${groupId} has two Programmer values for ${attribute}. The desk cannot choose one authoritative value until the conflict is repaired.`,
			action: `Reload the desk state. If the conflict returns, remove the duplicate stored value from Group ${groupId}.`,
		};
	}

	const dynamicDuplicate = message.match(
		/Dynamic control projected (.+?) more than once for fixture (.+?)\./u,
	);
	if (dynamicDuplicate) {
		const [, attribute, fixtureId] = dynamicDuplicate;
		return {
			id: `dynamic-fixture-${slug(fixtureId)}-${slug(attribute)}`,
			title: `Dynamic conflict · Fixture ${fixtureId} · ${attribute}`,
			summary: `More than one Dynamic value is targeting ${attribute} on Fixture ${fixtureId}. The desk cannot choose one authoritative value until the conflict is repaired.`,
			action: "Stop and restart the affected Dynamic. If it returns, inspect that Dynamic's fixture targets.",
		};
	}

	return {
		id: "desk-error",
		title: "Desk error",
		summary: "The desk reported an error that needs operator attention.",
		action: "Review the detail below, correct the named condition, then retry the operation.",
		detail: message,
	};
}

function slug(value: string) {
	return value.toLocaleLowerCase().replace(/[^a-z0-9]+/gu, "-");
}
