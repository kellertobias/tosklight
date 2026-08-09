import { expect, type Page } from "@playwright/test";
import type { ApiDriver } from "../../core/api";
import type { DeskDriver } from "../../core/desk";
import {
	activePlayback,
	inertSnapshot,
	pageObject,
	playbackAt,
	playbackSnapshot,
} from "./api";
import {
	authoritativeMasterObservation,
	hasSwapRuntime,
	hasTemporaryRuntime,
	playbackConfigurationObservation,
} from "./observations";
import { PlaybackConfigurationSetup } from "./setup";
import {
	armSet,
	choosePlaybackColor,
	chooseSelect,
	expectConfigurationModal,
	openPlaybackMode,
	playbackCard,
	playbackSlider,
} from "./ui";

export interface PlaybackConfigurationExpectation {
	number: number;
	targetType: string;
	buttons: string[];
	buttonCount: number;
	fader: string;
	hasFader: boolean;
	color: string;
}

export interface AuthoritativeMasterExpectation {
	speed: { manualBpm: number; effectiveBpm: number; paused: boolean };
	neighborBpms: number[];
	group: { master: number; flashLevel: number };
	grand: {
		level: number;
		effectiveLevel: number;
		blackout: boolean;
		dynamicsPaused: boolean;
	};
	programmerFadeMillis: number;
	cueFadeMillis: number;
}

