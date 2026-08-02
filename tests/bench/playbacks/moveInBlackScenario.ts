import { expect, type Page } from "@playwright/test";
import type {
	PatchFixtureInput,
	PatchSnapshot,
} from "../../../apps/light-desktop/src/api/generated/light-wire";
import type {
	CueList,
	PlaybackDefinition,
} from "../../../apps/light-desktop/src/api/types/playback";
import { openPatch, patchFixtureRow } from "../../support/foundational/ui";
import type { ApiDriver } from "../core/api";
import { type ClockDuration, parseClockDuration } from "../core/clockScenario";
import type { DeskDriver } from "../core/desk";

interface PatchedFixtureBody {
	fixture_number: number;
	move_in_black_enabled: boolean;
	move_in_black_delay_millis: number;
	[key: string]: unknown;
}

interface MoveInBlackInstallation {
	enabledFixture: number;
	disabledFixture: number;
	playback: number;
	delay: ClockDuration;
}

interface MoveInBlackRuntime {
	fixture_id: string;
	cue_list_id: string;
	state: string;
	current_cue_number?: number;
	target_cue_number?: number;
	dark_since?: string | null;
	delay_deadline?: string | null;
}

export interface MoveInBlackStateExpectation {
	state: "blocked" | "delaying" | "moving" | "completed" | "disabled";
	currentCue?: number;
	targetCue?: number;
}

class MoveInBlackUiSurface {
	constructor(private readonly owner: BrowserMoveInBlack) {}

	selectFixture(number: number) {
		return this.owner.selectFixture(number);
	}

	setEnabled(number: number, enabled: boolean) {
		return this.owner.setEnabled(number, enabled);
	}

	setDelay(number: number, delay: ClockDuration) {
		return this.owner.setDelay(number, delay);
	}
}

