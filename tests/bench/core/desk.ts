import { expect, type Locator, type Page } from "@playwright/test";
import type { SessionResponse } from "../../../apps/light-desktop/src/api/types";
import { ControllableDesktopDriver } from "../window-system/desktopBridge";
import { BrowserSessionHandoff } from "./sessionHandoff";

type RecordingClickPace =
	| "normal"
	| "compact"
	| "steady"
	| "rapid"
	| "typing"
	| "fastTyping"
	| "selection";

function recordingClickDelay(pace: RecordingClickPace) {
	return {
		normal: 280,
		compact: 120,
		steady: 180,
		rapid: 45,
		typing: 10,
		fastTyping: 3,
		selection: 70,
	}[pace];
}

function recordingClickMinimum(pace: RecordingClickPace) {
	return pace === "fastTyping"
		? 3
		: pace === "typing"
			? 10
			: pace === "rapid"
				? 40
				: pace === "selection"
					? 60
					: 120;
}

export class DeskDriver {
	private recordingStep = {
		title: "STARTING",
		description: "Preparing the test application.",
	};
	private recordingInstalled = false;
	private recordingCatalogEnabled = false;
	private recordingNavigationHandler?: () => void;
	private readonly sessionHandoffs = new Map<Page, BrowserSessionHandoff>();
	private baseUrl = "";
	private auditRevision = 0;
	private controllableDesktop?: ControllableDesktopDriver;
	private recordingClickPace: RecordingClickPace = "normal";
	private recordingClickDelayOverride?: number;
	private productIntroVisible = false;
	private readonly semanticStepObservers = new Set<
		(step: { title: string; description: string }) => void
	>();
	private readonly titleCardObservers = new Set<(title: string) => void>();

	constructor(
		readonly page: Page,
		private readonly testTitle = "Light UI test",
		private readonly controlDeskId: string | null = null,
		private readonly externalOscSummary: () => string = () => "",
	) {}

	fork(page: Page): DeskDriver {
		return new DeskDriver(
			page,
			this.testTitle,
			this.controlDeskId,
			this.externalOscSummary,
		);
	}

	setRecordingClickPace(
		pace: RecordingClickPace,
		delayOverrideMillis?: number,
	): void {
		this.recordingClickPace = pace;
		this.recordingClickDelayOverride =
			delayOverrideMillis !== undefined && Number.isFinite(delayOverrideMillis)
				? Math.max(0, delayOverrideMillis)
				: undefined;
	}

	async dispose(): Promise<void> {
		if (this.recordingNavigationHandler)
			this.page.off("domcontentloaded", this.recordingNavigationHandler);
		this.recordingNavigationHandler = undefined;
		for (const handoff of this.sessionHandoffs.values()) handoff.dispose();
		this.sessionHandoffs.clear();
	}

	async open(
		baseUrl: string,
		options: { beforeReadinessChecks?: Promise<void> } = {},
	): Promise<void> {
		this.baseUrl = baseUrl;
		const handoff = await this.prepareSessionHandoff(this.page);
		const checkpoint = handoff.checkpoint();
		if (this.controlDeskId) {
			await this.page.addInitScript((deskId) => {
				localStorage.setItem("light.control-desk", deskId);
			}, this.controlDeskId);
		}
		await this.page.goto(baseUrl);
		await options.beforeReadinessChecks;
		await handoff.adoptCurrentDocument();
		await expect(this.page.locator(".connection-cover")).toBeHidden({
			timeout: 10_000,
		});
		await expect(this.page.locator(".connection-banner")).toBeHidden({
			timeout: 10_000,
		});
		await handoff.waitForCapture(
			checkpoint,
			(session) =>
				!this.controlDeskId || session.desk.id === this.controlDeskId,
		);
		if (process.env.LIGHT_VISUAL_RECORDING === "1") {
			const url = new URL(baseUrl);
			this.recordingCatalogEnabled =
				url.searchParams.get("demo") !== "product" &&
				!this.testTitle.includes("records the complete desk");
			await this.installRecordingEffects();
		}
		if (this.recordingCatalogEnabled) {
			await this.installRecordingOverlay();
			await this.recordStep(
				this.recordingStep.title === "STARTING"
					? "APPLICATION READY"
					: this.recordingStep.title,
				this.recordingStep.title === "STARTING"
					? "The complete production desk is connected and ready for the scenario."
					: this.recordingStep.description,
			);
		}
	}

	async enableControllableDesktop(): Promise<ControllableDesktopDriver> {
		this.controllableDesktop ??= new ControllableDesktopDriver(this.page);
		await this.controllableDesktop.install();
		return this.controllableDesktop;
	}

