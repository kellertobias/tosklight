import fs from "node:fs/promises";
import { expect } from "@playwright/test";
import type { ApiDriver } from "./api";
import type { DeskDriver } from "./desk";
import type { LightBench, TestShow } from "./lightBench";

export enum Show {
	Empty = "empty",
	TwelveDimmers = "twelve-dimmers",
	CompactRig = "compact-rig",
	DefaultStage = "default-stage",
}

export interface ShowPrerequisites {
	fixtureNumbers?: readonly number[];
	profiles?: readonly string[];
	groups?: readonly string[];
	desktops?: readonly string[];
}

export interface DefinedShow {
	readonly name: string;
}

interface RegisteredShow {
	base: Show;
	requires: ShowPrerequisites;
}

interface WorkingShow {
	catalogName: string;
	canonicalId: string;
	workingId: string;
	workingName: string;
	canonicalBytes: Buffer;
}

const recipes = new Map<string, RegisteredShow>();
const fixtureUrl = (name: "compact-rig" | "default-stage") =>
	new URL(`../../../../tests/fixtures/${name}.show`, import.meta.url);

const initialCatalog: Readonly<Record<Show, ShowPrerequisites>> = {
	[Show.Empty]: { fixtureNumbers: [], profiles: [], groups: [], desktops: [] },
	[Show.TwelveDimmers]: {
		fixtureNumbers: Array.from({ length: 12 }, (_, index) => index + 1),
		profiles: ["Generic Dimmer"],
		groups: ["1", "2", "3"],
		desktops: [],
	},
	[Show.CompactRig]: {
		fixtureNumbers: [
			...Array.from({ length: 12 }, (_, index) => index + 1),
			21,
			22,
			23,
			24,
		],
		groups: ["1", "2", "3", "4"],
		desktops: [],
	},
	[Show.DefaultStage]: { fixtureNumbers: [1], groups: [], desktops: [] },
};

class ShowRecipeBuilder {
	baseShow: Show | null = null;
	prerequisites: ShowPrerequisites = {};

	from(show: Show): this {
		this.baseShow = show;
		return this;
	}

	requires(prerequisites: ShowPrerequisites): this {
		this.prerequisites = prerequisites;
		return this;
	}
}

export function defineShow(
	name: string,
	recipe: (show: ShowRecipeBuilder) => void,
): DefinedShow {
	if (!name.trim()) throw new Error("Defined show name must not be empty");
	if (recipes.has(name))
		throw new Error(`Show recipe "${name}" is already registered`);
	const builder = new ShowRecipeBuilder();
	recipe(builder);
	if (!builder.baseShow)
		throw new Error(`Show recipe "${name}" must select a catalog base`);
	recipes.set(name, {
		base: builder.baseShow,
		requires: builder.prerequisites,
	});
	return Object.freeze({ name });
}

export class BrowserShows {
	private current?: WorkingShow;

	constructor(
		private readonly api: ApiDriver,
		private readonly bench: LightBench,
		private readonly desk: DeskDriver,
		private readonly initialShow: TestShow,
	) {}

	readonly expect = {
		active: async (show: Show | DefinedShow) => {
			const current = this.requireCurrent();
			const expectedName = showName(show);
			if (current.catalogName !== expectedName) {
				throw new Error(
					`Active working copy is "${current.catalogName}", not "${expectedName}"`,
				);
			}
			const bootstrap = await this.api.request<{
				active_show: { id: string; name?: string } | null;
			}>("GET", "/api/v2/bootstrap", undefined, false);
			expect(bootstrap.active_show?.id).toBe(current.workingId);
			const entry = (
				await this.api.shows<Array<{ id: string; name: string }>[number]>()
			).find((candidate) => candidate.id === current.workingId);
			expect(entry?.name).toBe(current.workingName);
			expect(current.workingId).not.toBe(current.canonicalId);
		},
	};

	async use(show: Show | DefinedShow): Promise<void> {
		const resolved = resolveShow(show);
		await this.desk.recordStep(
			"FIXTURE SETUP",
			`Creating an isolated ${showName(show)} working copy. This is labelled setup, not an operator action.`,
		);
		await this.resetBenchState();
		const canonical = await this.createCanonical(resolved.base);
		const canonicalBytes = await this.download(canonical.id);
		const workingName = `${showName(show)}-working-${crypto.randomUUID()}`;
		const working = await this.api.createShow<{ id: string }>({
			name: workingName,
			data_base64: canonicalBytes.toString("base64"),
			overwrite: false,
		});
		await this.api.openShow(working.id, { transition: "hold_current" });
		await this.routeOutputs(working.id);
		this.current = {
			catalogName: showName(show),
			canonicalId: canonical.id,
			workingId: working.id,
			workingName,
			canonicalBytes,
		};
		await this.validatePrerequisites(
			mergeRequirements(initialCatalog[resolved.base], resolved.requires),
		);
		await this.resetBenchState();
	}

	async resetWorkingCopy(): Promise<void> {
		const current = this.requireCurrent();
		const workingName = `${current.catalogName}-working-${crypto.randomUUID()}`;
		const replacement = await this.api.createShow<{ id: string }>({
			name: workingName,
			data_base64: current.canonicalBytes.toString("base64"),
			overwrite: false,
		});
		await this.api.openShow(replacement.id, { transition: "hold_current" });
		await this.routeOutputs(replacement.id);
		this.current = { ...current, workingId: replacement.id, workingName };
		await this.resetBenchState();
	}

