import fs from "node:fs/promises";
import { expect, type Page } from "@playwright/test";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { LightBench } from "../core/lightBench";
import type { ActiveShow } from "./showIdentity";

export enum RestartMode {
	Graceful = "graceful",
	Abrupt = "abrupt",
}

interface BootstrapSnapshot {
	active_show: ActiveShow | null;
	active_show_error: string | null;
}

interface ReadinessSnapshot {
	status: string;
	recovery_mode: boolean;
	active_show_error: string | null;
}

interface ShowLibraryEntry {
	id: string;
	name: string;
	path: string;
}

interface DamagedEvidence {
	path: string;
	bytes: Buffer;
}

export class ShowRecoveryAdapter {
	private damaged?: DamagedEvidence;

	constructor(
		private readonly api: ApiDriver,
		private readonly bench: LightBench,
		private readonly desk: DeskDriver,
		private readonly page?: Page,
	) {}

	async restart(mode: RestartMode): Promise<void> {
		const active = await this.active();
		await this.desk.recordStep(
			"SHOW RESTART",
			`${mode === RestartMode.Graceful ? "Gracefully restart" : "Abruptly stop and restart"} the isolated test server.`,
		);
		if (mode === RestartMode.Graceful) {
			await this.bench.stopServerGracefully(this.sessionToken());
			await this.bench.startServer();
		} else {
			await this.bench.restart();
		}
		await this.api.login();
		if (this.page) await this.desk.open(this.bench.baseUrl);
		await expect.poll(async () => (await this.active()).id).toBe(active.id);
	}

	async prepareMalformedActiveShow(): Promise<void> {
		const active = await this.active();
		const entry = (await this.api.shows<ShowLibraryEntry>()).find(
			(show) => show.id === active.id,
		);
		if (!entry?.path.startsWith(this.bench.dataDir)) {
			throw new Error(
				"The active show is not owned by this isolated test bench",
			);
		}
		await this.desk.recordStep(
			"RECOVERY FIXTURE SETUP",
			"Stage a malformed active show inside the test-owned data directory, then start the real server.",
		);
		await this.bench.stopServerGracefully(this.sessionToken());
		const bytes = Buffer.from("not a ToskLight SQLite show\n");
		await fs.writeFile(entry.path, bytes);
		this.damaged = { path: entry.path, bytes };
		await this.bench.startServer();
		await this.api.login();
		if (this.page) await this.desk.open(this.bench.baseUrl);
	}

	async expectRecoveryRequired(): Promise<void> {
		const readiness = await this.readiness();
		expect(readiness).toMatchObject({ status: "ready", recovery_mode: true });
		expect(readiness.active_show_error).toBeTruthy();
		expect((await this.bootstrap()).active_show_error).toBeTruthy();
		const recovery = this.browser().getByRole("alertdialog", {
			name: "Show recovery required",
		});
		await expect(recovery).toBeVisible();
		await expect(recovery).toContainText("has not been changed or deleted");
		await expect(
			recovery.getByRole("button", {
				name: "Load Clean Built-in Default",
				exact: true,
			}),
		).toBeVisible();
	}

	async expectRecovered(): Promise<void> {
		const readiness = await this.readiness();
		expect(readiness).toMatchObject({
			status: "ready",
			recovery_mode: false,
			active_show_error: null,
		});
		expect((await this.bootstrap()).active_show_error).toBeNull();
		if (this.page) {
			await expect(
				this.page.getByRole("alertdialog", {
					name: "Show recovery required",
				}),
			).toBeHidden();
		}
		if (this.damaged) {
			expect(await fs.readFile(this.damaged.path)).toEqual(this.damaged.bytes);
		}
	}

	private async active(): Promise<ActiveShow> {
		const active = (await this.bootstrap()).active_show;
		if (!active) throw new Error("No show is active");
		return active;
	}

	private bootstrap(): Promise<BootstrapSnapshot> {
		return this.api.request("GET", "/api/v2/bootstrap", undefined, false);
	}

	private async readiness(): Promise<ReadinessSnapshot> {
		const response = await fetch(`${this.api.baseUrl}/api/v2/readiness`);
		if (!response.ok)
			throw new Error(`Readiness failed with ${response.status}`);
		return response.json() as Promise<ReadinessSnapshot>;
	}

	private browser(): Page {
		if (!this.page)
			throw new Error("Show recovery assertion has no browser page");
		return this.page;
	}

	private sessionToken(): string {
		const token = this.api.session?.token;
		if (!token)
			throw new Error("Show restart requires an authenticated session");
		return token;
	}
}