	async session(): Promise<SessionResponse> {
		const handoff = await this.prepareSessionHandoff(this.page);
		return handoff.currentSession() ?? handoff.waitForCapture(-1);
	}

	async openPeer(page: Page, baseUrl: string): Promise<SessionResponse> {
		const handoff = await this.prepareSessionHandoff(page);
		const checkpoint = handoff.checkpoint();
		await page.goto(baseUrl);
		await handoff.adoptCurrentDocument();
		await expect(page.locator(".connection-cover")).toBeHidden({
			timeout: 10_000,
		});
		await expect(page.locator(".connection-banner")).toBeHidden({
			timeout: 10_000,
		});
		return handoff.waitForCapture(checkpoint);
	}

	private async prepareSessionHandoff(page: Page) {
		let handoff = this.sessionHandoffs.get(page);
		if (!handoff) {
			handoff = new BrowserSessionHandoff(page);
			this.sessionHandoffs.set(page, handoff);
		}
		await handoff.install();
		return handoff;
	}

	/** Shows a full-screen chapter card over a blurred application background. */
	async titleCard(
		title: string,
		description: string,
		stayMillis = Number(process.env.LIGHT_VISUAL_TITLE_CARD_PAUSE ?? 3_200),
	): Promise<void> {
		if (this.page.isClosed()) return;
		for (const observer of this.titleCardObservers) observer(title);
		await this.updateDemoChapter(title, description);
		if (process.env.LIGHT_VISUAL_RECORDING !== "1") return;
		await this.installRecordingEffects();
		const crossfadeFromProductIntro = this.productIntroVisible;
		this.productIntroVisible = false;
		const fadeMillis = Number(process.env.LIGHT_VISUAL_TITLE_CARD_FADE ?? 900);
		await this.page.evaluate(
			async ({ title, description, crossfade, duration }) => {
				const card = document.querySelector<HTMLElement>(
					"#light-recording-title-card",
				);
				if (!card) return;
				const current = card.querySelector<HTMLElement>(":scope > div");
				if (crossfade && current) {
					const next = document.createElement("div");
					next.innerHTML = "<img hidden alt=''><strong></strong><p></p>";
					next.querySelector("strong")!.textContent = title;
					next.querySelector("p")!.textContent = description;
					current.style.gridArea = "1 / 1";
					next.style.gridArea = "1 / 1";
					next.style.opacity = "0";
					card.append(next);
					card.classList.remove("brand");
					const timing = {
						duration: Number.isFinite(duration) && duration > 0 ? duration : 0,
						easing: "ease",
						fill: "forwards" as const,
					};
					await Promise.all([
						current.animate([{ opacity: 1 }, { opacity: 0 }], timing).finished,
						next.animate([{ opacity: 0 }, { opacity: 1 }], timing).finished,
					]);
					current.remove();
					next.style.removeProperty("grid-area");
					next.style.removeProperty("opacity");
				} else if (current) {
					card.classList.remove("brand");
					current.classList.remove("brand-content");
					current.querySelector("img")?.setAttribute("hidden", "");
					current.querySelector("strong")!.textContent = title;
					current.querySelector("p")!.textContent = description;
				}
				card.setAttribute("aria-hidden", "false");
				card.classList.add("visible");
			},
			{
				title,
				description,
				crossfade: crossfadeFromProductIntro,
				duration: fadeMillis,
			},
		);
		if (Number.isFinite(stayMillis) && stayMillis > 0)
			await this.page.waitForTimeout(stayMillis);
		await this.page.evaluate(() => {
			const card = document.querySelector<HTMLElement>(
				"#light-recording-title-card",
			);
			card?.classList.remove("visible");
			card?.setAttribute("aria-hidden", "true");
		});
		if (Number.isFinite(fadeMillis) && fadeMillis > 0)
			await this.page.waitForTimeout(fadeMillis);
	}

	async productIntro(
		stayMillis = Number(process.env.LIGHT_VISUAL_TITLE_CARD_PAUSE ?? 3_200),
	): Promise<void> {
		if (this.page.isClosed() || process.env.LIGHT_VISUAL_RECORDING !== "1")
			return;
		for (const observer of this.titleCardObservers) observer("ToskLight");
		await this.installRecordingEffects();
		await this.page.evaluate(() => {
			const card = document.querySelector<HTMLElement>(
				"#light-recording-title-card",
			);
			const source = document.querySelector<HTMLImageElement>(
				"[data-demo-application-icon]",
			)?.src;
			if (!card || !source) return;
			const icon = card.querySelector("img")!;
			card.querySelector(":scope > div")?.classList.add("brand-content");
			icon.setAttribute("src", source);
			icon.removeAttribute("hidden");
			card.querySelector("strong")!.textContent = "ToskLight";
			card.querySelector("p")!.textContent = "Product Demo";
			card.classList.add("brand", "visible");
			card.setAttribute("aria-hidden", "false");
		});
		if (Number.isFinite(stayMillis) && stayMillis > 0)
			await this.page.waitForTimeout(stayMillis);
		this.productIntroVisible = true;
	}