	/** Framework-contract inspection only; scenario authors should use semantic expectations. */
	contractIdentity(): Readonly<{
		dataDir: string;
		canonicalId: string;
		workingId: string;
	}> {
		const current = this.requireCurrent();
		return {
			dataDir: this.bench.dataDir,
			canonicalId: current.canonicalId,
			workingId: current.workingId,
		};
	}

	private async createCanonical(show: Show): Promise<{ id: string }> {
		const name = `${show}-canonical-${crypto.randomUUID()}`;
		if (show === Show.Empty)
			return this.api.createShow({ name, overwrite: false });
		if (show === Show.TwelveDimmers) {
			return this.api.createShow({
				name,
				data_base64: (await this.download(this.initialShow.id)).toString(
					"base64",
				),
				overwrite: false,
			});
		}
		const bytes = await fs.readFile(
			fixtureUrl(show === Show.CompactRig ? "compact-rig" : "default-stage"),
		);
		return this.api.createShow({
			name,
			data_base64: bytes.toString("base64"),
			overwrite: false,
		});
	}

	private async validatePrerequisites(
		requirements: ShowPrerequisites,
	): Promise<void> {
		const current = this.requireCurrent();
		const fixtures = await this.api.showObjects<Record<string, unknown>>(
			current.workingId,
			"patched_fixture",
		);
		const groups = await this.api.showObjects(current.workingId, "group");
		const fixtureNumbers = new Set(
			fixtures.map((fixture) => Number(fixture.body.fixture_number)),
		);
		const groupIds = new Set(groups.map((group) => group.id));
		const profileNames = new Set(
			fixtures.flatMap((fixture) => {
				const definition = fixture.body.definition as
					| { name?: string; model?: string }
					| undefined;
				return [definition?.name, definition?.model].filter(
					(value): value is string => Boolean(value),
				);
			}),
		);
		const missingFixtures = (requirements.fixtureNumbers ?? []).filter(
			(number) => !fixtureNumbers.has(number),
		);
		const missingGroups = (requirements.groups ?? []).filter(
			(id) => !groupIds.has(id),
		);
		const missingProfiles = (requirements.profiles ?? []).filter(
			(name) => !profileNames.has(name),
		);
		const missingDesktops = requirements.desktops ?? [];
		const failures = [
			missingFixtures.length
				? `fixture numbers ${missingFixtures.join(", ")}`
				: "",
			missingGroups.length ? `Groups ${missingGroups.join(", ")}` : "",
			missingProfiles.length ? `profiles ${missingProfiles.join(", ")}` : "",
			missingDesktops.length
				? `Desktops ${missingDesktops.join(", ")} (Desktops are desk data; register them with desktop.use)`
				: "",
		].filter(Boolean);
		if (failures.length) {
			throw new Error(
				`Show catalog entry "${current.catalogName}" is stale: missing ${failures.join("; ")}. Available entries: ${[...Object.values(Show), ...recipes.keys()].join(", ")}`,
			);
		}
	}

	private async routeOutputs(showId: string): Promise<void> {
		for (const route of await this.api.showObjects<Record<string, unknown>>(
			showId,
			"route",
		)) {
			const protocol = route.body.protocol;
			if (protocol !== "art_net" && protocol !== "sacn") continue;
			await this.api.seedShowObject(
				showId,
				"route",
				route.id,
				{
					...route.body,
					destination: `127.0.0.1:${protocol === "art_net" ? this.bench.artnet.port : this.bench.sacn.port}`,
				},
				route.revision,
			);
		}
	}

	private async resetBenchState(): Promise<void> {
		await this.api.request(
			"POST",
			"/api/v2/test/clock/reset",
			undefined,
			false,
		);
		this.bench.artnet.reset();
		this.bench.sacn.reset();
		await this.api.login();
	}

	private async download(id: string): Promise<Buffer> {
		const response = await fetch(
			`${this.api.baseUrl}/api/v2/shows/${id}/download`,
			{
				headers: { authorization: `Bearer ${this.api.session?.token}` },
			},
		);
		if (!response.ok)
			throw new Error(`Could not copy catalog show ${id}: ${response.status}`);
		return Buffer.from(await response.arrayBuffer());
	}

	private requireCurrent(): WorkingShow {
		if (!this.current)
			throw new Error("No isolated show is active; call show.use(...) first");
		return this.current;
	}
}

function resolveShow(show: Show | DefinedShow): RegisteredShow {
	if (typeof show === "string") return { base: show, requires: {} };
	const recipe = recipes.get(show.name);
	if (!recipe)
		throw new Error(
			`Unknown show recipe "${show.name}". Available entries: ${[...Object.values(Show), ...recipes.keys()].join(", ")}`,
		);
	return recipe;
}

function showName(show: Show | DefinedShow): string {
	return typeof show === "string" ? show : show.name;
}

function mergeRequirements(
	base: ShowPrerequisites,
	extra: ShowPrerequisites,
): ShowPrerequisites {
	return {
		fixtureNumbers: [
			...new Set([
				...(base.fixtureNumbers ?? []),
				...(extra.fixtureNumbers ?? []),
			]),
		],
		profiles: [
			...new Set([...(base.profiles ?? []), ...(extra.profiles ?? [])]),
		],
		groups: [...new Set([...(base.groups ?? []), ...(extra.groups ?? [])])],
		desktops: [
			...new Set([...(base.desktops ?? []), ...(extra.desktops ?? [])]),
		],
	};
}
