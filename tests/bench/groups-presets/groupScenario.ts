import { expect, type Locator, type Page } from "@playwright/test";
import { HttpGroupManagementTransport } from "../../../apps/light-desktop/src/api/GroupManagementTransport";
import { HttpGroupRecordingTransport } from "../../../apps/light-desktop/src/api/GroupRecordingTransport";
import type { StoredGroup } from "../../../apps/light-desktop/src/api/types";
import type { ApiDriver } from "../core/api";
import type { BrowserCommands } from "../command-selection/commandScenario";
import type { DeskDriver } from "../core/desk";
import type { SimulatedHardware } from "../hardware/hardwareScenario";
import type { BrowserSelection } from "../command-selection/selectionScenario";

export enum StoreMode {
	Overwrite = "overwrite",
	Merge = "merge",
	Subtract = "subtract",
}

export interface GroupProperties {
	name: string;
	color?: string | null;
	icon?: string | null;
}

type GroupRoute = "pool" | "keypad" | "api" | "osc";
type GroupAction = "store" | "select" | "edit" | "delete";

export interface GroupRouteReport {
	seed: string;
	actionIndex: number;
	action: GroupAction;
	candidates: readonly GroupRoute[];
	selected: GroupRoute;
}

class GroupActionSurface {
	constructor(
		private readonly owner: BrowserGroups,
		private readonly route: GroupRoute,
	) {}

	store(number: number, options: { mode: StoreMode }) {
		return this.owner.storeVia(this.route, number, options.mode);
	}

	select(number: number) {
		return this.owner.selectVia(this.route, number);
	}

	recall(number: number) {
		return this.select(number);
	}

	edit(number: number, properties: GroupProperties) {
		return this.owner.editVia(this.route, number, properties);
	}

	delete(number: number) {
		return this.owner.deleteVia(this.route, number);
	}
}

class GroupExpectation {
	constructor(
		private readonly owner: BrowserGroups,
		private readonly number: number,
	) {}

	async present() {
		expect(await this.owner.object(this.number)).not.toBeNull();
	}

	async absent() {
		expect(await this.owner.object(this.number)).toBeNull();
	}

	async empty() {
		await this.fixtures();
	}

	async fixtures(...numbers: number[]) {
		const group = await this.owner.requiredObject(this.number);
		const byId = new Map(
			(await this.owner.patchFixtures()).map((fixture) => [
				fixture.fixture_id,
				fixture.fixture_number,
			]),
		);
		expect(group.body.fixtures.map((id) => byId.get(id))).toEqual(numbers);
	}

	async metadata(properties: Partial<GroupProperties>) {
		const group = await this.owner.requiredObject(this.number);
		expect(group.body).toMatchObject(properties);
	}
}