	async prepareProductIntro(iconDataUrl: string): Promise<void> {
		await this.page.addInitScript((icon) => {
			const install = () => {
				if (document.querySelector("#light-recording-title-card")) return;
				const style = document.createElement("style");
				style.id = "light-recording-bootstrap-title-style";
				style.textContent = `#light-recording-title-card{position:fixed;z-index:2147483647;inset:0;display:grid;place-items:center;padding:8vw;color:#f7fcff;background:#061016;font-family:Inter,system-ui,sans-serif;text-align:center}#light-recording-title-card>div{max-width:1180px;padding:58px 74px;border:1px solid #4dd9e8aa;border-radius:22px;background:#08141be8;box-shadow:0 28px 90px #000b,0 0 42px #37d8e522}#light-recording-title-card img{display:block;width:220px;height:220px;margin:0 auto 34px;object-fit:contain;filter:drop-shadow(0 18px 36px #000a)}#light-recording-title-card strong{display:block;color:#79edf7;font-size:86px;line-height:1.02;letter-spacing:.015em}#light-recording-title-card p{margin:12px auto 0;color:#a9bbc5;font-size:30px}`;
				document.head.append(style);
				const card = document.createElement("aside");
				card.id = "light-recording-title-card";
				card.className = "brand visible";
				card.setAttribute("aria-hidden", "false");
				card.innerHTML = `<div class="brand-content"><img alt="" src="${icon}"><strong>ToskLight</strong><p>Product Demo</p></div>`;
				document.documentElement.append(card);
			};
			if (document.readyState === "loading")
				document.addEventListener("DOMContentLoaded", install, { once: true });
			else install();
		}, iconDataUrl);
	}

	async setDemoAction(description: string): Promise<void> {
		if (this.page.isClosed()) return;
		await this.page.evaluate((copy) => {
			const action = document.querySelector<HTMLElement>(
				"[data-demo-current-action]",
			);
			if (action) action.textContent = copy;
		}, description);
	}

	/** Previews a recorded click before dispatching it, then leaves enough time to read the result. */
	async click(
		target: Locator,
		options: {
			button?: "left" | "right";
			modifiers?: Array<"Alt" | "Control" | "Meta" | "Shift">;
		} = {},
	): Promise<void> {
		if (process.env.LIGHT_VISUAL_RECORDING !== "1") {
			await target.click({
				button: options.button ?? "left",
				modifiers: options.modifiers,
			});
			return;
		}
		await this.installRecordingEffects();
		await target.scrollIntoViewIfNeeded();
		await target.hover();
		const box = await target.boundingBox();
		if (box) {
			await target.evaluate((element) =>
				element.classList.add("light-recording-click-target"),
			);
			await this.page.evaluate(
				({ x, y }) => {
					document.querySelector(".light-recording-click-preview")?.remove();
					const preview = document.createElement("i");
					preview.className = "light-recording-click-preview";
					preview.style.left = `${x}px`;
					preview.style.top = `${y}px`;
					document
						.querySelector("#light-recording-click-layer")
						?.append(preview);
				},
				{ x: box.x + box.width / 2, y: box.y + box.height / 2 },
			);
		}
		const hardwareTyping = await target.evaluate((element) =>
			Boolean(element.closest(".demo-number-block")),
		);
		const defaultPreview = hardwareTyping
			? (this.recordingClickDelayOverride ?? 45)
			: (this.recordingClickDelayOverride ??
				recordingClickDelay(this.recordingClickPace));
		const previewMillis = Math.max(
			hardwareTyping
				? this.recordingClickDelayOverride !== undefined
					? 0
					: 40
				: recordingClickMinimum(this.recordingClickPace),
			Number(process.env.LIGHT_VISUAL_CLICK_PREVIEW ?? defaultPreview),
		);
		if (Number.isFinite(previewMillis))
			await this.page.waitForTimeout(previewMillis);
		await target.click({
			button: options.button ?? "left",
			modifiers: options.modifiers,
		});
		const defaultPause = hardwareTyping
			? (this.recordingClickDelayOverride ?? 45)
			: (this.recordingClickDelayOverride ??
				recordingClickDelay(this.recordingClickPace));
		const settleMillis = Math.max(
			hardwareTyping
				? this.recordingClickDelayOverride !== undefined
					? 0
					: 40
				: recordingClickMinimum(this.recordingClickPace),
			Number(process.env.LIGHT_VISUAL_CLICK_PAUSE ?? defaultPause),
		);
		if (Number.isFinite(settleMillis))
			await this.page.waitForTimeout(settleMillis);
		await this.page.evaluate(() => {
			document
				.querySelectorAll(".light-recording-click-target")
				.forEach((element) => {
					element.classList.remove("light-recording-click-target");
				});
		});
	}

