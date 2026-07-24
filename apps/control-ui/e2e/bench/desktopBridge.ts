import type { Page } from "@playwright/test";
import {
	DESKTOP_TEST_CONTROL,
	type ControllableDesktopAction,
} from "../../src/platform/desktop/controllableBrowserDesktopBridge";

const ACCEPT_DESKTOP_ACTION = "__lightAcceptDesktopAction";

export class ControllableDesktopDriver {
	readonly actions: ControllableDesktopAction[] = [];
	private installed = false;
	private displays: Array<{ id: string; name: string }> = [];

	setDisplays(displays: Array<{ id: string; name: string }>): void {
		this.displays = structuredClone(displays);
	}

	constructor(private readonly page: Page) {}

	async install(): Promise<void> {
		if (this.installed) return;
		this.installed = true;
		await this.page.exposeBinding(
			ACCEPT_DESKTOP_ACTION,
			(_source, action) => this.actions.push(action as ControllableDesktopAction),
		);
		await this.page.exposeBinding(
			"__lightListDesktopDisplays",
			() => structuredClone(this.displays),
		);
		await this.page.addInitScript(
			({ controlName, actionBinding }) => {
				const accept = (window as unknown as Record<string, unknown>)[
					actionBinding
				] as (action: unknown) => Promise<void>;
				const listDisplays = (window as unknown as Record<string, unknown>)[
					"__lightListDesktopDisplays"
				] as () => Promise<Array<{ id: string; name: string }>>;
				Object.defineProperty(window, controlName, {
					configurable: true,
					value: {
						perform: (action: unknown) => accept(action),
						listDisplays,
						currentWindowState: () => ({
							displayId: null,
							bounds: { x: 0, y: 0, width: 1440, height: 1080 },
							fullscreen: false,
						}),
						subscribe: () => () => undefined,
					},
				});
			},
			{
				controlName: DESKTOP_TEST_CONTROL,
				actionBinding: ACCEPT_DESKTOP_ACTION,
			},
		);
	}
}
