import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { EFFECT_TYPES, type EffectLibrarySlot } from "../../shared/api/effects";
import { resetResources } from "../../shared/api/resource";
import { EffectsPage } from "./EffectsPage";

afterEach(() => {
	vi.unstubAllGlobals();
	resetResources();
});

describe("the effects library", () => {
	it("shows all 255 assignable slots and the exact effect catalogue", async () => {
		stubEffects([]);
		const { container } = render(<EffectsPage />);

		expect(await screen.findByText("0/255 assigned")).toBeInTheDocument();
		expect(
			container.querySelectorAll(".media-effects-pool-grid .pool-card"),
		).toHaveLength(255);
		expect(screen.getByRole("tablist")).toHaveTextContent(
			"MediaVisualizersTextEffects",
		);
		expect(EFFECT_TYPES.map((effect) => effect.label)).toEqual([
			"TV/CRT/VHS Simulation",
			"Digital Video/ Glitch Simulation",
			"Blur",
			"Feedback",
			"Beat Move",
			"Beat Scan",
			"Beat Scale & Turn",
			"Beat form Flash",
			"Kaleidoscope",
			"B/W Rasterize",
			"CMYK Rasterize",
			"Drawn Image Style",
		]);
	});

	it("assigns, edits, and clears one stable effect slot", async () => {
		const server = stubEffects([]);
		const { container } = render(<EffectsPage />);
		await screen.findByText("0/255 assigned");

		const cards = container.querySelectorAll(
			".media-effects-pool-grid .pool-card",
		);
		await userEvent.click(cards[1] as HTMLElement);
		await userEvent.type(screen.getByLabelText("Name"), "Broadcast damage");
		await userEvent.click(
			screen.getByRole("button", { name: "Assign effect" }),
		);

		await waitFor(() => expect(server.writes).toHaveLength(1));
		expect(server.writes[0]?.path).toBe("/api/v2/effects/2/update");
		expect(server.writes[0]?.body).toMatchObject({
			name: "Broadcast damage",
			effectType: "analog-tv",
			parameters: [],
		});
		expect(server.writes[0]?.body.requestId).toEqual(expect.any(String));

		await waitFor(() =>
			expect(screen.getByRole("button", { name: "Clear slot" })).toBeVisible(),
		);
		await userEvent.click(screen.getByRole("button", { name: "Clear slot" }));
		await waitFor(() => expect(server.writes).toHaveLength(2));
		expect(server.writes[1]).toMatchObject({
			path: "/api/v2/effects/2/update",
			body: { clear: true },
		});
	});

	it("edits the ordered settings advertised by the selected effect", async () => {
		const server = stubEffects([anEffect()]);
		render(<EffectsPage />);
		await screen.findByText("1/255 assigned");

		fireEvent.change(screen.getByLabelText("TV curvature"), {
			target: { value: "0.7" },
		});
		await userEvent.click(screen.getByRole("button", { name: "Save effect" }));

		await waitFor(() => expect(server.writes).toHaveLength(1));
		expect(server.writes[0]?.body.parameters).toEqual([0.7]);
	});

	it("names Blur, Feedback, and Kaleidoscope choices exactly", async () => {
		const effects = [
			typedEffect(1, "Blur", "blur", "blur-type", "Blur type", 0, 0, 4),
			typedEffect(
				2,
				"Feedback",
				"feedback",
				"feedback-direction",
				"Direction",
				7,
				0,
				7,
			),
			typedEffect(
				3,
				"Kaleidoscope",
				"kaleidoscope",
				"kaleidoscope-repetitions",
				"Mirror repetitions",
				0,
				0,
				12,
			),
		];
		stubEffects(effects);
		const { container } = render(<EffectsPage />);

		await screen.findByText("3/255 assigned");
		await userEvent.click(screen.getByRole("button", { name: "Blur type" }));
		expect(screen.getByRole("option", { name: "Gaussian" })).toBeVisible();
		await userEvent.keyboard("{Escape}");
		await userEvent.click(
			container.querySelectorAll(
				".media-effects-pool-grid .pool-card",
			)[1] as HTMLElement,
		);
		await userEvent.click(screen.getByRole("button", { name: "Direction" }));
		expect(screen.getByRole("option", { name: "Tunnel" })).toBeVisible();
		expect(screen.getByRole("option", { name: "Shake" })).toBeVisible();
		await userEvent.keyboard("{Escape}");
		await userEvent.click(
			container.querySelectorAll(
				".media-effects-pool-grid .pool-card",
			)[2] as HTMLElement,
		);
		await userEvent.click(
			screen.getByRole("button", { name: "Mirror repetitions" }),
		);
		expect(screen.getByRole("option", { name: "Off" })).toBeVisible();
		expect(screen.getByRole("option", { name: "12" })).toBeVisible();
	});
});

function stubEffects(initial: EffectLibrarySlot[]) {
	const effects = structuredClone(initial);
	const writes: Array<{ path: string; body: Record<string, unknown> }> = [];
	vi.stubGlobal(
		"fetch",
		vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
			const path = String(input);
			if (path === "/api/v2/effects" && !init?.method)
				return Response.json(effects);
			const match = /^\/api\/v2\/effects\/(\d+)\/update$/u.exec(path);
			if (match && init?.method === "POST") {
				const slot = Number(match[1]);
				const body = JSON.parse(String(init.body)) as Record<string, unknown>;
				writes.push({ path, body });
				const index = effects.findIndex((effect) => effect.slot === slot);
				if (body.clear) {
					if (index >= 0) effects.splice(index, 1);
					return Response.json(null);
				}
				const effect = {
					slot,
					name: String(body.name),
					effect: {
						index: 0,
						effectType: String(body.effectType),
						label: String(body.name),
						enabled: true,
						mix: 1,
						supported: true,
						capabilityDetail: null,
						parameters: [],
					},
				} as EffectLibrarySlot;
				if (index >= 0) effects[index] = effect;
				else effects.push(effect);
				return Response.json(effect);
			}
			return Response.json(
				{ code: "not-found", message: path },
				{ status: 404 },
			);
		}),
	);
	return { effects, writes };
}

function anEffect(): EffectLibrarySlot {
	return {
		slot: 1,
		name: "Broadcast damage",
		effect: {
			index: 0,
			effectType: "analog-tv",
			label: "TV/CRT/VHS Simulation",
			enabled: true,
			mix: 1,
			supported: true,
			capabilityDetail: null,
			parameters: [
				{
					id: "tv-curvature",
					label: "TV curvature",
					value: 0.3,
					defaultValue: 0.3,
					minimum: 0,
					maximum: 1,
					step: 0.01,
				},
			],
		},
	};
}

function typedEffect(
	slot: number,
	name: string,
	effectType: string,
	id: string,
	label: string,
	value: number,
	minimum: number,
	maximum: number,
): EffectLibrarySlot {
	return {
		slot,
		name,
		effect: {
			index: 0,
			effectType,
			label: name,
			enabled: true,
			mix: 1,
			supported: true,
			capabilityDetail: null,
			parameters: [
				{
					id,
					label,
					value,
					defaultValue: value,
					minimum,
					maximum,
					step: 1,
				},
			],
		},
	};
}