	/** Keeps API-accelerated demo work visually honest while the operation runs. */
	async fastForward<T>(
		description: string,
		operation: () => Promise<T>,
	): Promise<T> {
		if (this.page.isClosed()) return operation();
		await this.installRecordingEffects();
		await this.page.evaluate((copy) => {
			document.querySelector("#light-recording-fast-forward")?.remove();
			const overlay = document.createElement("aside");
			overlay.id = "light-recording-fast-forward";
			overlay.setAttribute("role", "status");
			overlay.innerHTML = `<strong aria-hidden="true">⏩</strong><span>Fast forwarding repetitive setup</span>`;
			const narrative = document.querySelector<HTMLElement>(
				".product-demo-narrative",
			);
			const progress = narrative?.querySelector<HTMLElement>(
				"[data-demo-chapter-strip]",
			);
			if (narrative && progress) {
				overlay.dataset.placement = "narrative";
				narrative.insertBefore(overlay, progress);
			} else {
				document.documentElement.append(overlay);
			}
			const action = document.querySelector<HTMLElement>(
				"[data-demo-current-action]",
			);
			if (action) action.textContent = copy;
		}, description);
		await expect(
			this.page.locator("#light-recording-fast-forward"),
		).toContainText("Fast forwarding repetitive setup");
		const pauseMillis =
			process.env.LIGHT_VISUAL_RECORDING === "1"
				? Number(process.env.LIGHT_VISUAL_FAST_FORWARD_PAUSE ?? 250)
				: 0;
		try {
			if (Number.isFinite(pauseMillis) && pauseMillis > 0)
				await this.page.waitForTimeout(pauseMillis);
			return await operation();
		} finally {
			if (!this.page.isClosed()) {
				if (Number.isFinite(pauseMillis) && pauseMillis > 0)
					await this.page.waitForTimeout(pauseMillis);
				await this.page.evaluate(() =>
					document.querySelector("#light-recording-fast-forward")?.remove(),
				);
			}
		}
	}

	private async updateDemoChapter(
		title: string,
		description: string,
	): Promise<void> {
		await this.page.evaluate(
			({ title, description }) => {
				const [section, ...subsectionParts] = title.split(" · ");
				const sectionTitle = document.querySelector<HTMLElement>(
					"[data-demo-section-title]",
				);
				const subsection = document.querySelector<HTMLElement>(
					"[data-demo-subsection]",
				);
				if (sectionTitle) sectionTitle.textContent = section;
				if (subsection) subsection.textContent = subsectionParts.join(" · ");
				const chapters = [
					...document.querySelectorAll<HTMLElement>("[data-demo-chapter]"),
				];
				const activeIndex = chapters.findIndex((chapter) => {
					const key = chapter.dataset.demoChapter ?? "";
					return title === key || title.startsWith(`${key} ·`);
				});
				chapters.forEach((chapter, index) => {
					chapter.classList.toggle(
						"completed",
						activeIndex >= 0 && index < activeIndex,
					);
					chapter.classList.toggle("active", index === activeIndex);
				});
				const action = document.querySelector<HTMLElement>(
					"[data-demo-current-action]",
				);
				if (action) action.textContent = description;
			},
			{ title, description },
		);
	}

	/** Adds a human-readable chapter card to visual recordings without affecting fast test runs. */
	async recordStep(title: string, description: string): Promise<void> {
		this.recordingStep = { title, description };
		for (const observer of this.semanticStepObservers)
			observer({ title, description });
		if (
			process.env.LIGHT_VISUAL_RECORDING !== "1" ||
			!this.recordingInstalled ||
			this.page.isClosed()
		)
			return;
		if ((await this.page.locator("#light-catalog-recording").count()) === 0) {
			await this.renderRecordingOverlay();
		}
		await this.page.evaluate(
			({ title, description }) => {
				document.querySelector("#light-catalog-step")!.textContent = title;
				document.querySelector("#light-catalog-description")!.textContent =
					description;
			},
			{ title, description },
		);
		const pause = Number(process.env.LIGHT_VISUAL_STEP_PAUSE ?? 1_200);
		if (Number.isFinite(pause) && pause > 0)
			await this.page.waitForTimeout(pause);
	}

