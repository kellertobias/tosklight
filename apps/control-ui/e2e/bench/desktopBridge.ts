import type { Page } from "@playwright/test";
import {
	DESKTOP_TEST_CONTROL,
	type ControllableDesktopAction,
} from "../../src/platform/desktop/controllableBrowserDesktopBridge";

const ACCEPT_DESKTOP_ACTION = "__lightAcceptDesktopAction";

export class ControllableDesktopDriver {
	readonly actions: ControllableDesktopAction[] = [];
	private installed = false;

	constructor(private readonly page: Page) {}

	async install(): Promise<void> {
		if (this.installed) return;
		this.installed = true;
		await this.page.exposeBinding(
			ACCEPT_DESKTOP_ACTION,
			(_source, action) => this.actions.push(action as ControllableDesktopAction),
		);
		await this.page.addInitScript(
			({ controlName, actionBinding }) => {
				const accept = (window as unknown as Record<string, unknown>)[
					actionBinding
				] as (action: unknown) => Promise<void>;
				Object.defineProperty(window, controlName, {
					configurable: true,
					value: {
						perform: (action: unknown) => accept(action),
						listDisplays: () => [],
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
