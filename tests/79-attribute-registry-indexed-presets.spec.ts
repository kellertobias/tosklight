import { expect, test } from "./bench/core/fixtures";
import { CURRENT_FIXTURE_PROFILE_SCHEMA_VERSION } from "./support/fixtureSchema";

interface AttributeConfigurationSnapshot {
	configuration: {
		custom_attributes: Array<{
			label: string;
			value_type: string;
			lifecycle: string;
		}>;
	};
}

interface ProgrammerProjection {
	session_id: string;
	values: Array<{ fixture_id: string; attribute: string }>;
}

test.describe("docs/testing/04-osc-api-and-cross-surface.md", () => {
	test("CROSS-004 @ui › Desk Setup creates a fully placed show-owned attribute", async ({
		api,
		desk,
		page,
	}) => {
		await desk.open(api.baseUrl);
		await page.getByRole("button", { name: /Open show menu/ }).click();
		await page
			.locator(".show-modal")
			.getByRole("button", { name: "Enter Setup", exact: true })
			.click();
		await page
			.locator(".setup-window nav")
			.getByRole("button", { name: "Attributes & encoders", exact: true })
			.click();
		const registry = page.locator(".programmer-setup-list");
		for (const group of [
			"Intensity",
			"Color",
			"Position",
			"Beam",
			"Shapers",
			"Focus",
			"Control",
			"Media",
		])
			await expect(
				registry.getByRole("heading", { name: group, exact: true }),
			).toBeVisible();
		const mediaFolderSlot = registry.getByLabel("media page 1 encoder 1");
		await expect(mediaFolderSlot).toContainText("Media Folder");
		const bladeFourAngleSlot = registry.getByLabel("shapers page 2 encoder 4");
		await expect(bladeFourAngleSlot).toContainText("Blade 4 Angle");

		await page.getByRole("button", { name: "Attributes", exact: true }).click();
		await registry
			.getByLabel("New custom attribute")
			.fill("Test aperture mode");
		await registry
			.getByRole("button", { name: "Create custom attribute" })
			.click();
		await expect(
			registry.getByText("Test aperture mode").first(),
		).toBeVisible();
		const updateCompleted = page.waitForResponse(
			(response) =>
				response.url().endsWith("/api/v2/attribute-configuration/update") &&
				response.request().method() === "POST",
		);
		await page.getByRole("button", { name: "Save changes" }).click();
		expect((await updateCompleted).ok()).toBe(true);

		await expect
			.poll(async () => {
				const snapshot = await api.request<AttributeConfigurationSnapshot>(
					"GET",
					"/api/v2/attribute-configuration",
				);
				return snapshot.configuration.custom_attributes.find(
					(descriptor) => descriptor.label === "Test aperture mode",
				);
			})
			.toMatchObject({
				value_type: "continuous",
				lifecycle: "active",
			});
	});

	test("CROSS-004 @ui › one Indexed Preset resolves each embedded fixture profile", async ({
		api,
		bench,
		desk,
		page,
		show,
	}) => {
		const first = indexedFixture(901, 30, 93);
		const second = indexedFixture(902, 31, 41);
		await api.seedShowObject(
			show.id,
			"patched_fixture",
			first.fixture_id,
			first,
		);
		await api.seedShowObject(
			show.id,
			"patched_fixture",
			second.fixture_id,
			second,
		);
		await api.openShow(show.id, { transition: "hold_current" });
		await api.executeCommandLine("FIXTURE 901 THRU 902");
		await desk.open(api.baseUrl);

		let beam = page.getByRole("button", { name: "Beam", exact: true });
		if (!(await beam.isVisible())) {
			await page
				.getByRole("button", {
					name: "Desktops / Built-ins",
					exact: true,
				})
				.click();
			await page
				.locator("[aria-label='Built-ins']")
				.getByRole("button", { name: "Fixtures", exact: true })
				.click();
			beam = page.getByRole("button", { name: "Beam", exact: true });
		}
		await beam.click();
		await page
			.getByRole("button", { name: "Set Enc 1 · Gobo 1 value" })
			.click();
		const setValue = page.getByRole("dialog", {
			name: "Enc 1 · Gobo 1 value",
		});
		await expect(setValue.getByText("Direct input")).toBeVisible();
		await setValue.getByRole("button", { name: "Show presets" }).click();
		const dots = setValue.getByRole("button", { name: /Dots/ });
		await expect(dots).toContainText("All selected fixtures");
		await dots.click();

		const programmer = (
			await api.request<ProgrammerProjection[]>("GET", "/api/v2/programmers")
		).find((entry) => entry.session_id === api.session?.session_id);
		expect(programmer).toBeDefined();
		expect(
			programmer?.values
				.filter((entry) => entry.attribute === "gobo.1")
				.map((entry) => entry.fixture_id)
				.sort(),
		).toEqual([first.fixture_id, second.fixture_id].sort());

		const frame = await bench.tick();
		const universe = frame.universes.find((entry) => entry.universe === 1);
		expect(universe?.slots.slice(29, 31)).toEqual([93, 41]);
	});
});