	observeSemanticSteps(
		observer: (step: { title: string; description: string }) => void,
	): () => void {
		this.semanticStepObservers.add(observer);
		return () => this.semanticStepObservers.delete(observer);
	}

	observeTitleCards(observer: (title: string) => void): () => void {
		this.titleCardObservers.add(observer);
		return () => this.titleCardObservers.delete(observer);
	}

	private async installRecordingOverlay(): Promise<void> {
		if (this.recordingInstalled) return;
		await this.page.exposeFunction("__lightVisualDeskState", () =>
			this.visualDeskState(),
		);
		await this.renderRecordingOverlay();
		this.recordingInstalled = true;
	}

	private async installRecordingEffects(): Promise<void> {
		if (!this.recordingNavigationHandler) {
			this.recordingNavigationHandler = () => {
				void this.renderRecordingEffects().catch(() => undefined);
				if (this.recordingCatalogEnabled)
					void this.renderRecordingOverlay().catch(() => undefined);
			};
			this.page.on("domcontentloaded", this.recordingNavigationHandler);
		}
		await this.renderRecordingEffects();
	}

	private async renderRecordingEffects(): Promise<void> {
		if (this.page.isClosed()) return;
		await this.page.evaluate(() => {
			const visualWindow = window as Window & {
				__lightRecordingEffectsInstalled?: boolean;
			};
			if (!document.querySelector("#light-recording-effects-style")) {
				const style = document.createElement("style");
				style.id = "light-recording-effects-style";
				style.textContent = `
          #light-recording-click-layer{position:fixed;z-index:2147483646;inset:0;pointer-events:none;overflow:hidden}
          .light-recording-click-preview{position:fixed;width:46px;height:46px;border:3px dashed #72edff;border-radius:999px;box-shadow:0 0 0 3px #071017cc,0 0 24px #36dfff;transform:translate(-50%,-50%);animation:light-recording-preview 800ms ease-in-out infinite alternate}
          .light-recording-click-preview:before,.light-recording-click-preview:after{content:"";position:absolute;left:50%;top:50%;background:#eafdff;box-shadow:0 0 8px #36dfff;transform:translate(-50%,-50%)}
          .light-recording-click-preview:before{width:18px;height:2px}.light-recording-click-preview:after{width:2px;height:18px}
          .light-recording-click-preview.committed{opacity:0;transform:translate(-50%,-50%) scale(1.8);transition:opacity 120ms ease,transform 120ms ease}
          .light-recording-click-target{outline:3px solid #72edff!important;outline-offset:3px!important;box-shadow:0 0 22px #36dfffcc!important}
          @keyframes light-recording-preview{from{filter:brightness(.8);transform:translate(-50%,-50%) scale(.92)}to{filter:brightness(1.35);transform:translate(-50%,-50%) scale(1.08)}}
          .light-recording-click-ping{position:fixed;width:24px;height:24px;border:3px solid #72edff;border-radius:999px;box-shadow:0 0 0 2px #071017cc,0 0 20px #36dfffee;transform:translate(-50%,-50%) scale(.35);animation:light-recording-ping 140ms ease-out forwards}
          .light-recording-click-ping:after{content:"";position:absolute;inset:4px;border-radius:inherit;background:#fff;box-shadow:0 0 12px #63eaff}
          @keyframes light-recording-ping{0%{opacity:1;transform:translate(-50%,-50%) scale(.35)}70%{opacity:.9}100%{opacity:0;transform:translate(-50%,-50%) scale(3.2)}}
          .light-recording-pressed{transform:translateY(3px) scale(.96)!important;border-color:#eafdff!important;background:#285463!important;box-shadow:inset 0 0 0 3px #ffffffbb,0 0 18px #42e3f0cc!important;filter:brightness(1.35)!important}
          #light-recording-title-card{position:fixed;z-index:2147483647;inset:0;display:grid;place-items:center;padding:8vw;color:#f7fcff;background:#061016a8;backdrop-filter:blur(18px) brightness(.48);-webkit-backdrop-filter:blur(18px) brightness(.48);opacity:0;visibility:hidden;transition:opacity 600ms ease,visibility 0s linear 600ms;pointer-events:none;font-family:Inter,system-ui,sans-serif;text-align:center}
          #light-recording-title-card.visible{opacity:1;visibility:visible;transition:opacity 420ms ease}
		  #light-recording-title-card.brand{background:#061016;backdrop-filter:none;-webkit-backdrop-filter:none}
          #light-recording-title-card>div{max-width:1180px;padding:58px 74px;border:1px solid #4dd9e8aa;border-radius:22px;background:#08141be8;box-shadow:0 28px 90px #000b,0 0 42px #37d8e522}
          #light-recording-title-card img{display:block;width:220px;height:220px;margin:0 auto 34px;object-fit:contain;filter:drop-shadow(0 18px 36px #000a)}
          #light-recording-title-card img[hidden]{display:none}
          #light-recording-title-card strong{display:block;color:#79edf7;font-size:clamp(48px,5vw,86px);line-height:1.02;letter-spacing:.055em;text-transform:uppercase}
          #light-recording-title-card p{max-width:980px;margin:28px auto 0;color:#e9f2f6;font-size:clamp(22px,2vw,34px);line-height:1.35}
          #light-recording-title-card .brand-content strong{text-transform:none;letter-spacing:.015em}
          #light-recording-title-card .brand-content p{margin-top:12px;color:#a9bbc5;font-size:clamp(22px,1.65vw,30px)}
          #light-recording-fast-forward{position:fixed;z-index:2147483647;top:24px;left:50%;display:flex;align-items:center;gap:18px;max-width:min(900px,80vw);padding:18px 26px;border:2px solid #79edf7;border-radius:14px;color:#f4fcff;background:#07151eef;box-shadow:0 12px 40px #000b,0 0 30px #37d8e555;transform:translateX(-50%);font:600 20px/1.3 Inter,system-ui,sans-serif;pointer-events:none}
          #light-recording-fast-forward strong{color:#79edf7;font-size:42px;line-height:1;text-shadow:0 0 18px #37d8e5}
          #light-recording-fast-forward[data-placement="narrative"]{position:relative;z-index:1;top:auto;left:auto;align-self:center;width:min(900px,94%);max-width:none;min-height:54px;margin:0 auto 14px;padding:9px 16px;box-sizing:border-box;transform:none;font-size:16px}
          #light-recording-fast-forward[data-placement="narrative"] strong{font-size:30px}
        `;
				document.head.append(style);
			}
			let clickLayer = document.querySelector<HTMLElement>(
				"#light-recording-click-layer",
			);
			if (!clickLayer) {
				clickLayer = document.createElement("div");
				clickLayer.id = "light-recording-click-layer";
				document.documentElement.append(clickLayer);
			}
			if (!document.querySelector("#light-recording-title-card")) {
				const card = document.createElement("aside");
				card.id = "light-recording-title-card";
				card.setAttribute("aria-hidden", "true");
				card.innerHTML =
					"<div><img hidden alt=''><strong></strong><p></p></div>";
				document.documentElement.append(card);
			}
			if (visualWindow.__lightRecordingEffectsInstalled) return;
			visualWindow.__lightRecordingEffectsInstalled = true;
			const releaseTimers = new WeakMap<Element, number>();
			const physicalButton = (target: EventTarget | null) =>
				target instanceof Element
					? target.closest<HTMLElement>(
							".demo-number-block button,.product-demo-playback-button,[data-recording-physical-button]",
						)
					: null;
			const release = (target: EventTarget | null) => {
				const button = physicalButton(target);
				if (!button) return;
				const previous = releaseTimers.get(button);
				if (previous) window.clearTimeout(previous);
				releaseTimers.set(
					button,
					window.setTimeout(
						() => button.classList.remove("light-recording-pressed"),
						500,
					),
				);
			};
			document.addEventListener(
				"pointerdown",
				(event) => {
					clickLayer!.dataset.clickCount = String(
						Number(clickLayer!.dataset.clickCount ?? 0) + 1,
					);
					const preview = document.querySelector<HTMLElement>(
						".light-recording-click-preview",
					);
					if (preview) {
						preview.classList.add("committed");
						window.setTimeout(() => preview.remove(), 150);
					}
					const ping = document.createElement("i");
					ping.className = "light-recording-click-ping";
					ping.style.left = `${event.clientX}px`;
					ping.style.top = `${event.clientY}px`;
					clickLayer!.append(ping);
					window.setTimeout(() => ping.remove(), 150);
					const button = physicalButton(event.target);
					if (button) {
						clickLayer!.dataset.physicalPressCount = String(
							Number(clickLayer!.dataset.physicalPressCount ?? 0) + 1,
						);
						const previous = releaseTimers.get(button);
						if (previous) window.clearTimeout(previous);
						button.classList.add("light-recording-pressed");
					}
				},
				true,
			);
			document.addEventListener(
				"pointerup",
				(event) => release(event.target),
				true,
			);
			document.addEventListener(
				"pointercancel",
				(event) => release(event.target),
				true,
			);
		});
	}

