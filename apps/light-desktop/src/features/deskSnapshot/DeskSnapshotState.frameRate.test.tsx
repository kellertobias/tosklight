import { cleanup, render, screen } from "@testing-library/react";
import { act } from "react";
import { afterEach, describe, expect, it } from "vitest";
import type { BootstrapSnapshot, DeskConfiguration } from "../../api/types";
import { ConfigurationStateProvider } from "../configuration/ConfigurationState";
import { ConfigurationStore } from "../configuration/store";
import {
	DeskSnapshotStateProvider,
	useFrameRateHz,
	useOutputHealth,
} from "./DeskSnapshotState";
import { DeskSnapshotStore } from "./store";

const configuration = (frame_rate_hz: number) =>
	({ frame_rate_hz }) as DeskConfiguration;
const bootstrap = {
	frame_rate_hz: 44,
	output_health: { frame_hz: 39.5 },
} as BootstrapSnapshot;

describe("configured output rate", () => {
	afterEach(cleanup);

	it("follows authoritative configuration without replacing bootstrap or measured health", () => {
		const desk = new DeskSnapshotStore();
		const config = new ConfigurationStore();
		desk.install(bootstrap, null);
		let renders = 0;
		function Reader() {
			renders += 1;
			const rate = useFrameRateHz();
			const health = useOutputHealth();
			return <output>{`${rate}/${health?.frame_hz}`}</output>;
		}
		render(
			<DeskSnapshotStateProvider store={desk}>
				<ConfigurationStateProvider store={config}>
					<Reader />
				</ConfigurationStateProvider>
			</DeskSnapshotStateProvider>,
		);
		expect(screen.getByRole("status").textContent).toBe("44/39.5");
		// The production configuration response/event path installs this separate store.
		act(() => config.install(configuration(40)));
		expect(screen.getByRole("status").textContent).toBe("40/39.5");
		act(() => config.install(configuration(44)));
		expect(screen.getByRole("status").textContent).toBe("44/39.5");
		const before = renders;
		act(() =>
			config.install({ ...configuration(44), programmer_fade_millis: 1234 }),
		);
		expect(renders).toBe(before);
		expect(desk.getSnapshot().bootstrap).toBe(bootstrap);
	});

	it("falls back to bootstrap with no configuration boundary", () => {
		const desk = new DeskSnapshotStore();
		desk.install(bootstrap, null);
		function Reader() {
			return <output>{useFrameRateHz() ?? "Unknown"}</output>;
		}
		render(
			<DeskSnapshotStateProvider store={desk}>
				<Reader />
			</DeskSnapshotStateProvider>,
		);
		expect(screen.getByRole("status").textContent).toBe("44");
	});

	it("can show known configuration before bootstrap, then falls back when configuration clears", () => {
		const desk = new DeskSnapshotStore();
		const config = new ConfigurationStore();
		function Reader() {
			return <output>{useFrameRateHz() ?? "Unknown"}</output>;
		}
		render(
			<DeskSnapshotStateProvider store={desk}>
				<ConfigurationStateProvider store={config}>
					<Reader />
				</ConfigurationStateProvider>
			</DeskSnapshotStateProvider>,
		);
		expect(screen.getByRole("status").textContent).toBe("Unknown");
		act(() => config.install(configuration(40)));
		expect(screen.getByRole("status").textContent).toBe("40");
		act(() => desk.install(bootstrap, null));
		expect(screen.getByRole("status").textContent).toBe("40");
		act(() => config.install(null));
		expect(screen.getByRole("status").textContent).toBe("44");
		act(() => desk.install(null, null));
		expect(screen.getByRole("status").textContent).toBe("Unknown");
	});
});