/** Semantic setup, operator actions, and runtime assertions for Move in Black. */
export class BrowserMoveInBlack {
	readonly via = { ui: new MoveInBlackUiSurface(this) };
	private installation?: MoveInBlackInstallation & {
		cueListId: string;
		fixtureIds: Map<number, string>;
	};

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly showId: () => string,
	) {}

	async install(input: MoveInBlackInstallation): Promise<void> {
		const delayMillis = parseClockDuration(input.delay);
		const fixtureIds = new Map<number, string>();
		const patch = await this.api.request<PatchSnapshot>("GET", "/api/v2/patch");
		const fixtures: PatchFixtureInput[] = [];
		for (const [number, enabled] of [
			[input.enabledFixture, true],
			[input.disabledFixture, false],
		] as const) {
			const fixture = patch.fixtures.find(
				(candidate) => candidate.fixture_number === number,
			);
			if (!fixture) throw new Error(`Fixture ${number} is not patched`);
			fixtureIds.set(number, fixture.fixture_id);
			fixtures.push(patchFixtureInput(fixture, enabled, delayMillis));
		}
		await this.api.request(
			"POST",
			"/api/v2/patch/fixtures",
			{
				request_id: crypto.randomUUID(),
				fixtures,
				remove_fixture_ids: [],
				placements: [],
			},
			true,
			patch.patch_revision,
			{ showId: this.showId() },
		);
		const cueListId = crypto.randomUUID();
		await this.installCueList(cueListId, [...fixtureIds.values()]);
		await this.installPlayback(input.playback, cueListId);
		this.installation = { ...input, cueListId, fixtureIds };
		await this.desk.recordStep(
			"MOVE IN BLACK SETUP",
			"Install a three-Cue look-ahead contract with one enabled and one disabled moving fixture.",
		);
	}

	async expectConfiguration(
		number: number,
		expected: { enabled: boolean; delay: ClockDuration },
	): Promise<void> {
		const delayMillis = parseClockDuration(expected.delay);
		await expect
			.poll(async () => {
				const fixture = await this.fixtureObject(number);
				return {
					enabled: fixture.body.move_in_black_enabled,
					delayMillis: fixture.body.move_in_black_delay_millis,
				};
			})
			.toEqual({ enabled: expected.enabled, delayMillis });
		await this.openPatch();
		const value = expected.enabled ? formatDelay(delayMillis) : "Off";
		await expect(
			this.page.getByRole("button", { name: `MIB ${number}: ${value}` }),
		).toHaveText(value);
	}

	async expectState(
		number: number,
		expected: MoveInBlackStateExpectation,
	): Promise<void> {
		await expect
			.poll(async () => {
				const runtime = await this.runtime(number);
				return {
					state: runtime.state,
					...(expected.currentCue == null
						? {}
						: { currentCue: runtime.current_cue_number }),
					...(expected.targetCue == null
						? {}
						: { targetCue: runtime.target_cue_number }),
				};
			})
			.toEqual(expected);
	}

	async expectSafetyDelay(number: number, delay: ClockDuration): Promise<void> {
		const expectedMillis = parseClockDuration(delay);
		await expect
			.poll(async () => {
				const runtime = await this.runtime(number);
				if (!runtime.dark_since || !runtime.delay_deadline) return null;
				return (
					Date.parse(runtime.delay_deadline) - Date.parse(runtime.dark_since)
				);
			})
			.toBe(expectedMillis);
	}

	async reset(): Promise<void> {
		const installation = this.requiredInstallation();
		await this.api.request(
			"POST",
			"/api/v2/test/clock/reset",
			undefined,
			false,
		);
		await this.api.cueListPlaybackAction(installation.cueListId, "release", {});
		await this.api.playbackNumberAction(installation.playback, "off", {});
	}

	async reopenPatch(): Promise<void> {
		await this.page.reload();
		await expect(this.page.locator(".connection-cover")).toBeHidden({
			timeout: 10_000,
		});
		await this.openPatch();
	}

	async selectFixture(number: number): Promise<void> {
		await this.openPatch();
		const before = await this.fixtureObject(number);
		await this.desk.recordStep(
			`SELECT FIXTURE ${number}`,
			"Select the Patch row without entering SET-gated Move in Black editing.",
		);
		await this.page
			.getByRole("button", { name: new RegExp(`^MIB ${number}:`) })
			.click();
		await expect(this.page.locator(".patch-edit-modal")).toHaveCount(0);
		expect((await this.fixtureObject(number)).revision).toBe(before.revision);
	}

	async setEnabled(number: number, enabled: boolean): Promise<void> {
		const fixture = await this.fixtureObject(number);
		const value = enabled
			? String(fixture.body.move_in_black_delay_millis / 1_000)
			: "Off";
		await this.edit(number, "Move in Black", async (editor) => {
			await expect(
				editor.getByRole("heading", { name: "Set fixture MIB" }),
			).toBeVisible();
			await editor
				.getByLabel("MIB value: Off or non-negative seconds")
				.fill(value);
		});
	}

	async setDelay(number: number, delay: ClockDuration): Promise<void> {
		const millis = parseClockDuration(delay);
		await this.edit(number, "MIB Delay", async (editor) => {
			await expect(
				editor.getByRole("heading", { name: "Set fixture MIB" }),
			).toBeVisible();
			await editor
				.getByLabel("MIB value: Off or non-negative seconds")
				.fill(String(millis / 1_000));
		});
	}

	private async edit(
		number: number,
		field: "Move in Black" | "MIB Delay",
		update: (editor: ReturnType<Page["locator"]>) => Promise<void>,
	): Promise<void> {
		await this.openPatch();
		const before = await this.fixtureObject(number);
		await this.desk.recordStep(
			`SET FIXTURE ${number} ${field.toUpperCase()}`,
			`Edit ${field} through the visible SET-gated Patch workflow.`,
		);
		await this.page.getByRole("button", { name: "SET", exact: true }).click();
		await this.page
			.getByRole("button", { name: new RegExp(`^MIB ${number}:`) })
			.click();
		const editor = this.page.locator(".patch-edit-modal");
		await update(editor);
		await this.desk.click(
			editor.getByRole("button", { name: "Set", exact: true }),
		);
		await expect
			.poll(async () => (await this.fixtureObject(number)).revision)
			.toBeGreaterThan(before.revision);
	}

	private async openPatch(): Promise<void> {
		if (await this.page.getByRole("button", { name: /^MIB 101:/ }).count())
			return;
		await openPatch(this.page);
		await expect(patchFixtureRow(this.page, 101)).toBeVisible();
	}

	private async runtime(number: number): Promise<MoveInBlackRuntime> {
		const installation = this.requiredInstallation();
		const fixtureId = installation.fixtureIds.get(number);
		if (!fixtureId)
			throw new Error(`Fixture ${number} is outside this Move in Black setup`);
		const diagnostics = await this.api.request<{
			move_in_black: MoveInBlackRuntime[];
		}>("GET", "/api/v2/diagnostics");
		const runtime = diagnostics.move_in_black.find(
			(entry) =>
				entry.fixture_id === fixtureId &&
				entry.cue_list_id === installation.cueListId,
		);
		if (!runtime)
			throw new Error(`No Move in Black runtime for Fixture ${number}`);
		return runtime;
	}

	private async fixtureObject(number: number) {
		const fixture = (await this.api.patch()).fixtures.find(
			(candidate) => candidate.fixture_number === number,
		);
		if (!fixture) throw new Error(`Fixture ${number} is not patched`);
		const object = await this.api.showObject<PatchedFixtureBody>(
			this.showId(),
			"patched_fixture",
			fixture.fixture_id,
		);
		if (!object)
			throw new Error(`Fixture ${number} has no persisted Patch object`);
		return object;
	}

	private async installCueList(
		cueListId: string,
		fixtureIds: string[],
	): Promise<void> {
		const change = (
			fixtureId: string,
			attribute: string,
			value: number,
			fadeMillis?: number,
		) => ({
			fixture_id: fixtureId,
			attribute,
			value: { kind: "normalized" as const, value },
			automatic_restore: false,
			...(fadeMillis == null ? {} : { fade_millis: fadeMillis }),
		});
		const cue = (
			number: number,
			changes: ReturnType<typeof change>[],
			fadeMillis = 0,
		) => ({
			id: crypto.randomUUID(),
			number,
			name: `Cue ${number}`,
			changes,
			group_changes: [],
			fade_millis: fadeMillis,
			delay_millis: 0,
			trigger: { type: "manual" as const },
		});
		const body: CueList = {
			id: cueListId,
			name: "Move in Black",
			priority: 10,
			mode: "sequence",
			looped: false,
			chaser_step_millis: 1_000,
			speed_group: null,
			intensity_priority_mode: "htp",
			wrap_mode: "off",
			restart_mode: "first_cue",
			force_cue_timing: false,
			disable_cue_timing: false,
			chaser_xfade_millis: 0,
			speed_multiplier: 1,
			cues: [
				cue(
					1,
					fixtureIds.flatMap((id) => [
						change(id, "intensity", 1),
						change(id, "pan", 0.2),
					]),
				),
				cue(
					2,
					fixtureIds.map((id) => change(id, "intensity", 0)),
					2_000,
				),
				cue(
					3,
					fixtureIds.flatMap((id) => [
						change(id, "intensity", 1),
						change(id, "pan", 0.8, 3_000),
					]),
				),
			],
		};
		await this.api.seedShowObject(this.showId(), "cue_list", cueListId, body);
	}

	private async installPlayback(
		number: number,
		cueListId: string,
	): Promise<void> {
		const playback: PlaybackDefinition = {
			number,
			name: "MIB",
			target: { type: "cue_list", cue_list_id: cueListId },
			buttons: ["go_minus", "go", "flash"],
			fader: "master",
			go_activates: true,
			auto_off: true,
			xfade_millis: 0,
			color: "#20c997",
			flash_release: "release_all",
			protect_from_swap: false,
		};
		await this.api.seedShowObject(
			this.showId(),
			"playback",
			String(number),
			playback,
			(await this.api.showObject(this.showId(), "playback", String(number)))
				?.revision ?? 0,
		);
		const page =
			(await this.api.showObject<Record<string, unknown>>(
				this.showId(),
				"playback_page",
				"1",
			)) ?? null;
		const body = page?.body ?? {};
		await this.api.seedShowObject(
			this.showId(),
			"playback_page",
			"1",
			{
				...body,
				number: 1,
				name: typeof body.name === "string" ? body.name : "Main",
				slots: {
					...(isRecord(body.slots) ? body.slots : {}),
					[String(number)]: number,
				},
				virtual_playbacks: isRecord(body.virtual_playbacks)
					? body.virtual_playbacks
					: {},
			},
			page?.revision ?? 0,
		);
	}

	private requiredInstallation() {
		if (!this.installation)
			throw new Error("Move in Black setup has not been installed");
		return this.installation;
	}
}

function patchFixtureInput(
	fixture: PatchSnapshot["fixtures"][number],
	enabled: boolean,
	delayMillis: number,
): PatchFixtureInput {
	const {
		fixture_revision: _fixtureRevision,
		logical_heads: _logicalHeads,
		...input
	} = fixture;
	return {
		...input,
		move_in_black_enabled: enabled,
		move_in_black_delay_millis: delayMillis,
	};
}

function formatDelay(millis: number): string {
	return `${Number((millis / 1_000).toFixed(3))} s`;
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}
