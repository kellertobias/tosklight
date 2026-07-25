import fs from "node:fs/promises";
import { expect, type Page } from "@playwright/test";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { LightBench, TestShow } from "../core/lightBench";
import {
	availableShowNames,
	type DefinedShow,
	initialShowCatalog,
	isDefinedShow,
	mergeShowRequirements,
	resolveShow,
	Show,
	type ShowPrerequisites,
	showName,
} from "./showCatalog";
import {
	type ResolvedShowTarget,
	resolveShowHandle,
	type ShowHandle,
} from "./showIdentity";
import {
	ShowOperatorAdapter,
	type ShowRevisionExpectation,
} from "./showOperatorScenario";
import { RestartMode, ShowRecoveryAdapter } from "./showRecoveryScenario";

export { defineShow, Show } from "./showCatalog";
export type { ShowHandle } from "./showIdentity";
export { RestartMode };

interface WorkingShow {
	catalogName: string;
	canonicalId: string;
	workingId: string;
	workingName: string;
	canonicalBytes: Buffer;
}

export type ShowTarget = Show | DefinedShow | ShowHandle | string;

class ShowActionSurface {
	constructor(
		private readonly owner: BrowserShows,
		private readonly adapter: ShowOperatorAdapter,
	) {}

	async create(name: string): Promise<ShowHandle> {
		return this.owner.adopt(await this.adapter.create(name));
	}

	async load(target: ShowTarget): Promise<ShowHandle> {
		return this.owner.adopt(
			await this.adapter.load(this.owner.resolveTarget(target)),
		);
	}

	save(): Promise<ShowHandle> {
		return this.adapter.save();
	}

	async saveAs(name: string): Promise<ShowHandle> {
		return this.owner.adopt(await this.adapter.saveAs(name));
	}

	saveRevision(name: string): Promise<number> {
		return this.adapter.saveRevision(name);
	}

	async loadRevision(target: ShowTarget, revision: number): Promise<ShowHandle> {
		return this.owner.adopt(
			await this.adapter.loadRevision(
				this.owner.resolveTarget(target),
				revision,
			),
		);
	}

	async loadCleanDefault(): Promise<ShowHandle> {
		return this.owner.adopt(await this.adapter.loadCleanDefault());
	}
}

const fixtureUrl = (name: "compact-rig" | "default-stage") =>
	new URL(`../../../../../tests/fixtures/${name}.show`, import.meta.url);

export class BrowserShows {
	private current?: WorkingShow;
	private readonly ui: ShowOperatorAdapter;
	private readonly apiRoute: ShowOperatorAdapter;
	private readonly recoveryDriver: ShowRecoveryAdapter;
	readonly via: { ui: ShowActionSurface; api: ShowActionSurface };
	readonly recovery: { prepareMalformedActive: () => Promise<void> };

	constructor(
		private readonly api: ApiDriver,
		private readonly bench: LightBench,
		private readonly desk: DeskDriver,
		private readonly initialShow: TestShow,
		page?: Page,
	) {
		this.ui = new ShowOperatorAdapter("ui", api, bench, desk, page);
		this.apiRoute = new ShowOperatorAdapter("api", api, bench, desk, page);
		this.recoveryDriver = new ShowRecoveryAdapter(api, bench, desk, page);
		this.via = {
			ui: new ShowActionSurface(this, this.ui),
			api: new ShowActionSurface(this, this.apiRoute),
		};
		this.recovery = {
			prepareMalformedActive: () =>
				this.recoveryDriver.prepareMalformedActiveShow(),
		};
	}

	readonly expect = {
		active: async (show: ShowTarget) => {
			const target = this.resolveTarget(show);
			const active = await this.ui.active();
			if (target.id) expect(active.id).toBe(target.id);
			else expect(active.name).toBe(target.name);
			if (
				typeof show === "string" &&
				Object.values(Show).includes(show as Show)
			) {
				const current = this.requireCurrent();
				expect(current.workingId).not.toBe(current.canonicalId);
			}
		},
		revision: (expectation: ShowRevisionExpectation) =>
			this.ui.expectRevision(expectation),
		dirty: (expected: boolean) => this.ui.expectAutosaved(expected),
		recoveryRequired: () => this.recoveryDriver.expectRecoveryRequired(),
		recovered: () => this.recoveryDriver.expectRecovered(),
	};

	create(name: string): Promise<ShowHandle> {
		return this.via.ui.create(name);
	}

	load(target: ShowTarget): Promise<ShowHandle> {
		return this.via.ui.load(target);
	}

	save(): Promise<ShowHandle> {
		return this.via.ui.save();
	}

	saveAs(name: string): Promise<ShowHandle> {
		return this.via.ui.saveAs(name);
	}

	saveRevision(name: string): Promise<number> {
		return this.via.ui.saveRevision(name);
	}

	loadRevision(target: ShowTarget, revision: number): Promise<ShowHandle> {
		return this.via.ui.loadRevision(target, revision);
	}

	loadCleanDefault(): Promise<ShowHandle> {
		return this.via.ui.loadCleanDefault();
	}

	restart(mode: RestartMode): Promise<void> {
		return this.recoveryDriver.restart(mode);
	}

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
			mergeShowRequirements(
				initialShowCatalog[resolved.base],
				resolved.requires,
			),
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

	adopt(handle: ShowHandle): ShowHandle {
		if (!this.current) return handle;
		const target = resolveShowHandle(handle);
		if (!target.id)
			throw new Error(`Show action did not return an id for "${target.name}"`);
		this.current = {
			...this.current,
			workingId: target.id,
			workingName: target.name,
		};
		return handle;
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

	resolveTarget(show: ShowTarget): ResolvedShowTarget {
		if (typeof show === "object" && "name" in show) {
			if (isDefinedShow(show.name)) {
				const current = this.requireCurrent();
				if (current.catalogName !== show.name) {
					throw new Error(
						`Catalog recipe "${show.name}" has not established this scenario's fixture`,
					);
				}
				return { id: current.workingId, name: current.workingName };
			}
			return resolveShowHandle(show);
		}
		if (Object.values(Show).includes(show as Show)) {
			const current = this.requireCurrent();
			if (current.catalogName !== show) {
				throw new Error(
					`Catalog show "${show}" has not established this scenario's fixture`,
				);
			}
			return { id: current.workingId, name: current.workingName };
		}
		return { name: show };
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
				`Show catalog entry "${current.catalogName}" is stale: missing ${failures.join("; ")}. Available entries: ${availableShowNames().join(", ")}`,
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
		return this.api.downloadShow(id);
	}

	private requireCurrent(): WorkingShow {
		if (!this.current)
			throw new Error("No isolated show is active; call show.use(...) first");
		return this.current;
	}
}
