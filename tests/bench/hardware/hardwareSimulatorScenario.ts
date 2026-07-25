import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import net from "node:net";
import { expect, type Locator, type Page } from "@playwright/test";
import { ControllableHardwareOscDriver } from "./hardwareControls";

type Box = NonNullable<Awaited<ReturnType<Locator["boundingBox"]>>>;

/** Semantic operations performed against the production hardware-controls app. */
export class BrowserHardwareSimulator {
	constructor(private readonly page: Page) {}

	async expectGeometryAndIndependentFaders(): Promise<void> {
		await this.withSimulator({ width: 1600, height: 1100 }, async (hardware) => {
			await expect(this.page.locator(".hardware-number-block")).toBeVisible();
			await expect(
				this.page.locator(
					".hardware-highlight-feedback,.highlight-hardware,[aria-label='Highlight status']",
				),
			).toHaveCount(0);

			const record = await this.requiredBox(
				this.page.locator('[data-keypad-key="RECORD"]'),
			);
			const preload = await this.requiredBox(
				this.page.locator('[data-keypad-key="PRELOAD GO"]'),
			);
			expect(Math.abs(record.y - preload.y)).toBeLessThanOrEqual(1.5);
			expect(Math.abs(record.width - preload.width)).toBeLessThanOrEqual(1.5);
			expect(Math.abs(record.height - preload.height)).toBeLessThanOrEqual(1.5);
			expect(record.x + record.width).toBeLessThanOrEqual(preload.x);

			const upper = await Promise.all(
				["HIGH", "PREV", "NEXT", "ALL"].map((key) =>
					this.requiredBox(
						this.page.locator(`[data-keypad-key="${key}"]`),
					),
				),
			);
			const lower = await Promise.all(
				["GRP", "CUE", "TIME", "DIV"].map((key) =>
					this.requiredBox(
						this.page.locator(`[data-keypad-key="${key}"]`),
					),
				),
			);
			for (let index = 0; index < upper.length; index += 1) {
				expect(
					Math.abs(this.centerX(upper[index]) - this.centerX(lower[index])),
				).toBeLessThanOrEqual(1.5);
				expect(Math.abs(upper[index].width - lower[index].width)).toBeLessThanOrEqual(
					1.5,
				);
				expect(
					Math.abs(upper[index].height - lower[index].height),
				).toBeLessThanOrEqual(1.5);
			}

			const programmerFade = this.page
				.locator(".fade-times .time-fader")
				.filter({ hasText: "Prog Fade" });
			const cueFade = this.page
				.locator(".fade-times .time-fader")
				.filter({ hasText: "Cue Fade" });
			const programmerBox = await this.requiredBox(programmerFade);
			const cueBox = await this.requiredBox(cueFade);
			expect(Math.abs(programmerBox.y - cueBox.y)).toBeLessThanOrEqual(1.5);
			expect(
				Math.abs(programmerBox.width - cueBox.width),
			).toBeLessThanOrEqual(1.5);
			expect(
				Math.abs(programmerBox.height - cueBox.height),
			).toBeLessThanOrEqual(1.5);
			const fadeArea = await this.requiredBox(this.page.locator(".fade-times"));
			expect(programmerBox.y).toBeGreaterThanOrEqual(fadeArea.y);
			expect(cueBox.y + cueBox.height).toBeLessThanOrEqual(
				fadeArea.y + fadeArea.height + 1.5,
			);

			hardware.clear();
			await this.setRange(programmerFade.locator('input[type="range"]'), 0.7);
			await expect
				.poll(() => hardware.values("programmer/prog-fade"))
				.toEqual([0.7]);
			expect(hardware.values("programmer/cue-fade")).toEqual([]);

			hardware.clear();
			await this.setRange(cueFade.locator('input[type="range"]'), 0.35);
			await expect
				.poll(() => hardware.values("programmer/cue-fade"))
				.toEqual([0.35]);
			expect(hardware.values("programmer/prog-fade")).toEqual([]);

			hardware.clear();
			await this.setRange(programmerFade.locator('input[type="range"]'), 0);
			await this.setRange(programmerFade.locator('input[type="range"]'), 1);
			await this.setRange(cueFade.locator('input[type="range"]'), 0);
			await this.setRange(cueFade.locator('input[type="range"]'), 1);
			await expect
				.poll(() => hardware.values("programmer/prog-fade"))
				.toEqual([0, 1]);
			await expect
				.poll(() => hardware.values("programmer/cue-fade"))
				.toEqual([0, 1]);
		});
	}

	async expectEncoderAndNavigationTokens(): Promise<void> {
		await this.withSimulator({ width: 2400, height: 1200 }, async (hardware) => {
			await this.page.getByRole("button", { name: "Encoder 2 up" }).click();
			await this.page.getByRole("button", { name: "Encoder 2 hold" }).click();
			await this.page.getByRole("button", { name: "Encoder 2 right" }).click();
			await this.page.getByRole("button", { name: "Encoder 2 click" }).click();
			await this.page.getByRole("button", { name: "Navigation down" }).click();
			await this.page.getByRole("button", { name: "Navigation hold" }).click();
			await this.page.getByRole("button", { name: "Navigation left" }).click();
			await this.page.getByRole("button", { name: "Navigation click" }).click();

			await expect
				.poll(() => hardware.values("encode/2"))
				.toEqual(["up", "right", "press"]);
			await expect
				.poll(() => hardware.values("nav"))
				.toEqual(["down", "left", "press"]);
		});
	}

