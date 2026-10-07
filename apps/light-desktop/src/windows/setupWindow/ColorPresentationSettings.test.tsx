import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DeskManagementApiClient } from "../../api/client/deskManagement";
import type { DeskConfiguration } from "../../api/types";
import { selectColorPresentation } from "../../features/configuration/selectors";
import type { ConfigurationSnapshot } from "../../features/configuration/store";
import { ColorPresentationSettings } from "./ColorModelSettings";
import type { SetupWindowController } from "./controller";

afterEach(cleanup);

const desk = { frame_rate_hz: 44, output_bind_ip: "0.0.0.0" } as DeskConfiguration;

function controller(draft: DeskConfiguration) {
	return {
		draft,
		editDraft: vi.fn(),
		attributeActions: { update: vi.fn(), canWrite: true },
		editAttributeConfiguration: vi.fn(),
	} as unknown as SetupWindowController & {
		editDraft: ReturnType<typeof vi.fn>;
		attributeActions: { update: ReturnType<typeof vi.fn> };
		editAttributeConfiguration: ReturnType<typeof vi.fn>;
	};
}

describe("Setup › Color controls on this desk (color_presentation)", () => {
	it("defaults to Easy and writes only the desk setting, never the show", () => {
		const setup = controller(desk);
		render(<ColorPresentationSettings controller={setup} />);
		const trigger = screen.getByRole("button", { name: "Color encoder presentation" });
		expect(trigger).toHaveTextContent("Easy — Red, Green, Blue, White Blend");
		fireEvent.click(trigger);
		fireEvent.click(screen.getByRole("option", { name: /Advanced/ }));
		expect(setup.editDraft).toHaveBeenCalledWith({ ...desk, color_presentation: "advanced" });
		expect(setup.attributeActions.update).not.toHaveBeenCalled();
		expect(setup.editAttributeConfiguration).not.toHaveBeenCalled();
	});

	it("round-trips through the desk configuration route and the desk selector", async () => {
		const request = vi.fn(async () => ({ requires_restart: false }));
		const client = new DeskManagementApiClient({ request } as never);
		await client.updateConfiguration({ ...desk, color_presentation: "easy_rgbwauv" });
		expect(request).toHaveBeenCalledWith(
			"/api/v2/configuration/update",
			expect.objectContaining({ method: "POST" }),
		);
		const [, init] = request.mock.calls[0] as unknown as [string, RequestInit];
		expect(JSON.parse(String(init.body)).patch.color_presentation).toBe("easy_rgbwauv");
		const snapshot = { configuration: { ...desk, color_presentation: "easy_rgbwauv" } };
		expect(selectColorPresentation(snapshot as unknown as ConfigurationSnapshot)).toBe("easy_rgbwauv");
		expect(selectColorPresentation({ configuration: desk } as unknown as ConfigurationSnapshot)).toBe("easy_rgbw");
	});

	it("shows the stored desk value", () => {
		render(<ColorPresentationSettings controller={controller({ ...desk, color_presentation: "advanced" })} />);
		expect(screen.getByRole("button", { name: "Color encoder presentation" })).toHaveTextContent(
			"Advanced — adds Temperature, Duv and colour wheels",
		);
	});
});
