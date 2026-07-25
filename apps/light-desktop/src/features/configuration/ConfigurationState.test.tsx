import { cleanup, render } from "@testing-library/react";
import { act } from "react";
import { afterEach, describe, expect, it } from "vitest";
import type { DeskConfiguration } from "../../api/types";
import {
	ConfigurationStateProvider,
	useProgrammerFadeMillis,
	useSequenceMasterFadeMillis,
	useSpeedGroupsBpm,
} from "./ConfigurationState";
import { ConfigurationStore } from "./store";

function configuration(
	overrides: Partial<DeskConfiguration> = {},
): DeskConfiguration {
	return {
		programmer_fade_millis: 3_000,
		sequence_master_fade_millis: 3_000,
		speed_groups_bpm: [120, 90, 60, 30, 15],
		...overrides,
	} as DeskConfiguration;
}

describe("scoped desk configuration", () => {
	afterEach(cleanup);

	it("does not rerender a reader when an unrelated setting changes", () => {
		const store = new ConfigurationStore();
		store.install(configuration());
		let renders = 0;
		function Reader() {
			renders += 1;
			useProgrammerFadeMillis();
			return null;
		}
		render(
			<ConfigurationStateProvider store={store}>
				<Reader />
			</ConfigurationStateProvider>,
		);
		expect(renders).toBe(1);

		act(() =>
			store.install(configuration({ sequence_master_fade_millis: 9_000 })),
		);

		expect(renders).toBe(1);
	});

	it("rerenders a reader when its own setting changes", () => {
		const store = new ConfigurationStore();
		store.install(configuration());
		let renders = 0;
		const observed: { current: number | null } = { current: null };
		function Reader() {
			renders += 1;
			observed.current = useSequenceMasterFadeMillis();
			return null;
		}
		render(
			<ConfigurationStateProvider store={store}>
				<Reader />
			</ConfigurationStateProvider>,
		);

		act(() =>
			store.install(configuration({ sequence_master_fade_millis: 9_000 })),
		);

		expect(renders).toBe(2);
		expect(observed.current).toBe(9_000);
	});

	it("keeps an equal speed-group array stable across a replaced configuration", () => {
		const store = new ConfigurationStore();
		store.install(configuration());
		let renders = 0;
		function Reader() {
			renders += 1;
			useSpeedGroupsBpm();
			return null;
		}
		render(
			<ConfigurationStateProvider store={store}>
				<Reader />
			</ConfigurationStateProvider>,
		);

		// A fresh configuration object carrying an equal array must not rerender the reader.
		act(() =>
			store.install(
				configuration({ speed_groups_bpm: [120, 90, 60, 30, 15] }),
			),
		);

		expect(renders).toBe(1);
	});

	it("reports no configuration outside a mounted boundary", () => {
		const observed: { current: number | null } = { current: 1 };
		function Reader() {
			observed.current = useProgrammerFadeMillis();
			return null;
		}
		render(<Reader />);

		expect(observed.current).toBeNull();
	});
});
