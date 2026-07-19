import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../../apps/control-ui/e2e/bench/pairedScenario";
import { blankFixtureProfile } from "../../apps/control-ui/src/components/setup/fixtureProfileModel";
import { loadCanonicalCopy } from "../support/catalog";
import { assignFaderlessMatterPlayback } from "../support/updateHighlight/matter";

interface FixtureProfileState {
	manufacturer: string;
	name: string;
}

interface MatterScenarioState {
	observed: any | null;
	page: number;
	slot: number;
	emptySlot: number;
	playbackNumber: number;
}

pairedScenario<FixtureProfileState>({
	id: "FIXTURE-001",
	title:
		"a complete fixture profile is created through the desk-wide revisioned library",
	arrange: async ({ api, bench }, surface) => {
		await loadCanonicalCopy(api, bench, `fixture-001-${surface}`);
		return {
			manufacturer: `Acceptance ${surface}`,
			name: `Revisioned profile ${crypto.randomUUID().slice(0, 8)}`,
		};
	},
	api: async ({ api }, state) => {
		const profile = blankFixtureProfile();
		profile.manufacturer = state.manufacturer;
		profile.name = state.name;
		await api.request("PUT", "/api/v1/fixture-profiles", profile, true, 0);
	},
	ui: async ({ bench, desk, page }, state) => {
		await desk.open(bench.baseUrl);
		await page.getByRole("button", { name: /Open show menu/ }).click();
		await page
			.getByRole("button", { name: "Enter Setup", exact: true })
			.click();
		await page
			.getByRole("button", { name: "Open Fixture Library", exact: true })
			.click();
		await expect(
			page.getByRole("dialog", { name: "Fixture Library" }),
		).toBeVisible();
		await page
			.getByRole("button", { name: "Create fixture", exact: true })
			.click();
		const editor = page.getByRole("dialog", { name: "Create fixture profile" });
		await editor.getByLabel(/^Manufacturer/).fill(state.manufacturer);
		await editor.getByLabel(/^Fixture name/).fill(state.name);
		await editor
			.getByRole("button", { name: "Save fixture", exact: true })
			.click();
		await expect(editor).toBeHidden();
	},
	assert: async ({ api }, state) => {
		const profiles = await api.request<any[]>(
			"GET",
			"/api/v1/fixture-profiles",
			undefined,
			false,
		);
		const profile = profiles.find(
			(candidate) =>
				candidate.manufacturer === state.manufacturer &&
				candidate.name === state.name,
		);
		expect(profile).toBeDefined();
		expect(profile).toMatchObject({ schema_version: 2, revision: 1 });
		expect(profile.modes).toHaveLength(1);
		expect(profile.modes[0]).toMatchObject({
			name: "Default",
			splits: [{ number: 1, footprint: 1 }],
		});
		const revisions = await api.request<any[]>(
			"GET",
			`/api/v1/fixture-profiles/${profile.id}/revisions`,
			undefined,
			false,
		);
		expect(revisions.map((candidate) => candidate.revision)).toEqual([1]);
	},
});

pairedScenario<MatterScenarioState>({
	id: "MATTER-001",
	title:
		"the desk-persistent Matter bridge toggle exposes stable explicit page playback lights",
	arrange: async ({ api, bench }, surface) => {
		await loadCanonicalCopy(api, bench, `matter-001-${surface}`);
		const response = await api.request<any>(
			"GET",
			"/api/v1/configuration",
			undefined,
			false,
		);
		if (response.configuration.matter_enabled) {
			await api.request("PUT", "/api/v1/configuration", {
				...response.configuration,
				matter_enabled: false,
			});
		}
		const assignment = await assignFaderlessMatterPlayback(api);
		return { observed: null, ...assignment };
	},
	api: async ({ api }, state) => {
		const response = await api.request<any>(
			"GET",
			"/api/v1/configuration",
			undefined,
			false,
		);
		await api.request("PUT", "/api/v1/configuration", {
			...response.configuration,
			matter_enabled: true,
		});
		state.observed = await api.request<any>("GET", "/api/v1/matter/status");
		const enabled = await api.request<any>(
			"GET",
			"/api/v1/configuration",
			undefined,
			false,
		);
		await api.request("PUT", "/api/v1/configuration", {
			...enabled.configuration,
			matter_enabled: false,
		});
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await desk.open(bench.baseUrl);
		await page.getByRole("button", { name: /Open show menu/ }).click();
		await page
			.getByRole("button", { name: "Enter Setup", exact: true })
			.click();
		await page
			.locator(".setup-window nav")
			.getByRole("button", { name: "Network & Inputs", exact: true })
			.click();
		const settings = page.locator(
			'article[aria-label="Matter playback bridge"]',
		);
		const toggle = settings.getByRole("switch", {
			name: "Matter server disabled",
		});
		await expect(
			settings.getByText(
				"Desk installation · shared across shows and Desktops",
			),
		).toBeVisible();
		await expect(toggle).not.toBeChecked();
		await toggle.click();
		await expect(
			settings.getByRole("switch", { name: "Matter server enabled" }),
		).toBeChecked();
		await expect
			.poll(
				async () =>
					(
						await api.request<any>(
							"GET",
							"/api/v1/configuration",
							undefined,
							false,
						)
					).configuration.matter_enabled,
			)
			.toBe(true);
		state.observed = await api.request<any>("GET", "/api/v1/matter/status");
		await settings
			.getByRole("switch", { name: "Matter server enabled" })
			.click();
		await expect
			.poll(
				async () =>
					(
						await api.request<any>(
							"GET",
							"/api/v1/configuration",
							undefined,
							false,
						)
					).configuration.matter_enabled,
			)
			.toBe(false);
	},
	assert: async ({ api }, state) => {
		expect(state.observed).toBeTruthy();
		expect(state.observed.enabled).toBe(true);
		const endpointIds = state.observed.lights.map(
			(light: any) => light.endpoint_id,
		);
		expect(new Set(endpointIds).size).toBe(endpointIds.length);
		for (const light of state.observed.lights) {
			expect(light.endpoint_id).toBe(
				1 + (light.page - 1) * 127 + (light.playback - 1),
			);
			expect(light.playback_number).toBeGreaterThan(0);
			expect(light.level).toBeGreaterThanOrEqual(0);
			expect(light.level).toBeLessThanOrEqual(254);
		}
		const faderlessEndpoint = 1 + (state.page - 1) * 127 + (state.slot - 1);
		expect(state.observed.lights).toContainEqual(
			expect.objectContaining({
				endpoint_id: faderlessEndpoint,
				page: state.page,
				playback: state.slot,
				playback_number: state.playbackNumber,
				name: expect.stringContaining("Matter Button Only"),
			}),
		);
		const emptyEndpoint = 1 + (state.page - 1) * 127 + (state.emptySlot - 1);
		expect(endpointIds).not.toContain(emptyEndpoint);
		const configuration = await api.request<any>(
			"GET",
			"/api/v1/configuration",
			undefined,
			false,
		);
		expect(configuration.configuration.matter_enabled).toBe(false);
		const disabled = await api.request<any>("GET", "/api/v1/matter/status");
		expect(disabled.lights).toEqual([]);
	},
});
