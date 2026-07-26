import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DeskConfiguration } from "../../api/types";
import { defaultPoolPresentation } from "../poolPresentation/poolPresentation";
import {
	ConfigurationActionsProvider,
	useConfigurationActions,
} from "./ConfigurationActionsProvider";
import { ConfigurationStore } from "./store";

afterEach(cleanup);

describe("ConfigurationActionsProvider", () => {
	it("writes pool presentation through its isolated partial-update capability", async () => {
		const store = new ConfigurationStore();
		store.install({ frame_rate_hz: 44 } as DeskConfiguration);
		const updateConfiguration = vi.fn();
		const updatePoolPresentation = vi.fn(async (pool_presentation) => ({
			configuration: {
				frame_rate_hz: 44,
				pool_presentation,
			} as DeskConfiguration,
			requires_restart: false,
			matter: {} as never,
			request_id: "pool-1",
			replayed: false,
		}));
		const onApplied = vi.fn();
		function Consumer() {
			const actions = useConfigurationActions();
			return (
				<button
					type="button"
					onClick={() =>
						void actions?.setPoolPresentation(defaultPoolPresentation())
					}
				>
					Save colors
				</button>
			);
		}
		render(
			<ConfigurationActionsProvider
				store={store}
				updateConfiguration={updateConfiguration}
				updatePoolPresentation={updatePoolPresentation}
				onApplied={onApplied}
				onError={vi.fn()}
			>
				<Consumer />
			</ConfigurationActionsProvider>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Save colors" }));
		await waitFor(() => expect(updatePoolPresentation).toHaveBeenCalledOnce());
		expect(updateConfiguration).not.toHaveBeenCalled();
		expect(onApplied).toHaveBeenCalledOnce();
	});
});