export class BrowserGroups {
	readonly routeReports: GroupRouteReport[] = [];
	readonly via = {
		pool: new GroupActionSurface(this, "pool"),
		keypad: new GroupActionSurface(this, "keypad"),
		api: new GroupActionSurface(this, "api"),
		osc: new GroupActionSurface(this, "osc"),
	};

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly commands: BrowserCommands,
		private readonly selection: BrowserSelection,
		private readonly hardware: SimulatedHardware,
		private readonly activeShowId: () => string,
		private readonly seed: string,
	) {}
	private actionIndex = 0;

	store(number: number, options: { mode: StoreMode }) {
		const candidates: GroupRoute[] =
			options.mode === StoreMode.Subtract
				? this.withOsc(["api", "keypad"])
				: this.withOsc(["api", "keypad", "pool"]);
		return this.unqualified("store", candidates, (route) =>
			this.storeVia(route, number, options.mode),
		);
	}

	select(number: number) {
		return this.unqualified(
			"select",
			this.withOsc(["api", "keypad", "pool"]),
			(route) => this.selectVia(route, number),
		);
	}

	recall(number: number) {
		return this.select(number);
	}

	edit(number: number, properties: GroupProperties) {
		return this.unqualified("edit", ["api", "pool"], (route) =>
			this.editVia(route, number, properties),
		);
	}

	delete(number: number) {
		return this.unqualified(
			"delete",
			this.withOsc(["api", "keypad"]),
			(route) => this.deleteVia(route, number),
		);
	}

	expect(number: number) {
		return new GroupExpectation(this, validNumber(number));
	}

	async storeVia(route: GroupRoute, number: number, mode: StoreMode) {
		number = validNumber(number);
		const before = await this.object(number);
		if (route === "api") {
			const session = this.session();
			await new HttpGroupRecordingTransport({
				baseUrl: this.api.baseUrl,
				sessionToken: session.token,
			}).record(this.showId(), {
				requestId: crypto.randomUUID(),
				groupId: String(number),
				operation: mode,
				expectedObjectRevision: before?.revision ?? 0,
			});
		} else if (route === "pool") {
			if (mode === StoreMode.Subtract)
				throw new Error("Group pool does not expose Subtract; use keypad, API, or OSC");
			await this.desk.click(this.page.locator(".global-store-button:visible").first());
			await this.desk.click(this.groupCard(number));
			const choice = this.page.getByRole("button", {
				name: mode === StoreMode.Merge ? "Merge" : "Overwrite",
				exact: true,
			});
			if (await choice.count()) await this.desk.click(choice);
		} else if (route === "keypad") {
			await this.commands.via.ui.execute(groupRecordCommand(number, mode));
		} else {
			await this.sendOsc(groupRecordKeys(number, mode));
		}
		await this.waitForRevision(number, before?.revision ?? 0);
	}

	async selectVia(route: GroupRoute, number: number) {
		number = validNumber(number);
		if (route === "api") await this.selection.groups.item(number);
		else if (route === "pool") await this.desk.click(this.groupCard(number));
		else if (route === "keypad")
			await this.commands.via.ui.execute(`GROUP ${number}`);
		else await this.sendOsc(["group", ...digits(number), "enter"]);
		await this.selection.expectSelection({ kind: "group", number });
	}

	async editVia(route: GroupRoute, number: number, properties: GroupProperties) {
		number = validNumber(number);
		const group = await this.requiredObject(number);
		if (route === "api") {
			const session = this.session();
			await new HttpGroupManagementTransport({
				baseUrl: this.api.baseUrl,
				sessionToken: session.token,
			}).manage(this.showId(), {
				requestId: crypto.randomUUID(),
				groupId: String(number),
				expectedObjectRevision: group.revision,
				operation: {
					type: "update_properties",
					properties: {
						name: properties.name,
						color: properties.color ?? null,
						icon: properties.icon ?? null,
					},
				},
			});
		} else if (route === "pool") {
			await this.commands.via.ui.type("SET");
			await this.desk.click(this.groupCard(number));
			const dialog = this.page.getByRole("dialog", { name: "Group properties" });
			await dialog.getByLabel("Group name").fill(properties.name);
			if (properties.icon)
				await choosePicker(dialog, this.page, "Icon", `Use ${properties.icon}`);
			if (properties.color)
				await choosePicker(
					dialog,
					this.page,
					"Color",
					`Use color ${properties.color.toLowerCase()}`,
				);
			await this.desk.click(dialog.getByRole("button", { name: "Save group" }));
		} else {
			throw new Error(`Group edit has no truthful ${route} route`);
		}
		await expect.poll(async () => (await this.requiredObject(number)).revision).toBeGreaterThan(group.revision);
	}

	async deleteVia(route: GroupRoute, number: number) {
		number = validNumber(number);
		const before = await this.requiredObject(number);
		if (route === "api") {
			const session = this.session();
			await new HttpGroupRecordingTransport({
				baseUrl: this.api.baseUrl,
				sessionToken: session.token,
			}).record(this.showId(), {
				requestId: crypto.randomUUID(),
				groupId: String(number),
				operation: "delete",
				expectedObjectRevision: before.revision,
			});
		} else if (route === "keypad")
			await this.commands.via.ui.execute(`DELETE GROUP ${number}`);
		else if (route === "osc")
			await this.sendOsc(["delete", "group", ...digits(number), "enter"]);
		else throw new Error("Group pool has no visible delete action");
		await expect.poll(async () => this.object(number)).toBeNull();
	}

	async object(number: number) {
		return this.api.showObject<StoredGroup>(this.showId(), "group", String(number));
	}

	async requiredObject(number: number) {
		const group = await this.object(number);
		if (!group) throw new Error(`Group ${number} is absent`);
		return group;
	}

	patchFixtures() {
		return this.api.patch().then((patch) => patch.fixtures);
	}

	private async unqualified(
		action: GroupAction,
		candidates: readonly GroupRoute[],
		execute: (route: GroupRoute) => Promise<unknown>,
	) {
		const actionIndex = this.actionIndex++;
		const selected =
			candidates[stableIndex(`${this.seed}:${actionIndex}`, candidates.length)];
		this.routeReports.push({
			seed: this.seed,
			actionIndex,
			action,
			candidates,
			selected,
		});
		await execute(selected);
	}

	private withOsc(routes: GroupRoute[]): GroupRoute[] {
		return this.hardware.connected ? [...routes, "osc"] : routes;
	}

	private groupCard(number: number) {
		return this.page
			.locator('[data-pane-type="groups"] .group-card:visible')
			.nth(number - 1);
	}

	private async waitForRevision(number: number, previous: number) {
		await expect.poll(async () => (await this.object(number))?.revision ?? 0).toBeGreaterThan(previous);
	}

	private async sendOsc(keys: string[]) {
		if (!this.hardware.connected)
			throw new Error("Group OSC route requires hardware.connect()");
		const alias = this.session().desk.osc_alias;
		for (const key of keys) {
			const mark = this.hardware.mark();
			await this.hardware.send(`/light/${alias}/programmer/${key}`, [true]);
			await this.hardware.expectAfter(
				mark,
				`/light/${alias}/feedback/command-line`,
			);
			await this.hardware.send(`/light/${alias}/programmer/${key}`, [false]);
		}
	}

	private showId() {
		return this.activeShowId();
	}

	private session() {
		if (!this.api.session) throw new Error("Group helper requires an API session");
		return this.api.session;
	}
}