/** Semantic Playback Configuration setup, visible editing, and focused oracles. */
export class BrowserPlaybackConfiguration {
	private readonly setup: PlaybackConfigurationSetup;

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly showId: () => string,
	) {
		this.setup = new PlaybackConfigurationSetup(api, showId);
	}

	prepareInspection() {
		return this.setup.prepareInspection();
	}

	prepareAssignment() {
		return this.setup.prepareAssignment();
	}

	prepareActionMatrix() {
		return this.setup.prepareActionMatrix();
	}

	prepareCrossfade() {
		return this.setup.prepareCrossfade();
	}

	prepareTempAndSwap() {
		return this.setup.prepareTempAndSwap();
	}

	prepareMasters() {
		return this.setup.prepareMasters();
	}

	clearPage() {
		return this.setup.clearPage();
	}

	async expectInspectionWithoutMutation(
		slot: number,
		expected: PlaybackConfigurationExpectation,
	): Promise<void> {
		const before = await inertSnapshot(this.api, expected.number);
		await this.open(slot);
		const modal = await expectConfigurationModal(this.page, 1, slot);
		for (const section of ["Function", "Behavior", "Layout"])
			await expect(
				modal.getByRole("button", { name: section, exact: true }),
			).toBeVisible();
		const cueListId = this.setup.requiredCueList("Configured Sequence");
		expect(
			await playbackConfigurationObservation(this.api, 1, slot, cueListId),
		).toEqual({
			page: 1,
			slot,
			targetMatchesExpected: true,
			...expected,
		});
		await this.close(modal);
		expect(await inertSnapshot(this.api, expected.number)).toEqual(before);
	}

	async assignCueList(
		slot: number,
		name: string,
		color: string,
	): Promise<void> {
		await this.open(slot);
		const modal = await expectConfigurationModal(this.page, 1, slot);
		await modal.getByRole("radio", { name, exact: true }).click();
		await choosePlaybackColor(this.page, modal, color);
		await this.apply(modal);
	}

	async clear(slot: number): Promise<void> {
		await this.open(slot);
		const modal = await expectConfigurationModal(this.page, 1, slot);
		await modal.getByRole("radio", { name: "None", exact: true }).click();
		await expect(
			modal.getByText("Playback will be cleared", { exact: true }),
		).toBeVisible();
		await this.apply(modal);
	}

	async setButton(
		slot: number,
		position: "Top button" | "Middle button" | "Bottom button",
		action: string,
	): Promise<void> {
		await this.open(slot);
		const modal = await expectConfigurationModal(this.page, 1, slot);
		await modal.getByRole("button", { name: "Layout", exact: true }).click();
		await chooseSelect(this.page, modal, position, action);
		await this.apply(modal);
	}

	async assignGroupMaster(slot: number, group: number): Promise<void> {
		await this.open(slot);
		const modal = await expectConfigurationModal(this.page, 1, slot);
		await modal
			.getByRole("radio", { name: "Group Master", exact: true })
			.click();
		await modal
			.getByRole("radio", { name: `Group ${group}`, exact: true })
			.click();
		await this.apply(modal);
		await expect
			.poll(async () => (await playbackAt(this.api, 1, slot)).body.target)
			.toEqual({ type: "group", group_id: String(group) });
	}

	async assignGrandMaster(slot: number): Promise<void> {
		await this.open(slot);
		const modal = await expectConfigurationModal(this.page, 1, slot);
		await modal.getByRole("radio", { name: "Special", exact: true }).click();
		await modal
			.getByRole("radio", { name: "Grand Master", exact: true })
			.click();
		await this.apply(modal);
		await expect
			.poll(async () => (await playbackAt(this.api, 1, slot)).body.target)
			.toEqual({ type: "grand_master" });
	}

	async press(slot: number, label: string): Promise<void> {
		await openPlaybackMode(this.page);
		await this.desk.click(
			playbackCard(this.page, slot).getByRole("button", {
				name: label,
				exact: true,
			}),
		);
	}

	async setSlotFader(slot: number, value: number): Promise<void> {
		if (!Number.isFinite(value) || value < 0 || value > 1)
			throw new Error("Playback fader values run from 0 through 1");
		await openPlaybackMode(this.page);
		const target = (await playbackAt(this.api, 1, slot)).body.target;
		const slider = playbackSlider(this.page, slot);
		if (target.type === "group" || target.type === "grand_master")
			await slider.fill("100");
		await slider.fill(String(value * 100));
		if (target.type === "group")
			await expect
				.poll(async () => {
					const snapshot = await playbackSnapshot(this.api);
					return snapshot.authoritative_controls.groups.find(
						(group: { id: string }) => group.id === target.group_id,
					)?.master;
				})
				.toBeCloseTo(value, 5);
		if (target.type === "grand_master")
			await expect
				.poll(
					async () =>
						(await playbackSnapshot(this.api)).authoritative_controls
							.grand_master.level,
				)
				.toBeCloseTo(value, 5);
	}

	async expectAssignment(
		slot: number,
		expected: Partial<PlaybackConfigurationExpectation> & {
			cueList?: string;
		},
	): Promise<void> {
		const playback = await playbackAt(this.api, 1, slot);
		expect(playback.body).toMatchObject({
			...(expected.number == null ? {} : { number: expected.number }),
			...(expected.targetType == null
				? {}
				: expected.targetType === "cue_list" && expected.cueList
					? {
							target: {
								type: "cue_list",
								cue_list_id: this.setup.requiredCueList(expected.cueList),
							},
						}
					: { target: { type: expected.targetType } }),
			...(expected.buttons == null ? {} : { buttons: expected.buttons }),
			...(expected.buttonCount == null
				? {}
				: { button_count: expected.buttonCount }),
			...(expected.fader == null ? {} : { fader: expected.fader }),
			...(expected.hasFader == null ? {} : { has_fader: expected.hasFader }),
			...(expected.color == null ? {} : { color: expected.color }),
		});
	}

	async expectUnassigned(slot: number): Promise<void> {
		await expect
			.poll(
				async () => (await pageObject(this.api, 1)).body.slots[String(slot)],
			)
			.toBeUndefined();
	}

	async expectCueListPresent(name: string): Promise<void> {
		const id = this.setup.requiredCueList(name);
		expect(
			(await this.api.showObjects(this.showId(), "cue_list")).some(
				(item) => item.id === id,
			),
		).toBe(true);
	}

	async selectContentsWithoutPlaybackMutation(
		slot: number,
		playbackNumber: number,
		expected: { fixtures: number[]; group: number },
	): Promise<void> {
		const before = await activePlayback(this.api, playbackNumber);
		await this.press(slot, "SELECT CONTENTS");
		await expect
			.poll(async () => (await this.programmer()).selection_expression?.type)
			.toBe("playback_contents");
		expect(await activePlayback(this.api, playbackNumber)).toEqual(before);
		const fixtureIds = await this.setup.fixtureIds();
		expect((await this.programmer()).selection_expression).toEqual({
			type: "playback_contents",
			items: [
				...expected.fixtures.map((number) => ({
					type: "fixture",
					fixture_id: fixtureIds[number],
				})),
				{ type: "live_group", group_id: String(expected.group) },
			],
		});
	}

	async expectTemporary(number: number, active: boolean): Promise<void> {
		await expect
			.poll(async () =>
				hasTemporaryRuntime(await playbackSnapshot(this.api), number),
			)
			.toBe(active);
	}

	async expectSwap(number: number, active: boolean): Promise<void> {
		await expect
			.poll(async () =>
				hasSwapRuntime(await playbackSnapshot(this.api), number),
			)
			.toBe(active);
	}

	async expectMasters(expected: AuthoritativeMasterExpectation): Promise<void> {
		await expect
			.poll(async () => {
				const snapshot = await playbackSnapshot(this.api);
				return authoritativeMasterObservation(snapshot.authoritative_controls);
			})
			.toEqual(expected);
	}

	private async programmer(): Promise<{
		selection_expression?: {
			type?: string;
			items?: Array<Record<string, unknown>>;
		} | null;
	}> {
		const sessionId = this.api.session?.session_id;
		const programmers = await this.api.request<
			Array<{
				session_id: string;
				selection_expression?: {
					type?: string;
					items?: Array<Record<string, unknown>>;
				} | null;
			}>
		>("GET", "/api/v2/programmers");
		const state =
			programmers.find((candidate) => candidate.session_id === sessionId) ??
			programmers[0];
		if (!state) throw new Error("No Programmer projection is available");
		return state;
	}

	private async open(slot: number): Promise<void> {
		await openPlaybackMode(this.page);
		await armSet(this.page);
		await this.page
			.getByRole("button", {
				name: `Playback representation page 1 playback ${slot}`,
			})
			.click();
	}

	private async apply(
		modal: Awaited<ReturnType<typeof expectConfigurationModal>>,
	) {
		await this.desk.click(
			modal.getByRole("button", { name: "Apply", exact: true }),
		);
		await expect(modal).toBeHidden();
	}

	private async close(
		modal: Awaited<ReturnType<typeof expectConfigurationModal>>,
	) {
		await this.desk.click(
			modal.getByRole("button", {
				name: "Close playback configuration",
				exact: true,
			}),
		);
		await expect(modal).toBeHidden();
	}
}