	private async renderRecordingOverlay(): Promise<void> {
		await this.page.addStyleTag({
			content: `
      body{position:fixed!important;inset:0 640px 0 0!important;width:auto!important;height:100%!important;transform:translateZ(0)}
      #light-catalog-recording{position:fixed;z-index:999998;inset:0 0 0 auto;width:640px;display:grid;grid-template-rows:auto minmax(0,1fr);gap:14px;padding:18px;border-left:3px solid #1bd6ec;background:#071017;color:#edf7ff;box-shadow:-10px 0 35px #000b;font:16px/1.35 ui-monospace,SFMono-Regular,Menlo,monospace;pointer-events:none}
      #light-catalog-recording .catalog-copy{min-width:0;padding:14px;border:1px solid #344552;border-radius:10px;background:#101922}#light-catalog-recording small{display:block;color:#74ddea;overflow-wrap:anywhere}#light-catalog-recording strong{display:block;margin:8px 0;color:#fff;font:800 25px/1.08 system-ui}#light-catalog-recording p{margin:0;color:#c5d0d8;overflow-wrap:anywhere}
      #light-catalog-recording .catalog-state{min-height:0;display:grid;grid-template-rows:repeat(3,minmax(0,1fr));gap:12px}#light-catalog-recording section{min-width:0;min-height:0;padding:14px;border:1px solid #344552;border-radius:10px;background:#101922;overflow:hidden}#light-catalog-recording b{display:block;margin-bottom:8px;color:#ffc44d;font-size:13px}#light-catalog-recording output{display:block;max-height:100%;overflow:auto;color:#d9eef9;white-space:pre-wrap;overflow-wrap:anywhere;font-size:14px}
    `,
		});
		await this.page.evaluate(
			({ testTitle, recordingStep }) => {
				const previous = document.querySelector("#light-catalog-recording");
				if (previous) {
					previous.querySelector("#light-catalog-step")!.textContent =
						recordingStep.title;
					previous.querySelector("#light-catalog-description")!.textContent =
						recordingStep.description;
					return;
				}
				const overlay = document.createElement("aside");
				overlay.id = "light-catalog-recording";
				overlay.innerHTML = `<div class="catalog-copy"><small id="light-catalog-test"></small><strong id="light-catalog-step"></strong><p id="light-catalog-description"></p></div><div class="catalog-state"><section><b>DESK EVENTS</b><output id="light-catalog-events">Waiting for events…</output></section><section><b>EXTERNAL OSC (TX / RX)</b><output id="light-catalog-osc">No external OSC yet</output></section><section><b>ACTUAL DMX U1</b><output id="light-catalog-dmx">Waiting for output…</output></section></div>`;
				overlay.querySelector("#light-catalog-test")!.textContent = testTitle;
				overlay.querySelector("#light-catalog-step")!.textContent =
					recordingStep.title;
				overlay.querySelector("#light-catalog-description")!.textContent =
					recordingStep.description;
				document.documentElement.append(overlay);
				const purpose = (testTitle.split("›").at(-1) ?? testTitle)
					.trim()
					.slice(0, 150);
				const describeControl = (target: EventTarget | null) => {
					const element =
						target instanceof Element
							? target.closest<HTMLElement>(
									'button,[role="button"],input,textarea,select,[role="slider"],[role="option"],[role="tab"]',
								)
							: null;
					if (!element || element.closest("#light-catalog-recording")) return;
					const label =
						element.getAttribute("aria-label") ||
						element.getAttribute("title") ||
						(element instanceof HTMLInputElement
							? element.labels?.[0]?.textContent
							: null) ||
						element.textContent ||
						element.getAttribute("name") ||
						element.tagName.toLowerCase();
					const concise = label.replace(/\s+/g, " ").trim().slice(0, 140);
					document.querySelector("#light-catalog-step")!.textContent =
						"UI ACTION";
					document.querySelector("#light-catalog-description")!.textContent =
						concise
							? `Using “${concise}” to verify: ${purpose}.`
							: `Operating the production UI to verify: ${purpose}.`;
				};
				document.addEventListener(
					"pointerdown",
					(event) => describeControl(event.target),
					true,
				);
				document.addEventListener(
					"input",
					(event) => describeControl(event.target),
					true,
				);
				document.addEventListener(
					"keydown",
					(event) => {
						if (
							event.key === "Shift" ||
							event.key === "Control" ||
							event.key === "Alt" ||
							event.key === "Meta"
						)
							return;
						describeControl(event.target);
						const description = document.querySelector(
							"#light-catalog-description",
						)!;
						description.textContent = `${description.textContent} Keyboard: ${event.key}.`;
					},
					true,
				);
				const visualWindow = window as Window & {
					__lightVisualDeskState?: () => Promise<{
						dmx: string;
						event: string;
						osc: string;
					}>;
				};
				let updating = false;
				const update = async () => {
					if (updating) return;
					updating = true;
					try {
						const state = await visualWindow.__lightVisualDeskState?.();
						if (!state) return;
						const oscOutput = document.querySelector("#light-catalog-osc");
						if (oscOutput)
							oscOutput.textContent = state.osc || "No external OSC yet";
						document.querySelector("#light-catalog-dmx")!.textContent =
							state.dmx;
						if (state.event)
							document.querySelector("#light-catalog-events")!.textContent =
								state.event;
					} catch {
						// The overlay is evidence only; a transient refresh error must not alter the test.
					} finally {
						updating = false;
					}
				};
				void update();
				window.setInterval(() => void update(), 500);
			},
			{ testTitle: this.testTitle, recordingStep: this.recordingStep },
		);
	}

