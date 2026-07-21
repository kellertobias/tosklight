import type { Page } from "@playwright/test";
import { OSC_TEST_CONTROL } from "../../../hardware-controls/src/transport/controllableOscBridge";
import type {
	ControlArgument,
	ControllerSettings,
} from "../../../hardware-controls/src/controller/types";

const ACCEPT_CONNECTION = "__lightAcceptOscConnection";
const ACCEPT_WRITE = "__lightAcceptOscWrite";

interface OscWrite {
	path: string;
	arguments: ControlArgument[];
}

export class ControllableHardwareOscDriver {
	readonly connections: Array<Pick<ControllerSettings, "host" | "port" | "desk">> = [];
	readonly writes: OscWrite[] = [];
	private installed = false;

	constructor(private readonly page: Page) {}

	async install(): Promise<void> {
		if (this.installed) return;
		this.installed = true;
		await this.page.exposeBinding(
			ACCEPT_CONNECTION,
			(_source, settings) => this.connections.push(
				settings as Pick<ControllerSettings, "host" | "port" | "desk">,
			),
		);
		await this.page.exposeBinding(
			ACCEPT_WRITE,
			(_source, write) => this.writes.push(write as OscWrite),
		);
		await this.installBrowserPort();
	}

	clear(): void {
		this.writes.length = 0;
	}

	values(path: string): ControlArgument[] {
		return this.writes
			.filter((write) => write.path === path)
			.map((write) => write.arguments[0]);
	}

	programmerButtonWrites(): Array<[string, boolean]> {
		return this.writes
			.filter((write) => /^programmer\/(?:shift|record)$/.test(write.path))
			.map((write) => [write.path, Boolean(write.arguments[0])]);
	}

	private installBrowserPort() {
		return this.page.addInitScript(
			({ controlName, connectBinding, writeBinding }) => {
				const bindings = window as unknown as Record<string, unknown>;
				const connect = bindings[connectBinding] as (
					settings: unknown,
				) => Promise<void>;
				const write = bindings[writeBinding] as (
					value: unknown,
				) => Promise<void>;
				Element.prototype.setPointerCapture = () => undefined;
				Object.defineProperty(window, controlName, {
					configurable: true,
					value: {
						connect,
						send: (path: string, arguments_: unknown[]) =>
							write({ path, arguments: arguments_ }),
						listenFeedback: async () => () => undefined,
					},
				});
			},
			{
				controlName: OSC_TEST_CONTROL,
				connectBinding: ACCEPT_CONNECTION,
				writeBinding: ACCEPT_WRITE,
			},
		);
	}
}
