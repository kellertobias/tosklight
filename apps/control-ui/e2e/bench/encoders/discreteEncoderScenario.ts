import { expect, type Page } from "@playwright/test";
import type { PatchedFixture } from "../../../src/api/types";
import {
	parameterFamilies,
	parameterLabels,
} from "../../../src/components/control/parameterControls/model";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import {
	batchProgrammerValues,
	clearProgrammerValues,
} from "../programmer/programmerValues";
import type { BrowserSelection } from "../command-selection/selectionScenario";

export interface DiscreteChoice {
	semanticId: string;
	label: string;
}

interface ResolvedDiscreteChoice extends DiscreteChoice {
	fixtureIds: string[];
}

export function discreteChoicesForSelection(
	fixtures: readonly PatchedFixture[],
	selectedFixtureIds: readonly string[],
	attribute: string,
): DiscreteChoice[] {
	return resolveDiscreteChoices(fixtures, selectedFixtureIds, attribute).map(
		({ semanticId, label }) => ({ semanticId, label }),
	);
}

export class BrowserDiscreteEncoders {
	constructor(
		private readonly api: ApiDriver,
		private readonly selection: BrowserSelection,
		private readonly desk: DeskDriver,
		private readonly page: Page,
	) {}

	async choices(attribute: string): Promise<DiscreteChoice[]> {
		const selected = (await this.selection.observe()).selected;
		return discreteChoicesForSelection(
			await this.fixtures(),
			selected,
			attribute,
		);
	}

	async set(attribute: string, semanticId: string): Promise<void> {
		const context = await this.context();
		const choice = resolveDiscreteChoices(
			context.fixtures,
			context.selected,
			attribute,
		).find((candidate) => candidate.semanticId === semanticId);
		if (!choice)
			throw new Error(
				`Discrete value ${semanticId} is not supplied for ${attribute} by the active selection`,
			);
		await this.desk.recordStep(
			"DISCRETE ENCODER",
			`Set ${attribute} to ${choice.label} (${choice.semanticId}) on compatible selected fixtures.`,
		);
		await batchProgrammerValues(this.api, {
			surface: "api",
			showId: context.showId,
			mutations: choice.fixtureIds.map((fixtureId) => ({
				action: "set_fixture",
				fixtureId,
				attribute,
				value: { kind: "discrete", value: choice.semanticId },
				timing: {
					fade: context.fadeMillis > 0,
					fadeMillis: context.fadeMillis || null,
					delayMillis: null,
				},
			})),
		});
	}

	async release(attribute: string): Promise<void> {
		const context = await this.context();
		await batchProgrammerValues(this.api, {
			surface: "api",
			showId: context.showId,
			mutations: context.selected.map((fixtureId) => ({
				action: "release_fixture",
				fixtureId,
				attribute,
			})),
		});
	}

	async releaseVisible(attribute: string): Promise<void> {
		const family = Object.entries(parameterFamilies).find(([, attributes]) =>
			(attributes as readonly string[]).includes(attribute),
		)?.[0];
		if (!family)
			throw new Error(`${attribute} has no visible Programmer family`);
		const label = parameterLabels[attribute] ?? attribute.replaceAll(".", " ");
		await this.desk.click(
			this.page.getByRole("button", { name: family, exact: true }),
		);
		const control = this.page
			.locator(".vertical-touch-fader-stack")
			.filter({
				has: this.page.getByRole("slider", {
					name: new RegExp(`^Enc \\d+ · ${escapeRegex(label)}$`),
				}),
			});
		await expect(control).toBeVisible();
		await this.desk.click(
			control.getByRole("button", {
				name: `Release ${label}`,
				exact: true,
			}),
		);
	}

	async clear(): Promise<void> {
		const { showId } = await this.context();
		await clearProgrammerValues(this.api, { surface: "api", showId });
	}

	private async context() {
		const [selection, fixtures, bootstrap, configurationResponse] =
			await Promise.all([
				this.selection.observe(),
				this.fixtures(),
				this.api.request<{ active_show: { id: string } | null }>(
					"GET",
					"/api/v2/bootstrap",
				),
				this.api.request<{
					configuration?: { programmer_fade_millis: number };
					programmer_fade_millis?: number;
				}>("GET", "/api/v2/configuration"),
			]);
		if (!bootstrap.active_show) throw new Error("No active Show");
		if (selection.selected.length === 0)
			throw new Error("Discrete encoder action requires a Fixture selection");
		const configuration =
			configurationResponse.configuration ?? configurationResponse;
		return {
			selected: selection.selected,
			fixtures,
			showId: bootstrap.active_show.id,
			fadeMillis: configuration.programmer_fade_millis ?? 0,
		};
	}

	private async fixtures(): Promise<PatchedFixture[]> {
		const patch = await this.api.patch();
		return (Array.isArray(patch) ? patch : patch.fixtures) as PatchedFixture[];
	}
}

function escapeRegex(value: string): string {
	return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function resolveDiscreteChoices(
	fixtures: readonly PatchedFixture[],
	selectedFixtureIds: readonly string[],
	attribute: string,
): ResolvedDiscreteChoice[] {
	const selected = new Set(selectedFixtureIds);
	const choices = new Map<string, ResolvedDiscreteChoice>();
	for (const fixture of fixtures) {
		const selectedIds = selectedIdsForFixture(fixture, selected);
		if (selectedIds.length === 0) continue;
		const profile = fixture.definition.profile_snapshot;
		const mode = profile?.modes.find(
			(candidate) => candidate.id === fixture.definition.mode_id,
		);
		if (!mode) continue;
		for (const channel of mode.channels) {
			if (channel.attribute !== attribute) continue;
			const targets = selectedTargetsForChannel(
				fixture,
				selectedIds,
				channel.head_id,
			);
			if (targets.length === 0) continue;
			for (const fn of channel.functions) {
				if (fn.behavior.type !== "fixed" && fn.behavior.type !== "indexed")
					continue;
				const key = `${fn.behavior.semantic_id}\u0000${fn.behavior.label}`;
				const choice = choices.get(key) ?? {
					semanticId: fn.behavior.semantic_id,
					label: fn.behavior.label,
					fixtureIds: [],
				};
				for (const fixtureId of targets)
					if (!choice.fixtureIds.includes(fixtureId))
						choice.fixtureIds.push(fixtureId);
				choices.set(key, choice);
			}
		}
	}
	return [...choices.values()].sort(
		(left, right) =>
			left.label.localeCompare(right.label) ||
			left.semanticId.localeCompare(right.semanticId),
	);
}

function selectedIdsForFixture(
	fixture: PatchedFixture,
	selected: ReadonlySet<string>,
): string[] {
	if (selected.has(fixture.fixture_id)) return [fixture.fixture_id];
	return fixture.logical_heads
		.map((head) => head.fixture_id)
		.filter((fixtureId) => selected.has(fixtureId));
}

function selectedTargetsForChannel(
	fixture: PatchedFixture,
	selectedIds: readonly string[],
	headId: string,
): string[] {
	if (selectedIds.includes(fixture.fixture_id)) return [fixture.fixture_id];
	const profile = fixture.definition.profile_snapshot;
	const mode = profile?.modes.find(
		(candidate) => candidate.id === fixture.definition.mode_id,
	);
	const headIndex = mode?.heads.findIndex((head) => head.id === headId) ?? -1;
	const logicalId = fixture.logical_heads.find(
		(head) =>
			head.profile_head_id === headId ||
			(head.profile_head_id == null && head.head_index === headIndex),
	)?.fixture_id;
	return logicalId && selectedIds.includes(logicalId) ? [logicalId] : [];
}