	private async visualDeskState() {
		const session = await this.session();
		const headers = { Authorization: `Bearer ${session.token}` };
		const [dmxResponse, eventsResponse] = await Promise.all([
			fetch(`${this.baseUrl}/api/v2/output/dmx`, { headers }),
			fetch(`${this.baseUrl}/api/v2/audit?after=${this.auditRevision}`, {
				headers,
			}),
		]);
		const dmx = dmxResponse.ok ? await dmxResponse.json() : null;
		const events = eventsResponse.ok
			? ((await eventsResponse.json()) as VisualEvent[])
			: [];
		if (events.length)
			this.auditRevision = Math.max(
				this.auditRevision,
				...events.map((event) => event.revision),
			);
		return {
			dmx: visualDmx(dmx),
			event: visualEvent(events),
			osc: this.externalOscSummary(),
		};
	}

	async command(
		value: string,
		visibleValue = formatVisibleCommand(value),
	): Promise<void> {
		const command = this.page.getByLabel("Command line");
		await this.page.getByRole("button", { name: "ESC", exact: true }).click();
		const keys = value
			.trim()
			.split(/\s+/)
			.flatMap((token) => {
				if (token === "GROUP") return ["GRP"];
				if (/^\d+$/.test(token)) return [...token];
				return [token];
			});
		for (const key of keys) {
			await this.page.getByRole("button", { name: key, exact: true }).click();
		}
		await expect(command).toHaveValue(visibleValue);
		await this.page.getByRole("button", { name: "ENT", exact: true }).click();
	}