	async expectShiftRecordPointerSequences(): Promise<void> {
		await this.withSimulator({ width: 1600, height: 1100 }, async (hardware) => {
			const shift = this.page.locator('[data-keypad-key="SHIFT"]');
			const record = this.page.locator('[data-keypad-key="RECORD"]');

			hardware.clear();
			await this.pointerDown(shift, 1);
			await this.pointerPress(record, 2);
			await this.pointerUp(shift, 1);
			await expect.poll(() => hardware.programmerButtonWrites()).toEqual([
				["programmer/shift", true],
				["programmer/record", true],
				["programmer/record", false],
				["programmer/shift", false],
			]);

			hardware.clear();
			await this.pointerDown(shift, 1);
			await this.pointerPress(record, 2);
			await this.pointerPress(record, 2);
			await this.pointerUp(shift, 1);
			await expect.poll(() => hardware.programmerButtonWrites()).toEqual([
				["programmer/shift", true],
				["programmer/record", true],
				["programmer/record", false],
				["programmer/record", true],
				["programmer/record", false],
				["programmer/shift", false],
			]);

			hardware.clear();
			await this.pointerDown(shift, 1);
			await this.pointerDown(record, 2);
			await expect(record.getByText("LONG", { exact: true })).toBeVisible({
				timeout: 1_000,
			});
			await this.pointerUp(record, 2);
			await this.pointerUp(shift, 1);
			await expect.poll(() => hardware.programmerButtonWrites()).toEqual([
				["programmer/shift", true],
				["programmer/record", true],
				["programmer/record", false],
				["programmer/shift", false],
			]);
		});
	}

	private async withSimulator(
		viewport: { width: number; height: number },
		callback: (hardware: ControllableHardwareOscDriver) => Promise<void>,
	): Promise<void> {
		const port = await this.freePort();
		const url = `http://127.0.0.1:${port}`;
		const server = spawn(
			"npm",
			["run", "dev", "--", "--port", String(port), "--strictPort"],
			{
				cwd: new URL("../../../apps/light-hardware-controls", import.meta.url).pathname,
				env: { ...process.env, NO_COLOR: "1", FORCE_COLOR: "0" },
				stdio: "pipe",
			},
		);
		try {
			await this.waitForServer(url, server);
			await this.page.setViewportSize(viewport);
			const hardware = new ControllableHardwareOscDriver(this.page);
			await hardware.install();
			await this.page.goto(url);
			await callback(hardware);
		} finally {
			await this.stopServer(server);
		}
	}

	private async freePort(): Promise<number> {
		return new Promise((resolve, reject) => {
			const server = net.createServer();
			server.once("error", reject);
			server.listen(0, "127.0.0.1", () => {
				const address = server.address();
				const port = typeof address === "object" && address ? address.port : 0;
				server.close((error) => (error ? reject(error) : resolve(port)));
			});
		});
	}

	private async waitForServer(
		url: string,
		process: ChildProcessWithoutNullStreams,
	): Promise<void> {
		let output = "";
		process.stdout.on("data", (chunk) => {
			output += String(chunk);
		});
		process.stderr.on("data", (chunk) => {
			output += String(chunk);
		});
		for (let attempt = 0; attempt < 100; attempt += 1) {
			if (process.exitCode !== null)
				throw new Error(
					`Hardware-controls Vite server exited early.\n${output}`,
				);
			try {
				const response = await fetch(url);
				if (response.ok) return;
			} catch {
				// The development server has not opened its socket yet.
			}
			await new Promise((resolve) => setTimeout(resolve, 50));
		}
		throw new Error(
			`Hardware-controls Vite server did not become ready.\n${output}`,
		);
	}

	private async stopServer(server: ChildProcessWithoutNullStreams): Promise<void> {
		if (server.exitCode !== null) return;
		server.kill("SIGTERM");
		await new Promise<void>((resolve) => {
			const timer = setTimeout(resolve, 2_000);
			server.once("exit", () => {
				clearTimeout(timer);
				resolve();
			});
		});
	}

	private async requiredBox(locator: Locator): Promise<Box> {
		await expect(locator).toBeVisible();
		const box = await locator.boundingBox();
		expect(box).toBeTruthy();
		return box as Box;
	}

	private centerX(box: Pick<Box, "x" | "width">): number {
		return box.x + box.width / 2;
	}

	private async setRange(locator: Locator, value: number): Promise<void> {
		await locator.fill(String(value));
	}

	private async pointerDown(locator: Locator, pointerId: number): Promise<void> {
		await this.requiredBox(locator);
		await locator.dispatchEvent("pointerdown", {
			pointerId,
			pointerType: "touch",
			isPrimary: pointerId === 1,
			buttons: 1,
		});
	}

	private async pointerUp(locator: Locator, pointerId: number): Promise<void> {
		await this.requiredBox(locator);
		await locator.dispatchEvent("pointerup", {
			pointerId,
			pointerType: "touch",
			isPrimary: pointerId === 1,
			buttons: 0,
		});
	}

	private async pointerPress(locator: Locator, pointerId: number): Promise<void> {
		await this.pointerDown(locator, pointerId);
		await this.pointerUp(locator, pointerId);
	}
}