function indexedFixture(
	fixtureNumber: number,
	address: number,
	rawValue: number,
) {
	const fixtureId = crypto.randomUUID();
	const profileId = crypto.randomUUID();
	const modeId = crypto.randomUUID();
	const headId = crypto.randomUUID();
	const channelId = crypto.randomUUID();
	const functionId = crypto.randomUUID();
	const name = `Indexed fixture ${fixtureNumber}`;
	const profile = {
		schema_version: CURRENT_FIXTURE_PROFILE_SCHEMA_VERSION,
		id: profileId,
		revision: 1,
		manufacturer: "ToskLight Test",
		name,
		short_name: name,
		fixture_type: "moving_head",
		patch_policy: "dmx",
		photograph_asset: null,
		stage_icon_asset: null,
		model_asset: null,
		model_units: "auto",
		physical: {},
		modes: [
			{
				id: modeId,
				name: "Indexed",
				splits: [{ number: 1, footprint: 1 }],
				heads: [{ id: headId, name: "Main", master_shared: true }],
				channels: [
					{
						id: channelId,
						head_id: headId,
						split: 1,
						fixture_attribute: "gobo.1",
						attribute: "gobo.1",
						canonical_transform: "identity",
						resolution: "u8",
						secondary_slots: [],
						default_raw: 0,
						highlight_raw: 0,
						physical_min: null,
						physical_max: null,
						unit: null,
						invert: false,
						snap: true,
						reacts_to_virtual_intensity: false,
						reacts_to_sequence_master: false,
						reacts_to_group_master: false,
						reacts_to_grand_master: false,
						behavior: "controlled",
						functions: [
							{
								id: functionId,
								name: "Dots",
								dmx_from: 0,
								dmx_to: 255,
								attribute: "gobo.1",
								priority: 100,
								behavior: {
									type: "indexed",
									semantic_id: "gobo.dots",
									label: "Dots",
									raw_value: rawValue,
								},
							},
						],
					},
				],
				control_actions: [],
				color_systems: [],
				geometry: { nodes: [], emitters: [] },
			},
		],
	};
	return {
		fixture_id: fixtureId,
		fixture_number: fixtureNumber,
		name,
		definition: {
			schema_version: CURRENT_FIXTURE_PROFILE_SCHEMA_VERSION,
			id: profileId,
			revision: 1,
			manufacturer: "ToskLight Test",
			device_type: "moving_head",
			name,
			model: name,
			mode: "Indexed",
			footprint: 1,
			heads: [
				{
					index: 0,
					name: "Main",
					shared: true,
					parameters: [
						{
							attribute: "gobo.1",
							components: [{ offset: 0, byte_order: "msb_first" }],
							default: 0,
							virtual_dimmer: false,
							metadata: {
								physical_min: 0,
								physical_max: 1,
								unit: null,
								invert: false,
								wrap: false,
								curve: "linear",
							},
							capabilities: [],
						},
					],
				},
			],
			color_calibration: null,
			physical: {},
			model_asset: null,
			icon_asset: null,
			hazardous: false,
			direct_control_protocols: [],
			signal_loss_policy: { type: "hold_last" },
			safe_values: {},
			profile_id: profileId,
			mode_id: modeId,
			profile_snapshot: profile,
		},
		universe: 1,
		address,
		layer_id: "default",
		direct_control: null,
		location: { x: fixtureNumber * 100, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		logical_heads: [],
		multipatch: [],
	};
}