	async openFixtures(): Promise<void> {
		await this.page
			.getByRole("button", { name: "Desktops / Built-ins", exact: true })
			.click();
		await this.page
			.locator(".dock-entry")
			.filter({ hasText: "Fixtures" })
			.click();
	}
}

interface VisualEvent {
	revision: number;
	kind: string;
	payload?: Record<string, unknown>;
}

function visualDmx(value: unknown) {
	const dmx = value as {
		universes?: Array<{ universe: number; slots: number[] }>;
	} | null;
	const slots =
		dmx?.universes
			?.find((universe) => universe.universe === 1)
			?.slots.slice(0, 12) ?? [];
	return (
		slots.map((slot, index) => `${index + 1}:${slot}`).join(" · ") ||
		"No Universe 1 frame"
	);
}

function visualEvent(events: VisualEvent[]) {
	const visible = events.filter(
		(event) =>
			event.kind === "desk_action" ||
			event.payload?.source === "osc" ||
			event.kind.includes("playback") ||
			event.kind.includes("speed_group"),
	);
	const latest = visible.at(-1) ?? events.at(-1);
	if (!latest) return "";
	const action =
		latest.payload?.action ??
		latest.payload?.command ??
		latest.payload?.source ??
		"state changed";
	return `${latest.kind} · ${String(action)}`;
}

function formatVisibleCommand(value: string): string {
	const tokens = value.trim().split(/\s+/);
	let scope: "fixture" | "group" = "fixture";
	let expectsSelectionNumber = true;
	return tokens
		.map((token) => {
			if (token === "GROUP") {
				scope = "group";
				expectsSelectionNumber = true;
				return null;
			}
			if (token === "+" || token === "-") {
				expectsSelectionNumber = true;
				return token;
			}
			if (token === "AT" || token === "THRU") {
				expectsSelectionNumber = token === "THRU";
				return token;
			}
			if (/^\d+$/.test(token) && expectsSelectionNumber) {
				expectsSelectionNumber = false;
				return `${scope === "group" ? "G" : "F"}${token}`;
			}
			return token;
		})
		.filter((token): token is string => token !== null)
		.join(" ");
}
