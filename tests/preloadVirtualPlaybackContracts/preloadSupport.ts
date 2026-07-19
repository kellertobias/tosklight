import type { ApiDriver } from "../../apps/control-ui/e2e/bench/api";
import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import type { Page } from "../../apps/control-ui/node_modules/@playwright/test/index.js";
import { programmer } from "../support/catalog";
import { configuration, visualizationLevel } from "./showFixtures";
import type {
	Configuration,
	PreloadCombinedPairState,
	PreloadProgrammerPairState,
} from "./types";

export async function preloadProgrammerObservation(
	api: ApiDriver,
	fixtures: [string, string],
): Promise<NonNullable<PreloadProgrammerPairState["pending"]>> {
	const pending = await programmer(api);
	return {
		blind: pending.blind,
		groupIds: Object.keys(pending.preload_group_pending).sort(),
		groupValues: Object.keys(pending.group_values).sort(),
		firstFadeMillis:
			pending.preload_group_pending["1"].intensity.fade_millis ?? null,
		secondFadeMillis:
			pending.preload_group_pending["2"].intensity.fade_millis ?? null,
		playbackActions: pending.preload_playback_pending.map(
			(entry: any) => entry.action,
		),
		liveLevels: [
			await visualizationLevel(api, fixtures[0]),
			await visualizationLevel(api, fixtures[1]),
		],
	};
}

export function playbackPendingObservation(
	state: any,
): Array<[number, string, string]> {
	return state.preload_playback_pending.map((entry: any) => [
		entry.playback_number,
		entry.action,
		entry.surface,
	]);
}

export function captureMask(
	config: Configuration,
): [boolean, boolean, boolean] {
	return [
		config.preload_programmer_changes,
		config.preload_physical_playback_actions,
		config.preload_virtual_playback_actions,
	];
}

export const preloadMaskLabels = [
	"Preload programmer changes",
	"Preload physical playback actions",
	"Preload virtual playback actions",
] as const;

export async function openPreloadInputSettings(page: Page) {
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
	await page.getByRole("button", { name: "Programmer", exact: true }).click();
}

export async function setPreloadMaskThroughUi(
	api: ApiDriver,
	page: Page,
	mask: number,
) {
	for (let index = 0; index < preloadMaskLabels.length; index++) {
		const desired = Boolean(mask & (1 << index));
		const control = page.getByRole("switch", {
			name: preloadMaskLabels[index],
		});
		if ((await control.isChecked()) !== desired)
			await control.locator("..").locator(".ui-switch-track").click();
	}
	await page.getByRole("button", { name: "Save changes", exact: true }).click();
	await expect
		.poll(async () => captureMask(await configuration(api)))
		.toEqual([Boolean(mask & 1), Boolean(mask & 2), Boolean(mask & 4)]);
}

export async function expectPreloadMaskControls(page: Page, mask: number) {
	for (let index = 0; index < preloadMaskLabels.length; index++)
		await expect(page.getByLabel(preloadMaskLabels[index])).toBeChecked({
			checked: Boolean(mask & (1 << index)),
		});
}

export function preloadCombinedObservation(
	state: any,
): NonNullable<PreloadCombinedPairState["pending"]> {
	return {
		groupIds: Object.keys(state.preload_group_pending).sort(),
		playbackActions: playbackPendingObservation(state),
	};
}

export async function normalizedVirtualZones(
	api: ApiDriver,
): Promise<Array<{ name: string; slots: number[] }>> {
	const response = await api.request<any>(
		"GET",
		"/api/v1/virtual-playback-exclusion-zones",
	);
	return Object.values(
		response.surfaces as Record<
			string,
			Array<{ name: string; slots: number[] }>
		>,
	)
		.flat()
		.map((zone) => ({ name: zone.name, slots: [...zone.slots] }))
		.sort((left, right) => left.name.localeCompare(right.name));
}
