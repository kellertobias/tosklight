import { preloadScopeText } from "../../apps/light-desktop/src/features/playbackPreloadScope/scopeText";
import type { PlaybackAddress } from "../../apps/light-desktop/src/features/playbackTopology/contracts";
import type { CaptureMask, PendingPlaybackAction } from "./types";
import type { Page } from "@playwright/test";
import type { ApiDriver } from "../bench/core/api";
import { expect } from "../bench/core/fixtures";
import { activeShowId, programmer } from "../support/catalog";
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

export function playbackPendingObservation(state:any): PendingPlaybackAction[] {
 return state.preload_playback_pending.map((entry:any) => [entry.address,entry.action,entry.control_source]);
}
export const panePendingObservation = playbackPendingObservation;
export async function captureMask(api:ApiDriver): Promise<CaptureMask> {
 const showId = await activeShowId(api);
 const scope = await api.request<any>("GET","/api/v2/playback-preload-scope",undefined,true,undefined,{showId});
 return {programmer:(await configuration(api)).preload_programmer_changes,playbackScope:scope.scope};
}
export async function openPreloadInputSettings(page:Page) {
 await page.getByRole("button",{name:/Open show menu/}).click();
 await page.getByRole("button",{name:"Enter Setup",exact:true}).click();
 await page.getByRole("button",{name:"Others",exact:true}).click();
}
export async function setPreloadMaskThroughUi(api:ApiDriver,page:Page,mask:CaptureMask) {
 const programmer=page.getByRole("switch",{name:"Preload programmer changes"});
 if(await programmer.isChecked() !== mask.programmer) await programmer.locator("..").locator(".ui-switch-track").click();
 const text=preloadScopeText(mask.playbackScope);
 await page.getByLabel("Playback position ranges").fill(text.positions);
 const all=page.getByRole("switch",{name:"All pages"});
 if(await all.isChecked() !== text.allPages) await all.locator("..").locator(".ui-switch-track").click();
 if(!text.allPages) await page.getByLabel("Eligible pages").fill(text.pages);
 await page.getByRole("button",{name:"Save playback eligibility",exact:true}).click();
 await expect.poll(() => captureMask(api)).toEqual(mask);
}
export async function expectPreloadMaskControls(page:Page,mask:CaptureMask) {
 const text=preloadScopeText(mask.playbackScope);
 await expect(page.getByRole("switch",{name:"Preload programmer changes"})).toBeChecked({checked:mask.programmer});
 await expect(page.getByLabel("Playback position ranges")).toHaveValue(text.positions);
 await expect(page.getByRole("switch",{name:"All pages"})).toBeChecked({checked:text.allPages});
 if(!text.allPages) await expect(page.getByLabel("Eligible pages")).toHaveValue(text.pages);
}

export function preloadCombinedObservation(
	state: any,
): NonNullable<PreloadCombinedPairState["pending"]> {
	return {
		groupIds: Object.keys(state.preload_group_pending).sort(),
		playbackActions: playbackPendingObservation(state),
	};
}

export function timestampMillis(value: unknown): number {
	expect(value).toEqual(expect.any(String));
	const millis = Date.parse(value as string);
	expect(Number.isFinite(millis)).toBe(true);
	return millis;
}

export interface SoloRegionFixture {
	id: string;
	name: string;
	members: PlaybackAddress[];
}

export interface SoloRegionSnapshot {
	show_id: string;
	revision: number;
	regions: SoloRegionFixture[];
}

export interface SoloRegionSaveOutcome {
	request_id: string;
	show_id: string;
	revision: number;
	regions: SoloRegionFixture[];
	replayed: boolean;
	changed: boolean;
}

export async function soloRegionSnapshot(
	api: ApiDriver,
): Promise<SoloRegionSnapshot> {
	const showId = await activeShowId(api);
	return api.request<SoloRegionSnapshot>(
		"GET",
		"/api/v2/playback-solo-regions",
		undefined,
		true,
		undefined,
		{ showId },
	);
}

export async function saveSoloRegions(
	api: ApiDriver,
	regions: readonly SoloRegionFixture[],
	options: {
		expectedRevision?: number;
		requestId?: string;
	} = {},
) {
	const showId = await activeShowId(api);
	const requestId = options.requestId ?? crypto.randomUUID();
	return api.request<SoloRegionSaveOutcome>(
		"POST",
		"/api/v2/playback-solo-regions/update",
		{
			request_id: requestId,
			expected_revision: options.expectedRevision ?? 0,
			regions,
		},
		true,
		undefined,
		{ showId },
	);
}

export async function normalizedSoloRegions(
	api: ApiDriver,
): Promise<Array<{ name: string; members: PlaybackAddress[] }>> {
	const response = await soloRegionSnapshot(api);
	return response.regions
		.map((zone) => ({
			name: zone.name,
			members: [...zone.members],
		}))
		.sort((left, right) => left.name.localeCompare(right.name));
}
