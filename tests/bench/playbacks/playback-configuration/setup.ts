import type { ApiDriver } from "../../core/api";
import {
	createCueList,
	definition,
	installPlaybacks,
	poolAction,
	setSpeedRates,
	writePage,
} from "./api";

/** Deterministic setup catalog shared by semantic Playback Configuration cases. */
export class PlaybackConfigurationSetup {
	private readonly cueLists = new Map<string, string>();

	constructor(
		private readonly api: ApiDriver,
		private readonly showId: () => string,
	) {}

	async prepareInspection(): Promise<void> {
		await this.prepareBase();
		const cueListId = await this.sequence(
			"Configured Sequence",
			[0.2, 0.8, 0.4],
		);
		await installPlaybacks(
			this.api,
			[
				definition("Configured Sequence", {
					type: "cue_list",
					cue_list_id: cueListId,
				}),
			],
		);
		await poolAction(this.api, 1, "go");
		await poolAction(this.api, 1, "master", { value: 0.6 });
	}

	async prepareAssignment(): Promise<void> {
		await this.prepareBase();
		await this.sequence("Configured Sequence", [0.2, 0.8, 0.4]);
	}

	async prepareActionMatrix(): Promise<void> {
		await this.prepareBase();
		const cueListId = await this.sequence(
			"Configured Sequence",
			[0.2, 0.8, 0.4],
		);
		await installPlaybacks(
			this.api,
			[
				definition("Action Matrix", {
					type: "cue_list",
					cue_list_id: cueListId,
				}),
			],
		);
	}

	async prepareCrossfade(): Promise<void> {
		await this.prepareBase();
		const cueListId = await this.sequence(
			"Manual Crossfade",
			[0, 1, 0.5],
			30_000,
			10_000,
		);
		await installPlaybacks(
			this.api,
			[
				definition("Manual Crossfade",
					{ type: "cue_list", cue_list_id: cueListId },
					{ fader: "x_fade" },
				),
			],
		);
		await poolAction(this.api, 1, "on");
	}

	async prepareTempAndSwap(): Promise<void> {
		await this.prepareBase();
		const configuration = await this.api.request<{
			configuration: Record<string, unknown>;
		}>("GET", "/api/v2/configuration");
		await this.api.request("PUT", "/api/v2/configuration", {
			...configuration.configuration,
			sequence_master_fade_millis: 0,
		});
		const fixtures = await this.fixtureIds();
		const sequence = (name: string, level: number, fixtureNumbers: number[]) =>
			createCueList(
				this.api,
				fixtures,
				name,
				[level],
				0,
				0,
				fixtureNumbers,
				false,
			);
		const [underlying, temporary, unprotected, protectedId] = await Promise.all(
			[
				sequence("Underlying", 0.3, [1]),
				sequence("Temporary", 0.8, [1]),
				sequence("Unprotected", 0.6, [2]),
				sequence("Protected", 0.4, [3]),
			],
		);
		await installPlaybacks(
			this.api,
			[
				definition("Underlying", {
					type: "cue_list",
					cue_list_id: underlying,
				}),
				definition("Temporary",
					{ type: "cue_list", cue_list_id: temporary },
					{ buttons: ["swap", "temp", "flash"] },
				),
				definition("Unprotected",
					{ type: "cue_list", cue_list_id: unprotected },
					{ auto_off: false },
				),
				definition("Protected",
					{ type: "cue_list", cue_list_id: protectedId },
					{ auto_off: false, protect_from_swap: true },
				),
			],
		);
		for (const number of [1, 3, 4]) await poolAction(this.api, number, "on");
	}

	async prepareMasters(): Promise<void> {
		await this.prepareBase();
		await setSpeedRates(this.api, [120, 96, 72, 60, 48]);
		await installPlaybacks(
			this.api,
			[
				definition("Speed A",
					{ type: "speed_group", group: "A" },
					{ color: "#8b5cf6" },
				),
				definition("Group 1", { type: "group", group_id: "1" }),
				definition("Grand", { type: "grand_master" }),
				definition("Programmer Fade", { type: "programmer_fade" }),
				definition("Cue Fade", { type: "cue_fade" }),
			],
		);
	}

	clearPage(): Promise<void> {
		return writePage(this.api, 1, {});
	}

	requiredCueList(name: string): string {
		const id = this.cueLists.get(name);
		if (!id) throw new Error(`Cuelist "${name}" has not been prepared`);
		return id;
	}

	async fixtureIds(): Promise<Record<number, string>> {
		return Object.fromEntries(
			(await this.api.patch()).fixtures.map((fixture) => [
				fixture.fixture_number,
				fixture.fixture_id,
			]),
		);
	}

	private async prepareBase(): Promise<void> {
		this.cueLists.clear();
		await this.ensureGroups();
		await this.clearPage();
	}

	private async sequence(
		name: string,
		levels: number[],
		fadeMillis = 0,
		delayMillis = 0,
	): Promise<string> {
		const id = await createCueList(
			this.api,
			await this.fixtureIds(),
			name,
			levels,
			fadeMillis,
			delayMillis,
			[1, 2],
		);
		this.cueLists.set(name, id);
		return id;
	}

	private async ensureGroups(): Promise<void> {
		const fixtures = await this.fixtureIds();
		const existing = await this.api.showObjects<Record<string, unknown>>(
			this.showId(),
			"group",
		);
		for (const [id, name, members] of [
			["1", "All Fixtures", Object.values(fixtures)],
			["3", "Front Fixtures", Object.values(fixtures).slice(0, 4)],
		] as const) {
			if (existing.some((group) => group.id === id)) continue;
			await this.api.seedShowObject(this.showId(), "group", id, {
				id,
				name,
				fixtures: members,
				derived_from: null,
				frozen_from: null,
				programming: {},
			});
		}
	}
}