function validNumber(number: number) {
	if (!Number.isSafeInteger(number) || number < 1)
		throw new Error("Group numbers start at 1");
	return number;
}

function groupRecordCommand(number: number, mode: StoreMode) {
	return `RECORD ${mode === StoreMode.Merge ? "+ " : mode === StoreMode.Subtract ? "- " : ""}GROUP ${number}`;
}

function groupRecordKeys(number: number, mode: StoreMode) {
	return [
		"record",
		...(mode === StoreMode.Merge
			? ["plus"]
			: mode === StoreMode.Subtract
				? ["minus"]
				: []),
		"group",
		...digits(number),
		"enter",
	];
}

function digits(number: number) {
	return String(number)
		.split("")
		.map((digit) => `digit-${digit}`);
}

function stableIndex(seed: string, modulo: number) {
	let hash = 0x811c9dc5;
	for (const byte of new TextEncoder().encode(seed)) {
		hash ^= byte;
		hash = Math.imul(hash, 0x01000193);
	}
	return (hash >>> 0) % modulo;
}

async function choosePicker(
	dialog: Locator,
	page: Page,
	label: string,
	option: string,
) {
	const trigger =
		label === "Color"
			? dialog.getByRole("button", { name: /#[0-9a-f]{6}/i })
			: dialog.getByRole("button", {
					name: new RegExp(`Choose ${label}`, "i"),
				});
	await trigger.click();
	await page.getByRole(label === "Color" ? "option" : "button", { name: option }).click();
}
