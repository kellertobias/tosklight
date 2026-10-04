import { unzipSync } from "fflate";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import { requireSemanticContract } from "./bench/core/semanticContract";
import { openPatch } from "./support/foundational/ui";
import {
	calibrationSection,
	chooseSelect,
	closeMappingAndMode,
	containedIn,
	editProfile,
	latestProfile,
	openFixtureLibrary,
	openZoomDetails,
	pageOverflows,
	saveNewRevision,
	saveProfile,
	saveShippedCopy,
	setField,
	zoomFunction,
	zoomProbeProfile,
} from "./bench/show-setup/fixtureMappingEditorScenario";
import {
	base64,
	deleteProfileRevision,
	exportMvr,
	exportPackage,
	gdtfDescription,
	gdtfMembers,
	gdtfProbeProfile,
	importGdtf,
	importPackage,
	librarySignature,
	mvrExportPreview,
	openFreshShow,
	packageProfile,
	patchProfile,
	previewGdtf,
	previewMvrImport,
	withExtraMember,
	withFixtureTypeId,
} from "./bench/show-setup/gdtfTransferScenario";
import { derivePrimarySlots } from "../apps/patch-library/src/sheet/fixtureProfileModel/channels";
import {
	arrangeFixtures,
	arrangeMovers,
	emittedSlot,
	libraryProfile,
	programZoom,
	MOVER,
	publishesSemanticFamily,
	RawJson,
	rawRequest,
	expectPanTilt,
	PAN_RANGE,
	patchFixture,
	positionCalibration,
	prepareInstalledUpdate,
	referencedMode,
	programAngles,
	publishesSemanticPosition,
	TILT_RANGE,
	u16For,
	updateInstalledFixture,
} from "./bench/show-setup/installedCalibrationScenario";

const MEASURED_SOURCE = "E2E bench goniometer, 2026-10 survey";
const SAMPLED_MAPPING = {
	quality: "measured",
	source: MEASURED_SOURCE,
	revision: 2,
	opening_convention: "beam",
	samples: [
		{ raw: 0, physical: 44 },
		{ raw: 127, physical: 30 },
		{ raw: 255, physical: 8 },
	],
};

/** A library profile imported from a GDTF archive, so it carries that archive as its source. */
async function importedSourceProfile(api: ApiDriver, label: string) {
	const origin = gdtfProbeProfile(`GDTF ${label} origin ${crypto.randomUUID().slice(0, 6)}`);
	await saveProfile(api, origin);
	const showId = await openFreshShow(api, `${label}-origin`);
	await patchProfile(api, showId, { id: origin.id, revision: 1, modeId: origin.modes[0].id, footprint: 3 }, 1);
	const [[, generated]] = gdtfMembers((await exportMvr(api, showId)).members);
	const targetId = crypto.randomUUID();
	const archive = withExtraMember(withFixtureTypeId(generated, targetId), "notes/source-only.txt", label);
	const imported = await importGdtf(api, targetId, archive);
	expect(imported.status, JSON.stringify(imported.body)).toBe(200);
	return { targetId, archive };
}

const ROOT_PAR_MODE = "D7CH — Delay Off, virtual dimmer";
const ZOOM_GATE = "semantic Zoom programming is not published by this runtime";
const GATE = "semantic Position programming is not published by this runtime";

test.describe("docs/testing/31-fixture-physical-mapping.md", () => {
	test("FIXTURE-MAPPING-001 @ui › author a sampled Zoom response curve and keep it through save and reopen", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		await page.setViewportSize({ width: 1496, height: 761 });
		const { profile, seeded } = zoomProbeProfile(`Zoom Curve ${crypto.randomUUID().slice(0, 8)}`);
		await saveProfile(api, profile);
		await openFixtureLibrary(page, desk, bench.baseUrl);
		let editor = await editProfile(page, seeded);
		let mapping = await openZoomDetails(page, editor);
		let calibration = calibrationSection(mapping);
		// Opening Details shows Unknown and a descending linear preview without attaching data.
		await expect(calibration.getByRole("button", { name: "Mapping quality" })).toContainText("Unknown");
		await expect(calibration.getByRole("img", { name: "Raw DMX to physical value curve" })).toBeVisible();
		await expect(calibration.getByText("Raw 0 → 44 degrees")).toBeVisible();
		await expect(calibration.getByText("Raw 255 → 8 degrees")).toBeVisible();
		const midpoint = calibration.getByLabel("Physical mapping midpoint");
		await expect(midpoint).toHaveText("Raw 127 → 26.0706 degrees");
		await expect(calibration.getByRole("button", { name: "Clear mapping calibration" })).toHaveCount(0);

		await calibration.getByRole("button", { name: "Use sampled mapping", exact: true }).click();
		await calibration.getByRole("button", { name: "Add intermediate sample", exact: true }).click();
		await setField(calibration, "Sample 2 physical", "30");
		await expect(midpoint).toHaveText("Raw 127 → 30 degrees");
		await expect(calibration.getByRole("textbox", { name: "Sample 1 physical" })).toHaveValue("44");
		await expect(calibration.getByRole("textbox", { name: "Sample 3 physical" })).toHaveValue("8");
		await page.screenshot({ path: test.info().outputPath("mapping-001-sampled.png") });

		// Measured needs its source: the error is shown and Save leaves the editor and library as they were.
		await chooseSelect(calibration, "Mapping quality", "Measured");
		await expect(calibration.getByRole("alert")).toContainText("Manufacturer and measured mappings need a source.");
		await closeMappingAndMode(page);
		await editor.getByRole("button", { name: "Save fixture", exact: true }).click();
		await expect(editor.getByText("Fixture profile needs attention")).toBeVisible();
		await expect(editor).toContainText("Default: Zoom: Manufacturer and measured mappings need a source.");
		expect((await latestProfile(api, seeded.id)).revision).toBe(1);

		mapping = await openZoomDetails(page, editor);
		calibration = calibrationSection(mapping);
		await calibration.getByPlaceholder("Manual, measurement or instrument").fill(MEASURED_SOURCE);
		await setField(calibration, "Mapping revision", "2");
		await chooseSelect(calibration, "Zoom opening convention", "Beam angle");
		await expect(calibration.getByRole("alert")).toHaveCount(0);
		await closeMappingAndMode(page);
		await saveNewRevision(page, editor);
		const saved = await latestProfile(api, seeded.id);
		expect(saved.revision).toBe(2);
		expect(zoomFunction(saved)).toMatchObject({
			dmx_from: 0,
			dmx_to: 255,
			behavior: { type: "continuous", physical_min: 44, physical_max: 8, unit: "degrees" },
			physical_mapping: SAMPLED_MAPPING,
		});

		// Reopen: every authored value is back in the editor.
		editor = await editProfile(page, seeded);
		mapping = await openZoomDetails(page, editor);
		calibration = calibrationSection(mapping);
		await expect(calibration.getByRole("button", { name: "Mapping quality" })).toContainText("Measured");
		await expect(calibration.getByPlaceholder("Manual, measurement or instrument")).toHaveValue(MEASURED_SOURCE);
		await expect(calibration.getByRole("textbox", { name: "Mapping revision" })).toHaveValue("2");
		await expect(calibration.getByRole("button", { name: "Zoom opening convention" })).toContainText("Beam angle");
		await expect(calibration.getByRole("textbox", { name: "Sample 2 raw" })).toHaveValue("127");
		await expect(calibration.getByRole("textbox", { name: "Sample 2 physical" })).toHaveValue("30");

		// Step 7: linear mapping drops only the samples; clearing removes the optional data only.
		await calibration.getByRole("button", { name: "Use linear mapping", exact: true }).click();
		await expect(calibration.getByRole("textbox", { name: "Sample 1 raw" })).toHaveCount(0);
		await expect(calibration.getByRole("button", { name: "Mapping quality" })).toContainText("Measured");
		await expect(calibration.getByPlaceholder("Manual, measurement or instrument")).toHaveValue(MEASURED_SOURCE);
		await calibration.getByRole("button", { name: "Clear mapping calibration", exact: true }).click();
		await expect(calibration.getByRole("button", { name: "Mapping quality" })).toContainText("Unknown");
		await expect(mapping.getByRole("textbox", { name: "Function physical minimum" })).toHaveValue("44");
		await expect(mapping.getByRole("textbox", { name: "Function physical maximum" })).toHaveValue("8");
		await closeMappingAndMode(page);
		await saveNewRevision(page, editor);
		const cleared = await latestProfile(api, seeded.id);
		expect(cleared.revision).toBe(3);
		expect(zoomFunction(cleared).physical_mapping ?? null).toBeNull();
		expect(zoomFunction(cleared)).toMatchObject({
			dmx_from: 0,
			dmx_to: 255,
			behavior: { physical_min: 44, physical_max: 8, unit: "degrees" },
		});
		expect(cleared.modes[0].channels[0]).toMatchObject({ default_raw: 0, highlight_raw: 0, resolution: "u8" });
	});

	test("FIXTURE-INSTALLATION-001 @api › saved Position calibration changes the emitted Pan/Tilt for the same programmed degrees", async ({
		api,
		bench,
	}) => {
		const { showId, fixtureIds } = await arrangeMovers(api, bench, "INSTALLATION-001 output");
		requireSemanticContract(await publishesSemanticPosition(api, fixtureIds), GATE);
		const [mover] = fixtureIds;
		await programAngles(api, showId, fixtureIds, 0, 0);
		await expectPanTilt(api, bench, 1, { pan: u16For(PAN_RANGE, 0), tilt: u16For(TILT_RANGE, 0) });

		const saved = await updateInstalledFixture(api, showId, mover, {
			action: "set_position_calibration",
			calibration: positionCalibration(30, -10),
		});
		expect(saved.status, JSON.stringify(saved.body)).toBe(200);
		// The Programmer still asks for 0°/0°; the installed zero offsets move the native words.
		await expectPanTilt(api, bench, 1, { pan: u16For(PAN_RANGE, -30), tilt: u16For(TILT_RANGE, 10) });
		const { fixture } = await patchFixture(api, showId, mover);
		expect(fixture.position_calibration).toMatchObject({ pan_zero_degrees: 30, tilt_zero_degrees: -10 });

		const cleared = await updateInstalledFixture(api, showId, mover, {
			action: "set_position_calibration",
			calibration: null,
		});
		expect(cleared.status).toBe(200);
		await expectPanTilt(api, bench, 1, { pan: u16For(PAN_RANGE, 0), tilt: u16For(TILT_RANGE, 0) });
	});

	test("FIXTURE-GDTF-001 @api › MVR export writes directed physical functions, fine slots and explicit gaps, and reports refused curves", async ({
		api,
	}) => {
		const profile = gdtfProbeProfile(`GDTF Export ${crypto.randomUUID().slice(0, 6)}`);
		await saveProfile(api, profile);
		const showId = await openFreshShow(api, "001");
		const mode = profile.modes[0];
		await patchProfile(api, showId, { id: profile.id, revision: 1, modeId: mode.id, footprint: 3 }, 1);
		const { members } = await exportMvr(api, showId);
		const [[gdtfName, archive]] = gdtfMembers(members);
		expect(gdtfName).toBe(`E2E Physical@${profile.name}.gdtf`);
		const xml = gdtfDescription(archive);
		expect(xml).toContain(`FixtureTypeID="${profile.id}"`);
		// U16 Pan keeps its separated fine slot, exact default, InitialFunction and directed endpoints.
		expect(xml).toMatch(/<DMXChannel DMXBreak="1" Offset="1,3"[^>]*InitialFunction="Body_Pan\.Pan\.Pan"/u);
		expect(xml).toContain('DMXFrom="0/2" Default="32769/2" PhysicalFrom="540" PhysicalTo="-540"');
		// Zoom keeps both function boundaries and directions; the raw gap is an explicit NoFeature.
		expect(xml).toMatch(/Name="Zoom" Attribute="Zoom"[^>]*DMXFrom="0\/1"[^>]*PhysicalFrom="4" PhysicalTo="50"/u);
		expect(xml).toMatch(/Attribute="NoFeature"[^>]*DMXFrom="100\/1"/u);
		expect(xml).toMatch(/Name="Zoom wide" Attribute="Zoom"[^>]*DMXFrom="150\/1"[^>]*PhysicalFrom="50" PhysicalTo="4"/u);

		// A nonlinear sampled curve cannot be flattened into GDTF: the export says so and why.
		const curved = gdtfProbeProfile(`GDTF Curve ${crypto.randomUUID().slice(0, 6)}`);
		curved.modes[0].channels[1].functions[0].physical_mapping = {
			quality: "estimated",
			source: "E2E nonlinear bench curve",
			revision: 1,
			samples: [
				{ raw: 0, physical: 4 },
				{ raw: 50, physical: 40 },
				{ raw: 99, physical: 50 },
			],
		};
		await saveProfile(api, curved);
		await patchProfile(api, showId, { id: curved.id, revision: 1, modeId: curved.modes[0].id, footprint: 3 }, 1, 2);
		const preview = await mvrExportPreview(api, showId);
		const label = `E2E Physical · ${curved.name}`;
		expect(preview.missing_profiles).toContain(label);
		const refusal = preview.warnings.find((warning) => warning.startsWith(`${label} (revision 1)`));
		expect(refusal, JSON.stringify(preview.warnings)).toMatch(/GDTF could not be generated: .+/u);
		const curvedExport = await exportMvr(api, showId);
		expect(gdtfMembers(curvedExport.members).map(([name]) => name)).toEqual([gdtfName]);
	});

	test("FIXTURE-GDTF-001 @api › an attached unchanged source is reused byte for byte; an edited profile exports its current data", async ({
		api,
	}) => {
		const profile = gdtfProbeProfile(`GDTF Source ${crypto.randomUUID().slice(0, 6)}`);
		await saveProfile(api, profile);
		const showId = await openFreshShow(api, "001-source");
		const mode = profile.modes[0];
		await patchProfile(api, showId, { id: profile.id, revision: 1, modeId: mode.id, footprint: 3 }, 1);
		const [[name, generated]] = gdtfMembers((await exportMvr(api, showId)).members);
		// Rich source data the generator cannot represent rides along in the original archive.
		const source = withExtraMember(generated, "notes/source-only.txt", "Original manufacturer archive member");
		await api.fixtureLibraryAction({
			type: "attach_gdtf",
			profile_id: profile.id,
			revision: 1,
			source_base64: base64(source),
		});
		const reused = gdtfMembers((await exportMvr(api, showId)).members);
		expect(reused.map(([member]) => member)).toEqual([name]);
		expect(Buffer.from(reused[0][1]).equals(Buffer.from(source))).toBe(true);

		// Edit the physical data: the export must describe revision 2, not the retained original.
		const edited = structuredClone(profile);
		edited.revision = 2;
		edited.modes[0].channels[1].functions[0].behavior.physical_max = 45;
		await saveProfile(api, edited, 1);
		const editedShow = await openFreshShow(api, "001-edited");
		await patchProfile(api, editedShow, { id: profile.id, revision: 2, modeId: mode.id, footprint: 3 }, 1);
		const [[, current]] = gdtfMembers((await exportMvr(api, editedShow)).members);
		expect(Buffer.from(current).equals(Buffer.from(source))).toBe(false);
		expect(gdtfDescription(current)).toContain("from fixture profile revision 2");
		expect(gdtfDescription(current)).toMatch(/Name="Zoom" Attribute="Zoom"[^>]*PhysicalFrom="4" PhysicalTo="45"/u);
		expect((await mvrExportPreview(api, editedShow)).warnings.join("\n")).toContain("generated GDTF files from the current fixture profiles");
	});

	test("FIXTURE-GDTF-002 @api › GDTF preview publishes nothing; one confirmed import keeps exact raw values and retries idempotently", async ({
		api,
	}) => {
		const origin = gdtfProbeProfile(`GDTF Origin ${crypto.randomUUID().slice(0, 6)}`);
		await saveProfile(api, origin);
		const showId = await openFreshShow(api, "002");
		await patchProfile(api, showId, { id: origin.id, revision: 1, modeId: origin.modes[0].id, footprint: 3 }, 1);
		const [[, generated]] = gdtfMembers((await exportMvr(api, showId)).members);
		const targetId = crypto.randomUUID();
		const archive = withFixtureTypeId(generated, targetId);

		const before = await librarySignature(api);
		const preview = await previewGdtf(api, archive);
		expect(preview.status, JSON.stringify(preview.body)).toBe(200);
		expect(await librarySignature(api)).toEqual(before);
		expect(preview.body.unknown_attributes).toEqual([]);
		// Source-only geometry is disclosed even though every attribute maps.
		expect(preview.body.diagnostics.map((item: { node: string }) => item.node)).toEqual(
			expect.arrayContaining(["Geometries"]),
		);
		const [pan, zoom] = preview.body.profile.modes[0].channels;
		expect(pan).toMatchObject({ attribute: "pan", resolution: "u16", secondary_slots: [3], default_raw: 32769 });
		expect(pan.functions[0].behavior).toMatchObject({ physical_min: 540, physical_max: -540, unit: "degrees" });
		expect(zoom).toMatchObject({ attribute: "zoom", resolution: "u8", secondary_slots: [] });
		expect(zoom.functions[0]).toMatchObject({ dmx_from: 0, dmx_to: 99, behavior: { physical_min: 4, physical_max: 50 } });

		// Confirm once, twice in flight with the same request: exactly one revision is published.
		const requestId = crypto.randomUUID();
		const [first, retry] = await Promise.all([
			importGdtf(api, targetId, archive, { requestId }),
			importGdtf(api, targetId, archive, { requestId }),
		]);
		expect([first.status, retry.status]).toEqual([200, 200]);
		expect(first.body.result).toEqual(retry.body.result);
		expect([first.body.replayed, retry.body.replayed].sort()).toEqual([false, true]);
		const revisions = await api.fixtureProfileRevisions<Record<string, any>>(targetId);
		expect(revisions.map((revision) => revision.revision)).toEqual([1]);
		expect(revisions[0].modes[0].channels[0]).toMatchObject({ secondary_slots: [3], default_raw: 32769 });
		const fingerprint = base64(archive).slice(0, 120);
		expect(JSON.stringify(revisions[0].source_gdtf ?? null).includes(fingerprint)).toBe(true);

		// The original archive travels in the package and survives import into a library without it.
		const transferable = await exportPackage(api, targetId, 1);
		expect(Buffer.from(unzipSync(transferable)["assets/source.gdtf"]).equals(Buffer.from(archive))).toBe(true);
		await deleteProfileRevision(api, targetId, 1);
		expect(await api.fixtureProfileRevisions(targetId)).toEqual([]);
		await importPackage(api, transferable);
		const restored = await api.fixtureProfileRevisions<Record<string, any>>(targetId);
		expect(restored.map((revision) => revision.revision)).toEqual([1]);
		expect(JSON.stringify(restored[0].source_gdtf ?? null).includes(fingerprint)).toBe(true);
		expect(restored[0].modes).toEqual(revisions[0].modes);

		// A reused request ID with a changed payload, or a stale expected revision, is refused.
		const changed = await importGdtf(api, targetId, archive, {
			requestId,
			attributeMappings: [{ source_attribute: "Zoom", target_attribute: "focus" }],
		});
		expect(changed.status).toBeGreaterThanOrEqual(400);
		const stale = await importGdtf(api, targetId, archive, { expectedRevision: 0 });
		expect(stale.status).toBe(409);
		expect((await api.fixtureProfileRevisions(targetId)).length).toBe(1);
	});

	test("FIXTURE-GDTF-003 @api › 300 fixtures sharing a source-bearing profile carry its original archive once", async ({
		api,
	}) => {
		const { targetId, archive } = await importedSourceProfile(api, "003");
		const imported = (await api.fixtureProfileRevisions<Record<string, any>>(targetId))[0];
		const showId = await openFreshShow(api, "003");
		await patchProfile(api, showId, { id: targetId, revision: 1, modeId: imported.modes[0].id, footprint: 3 }, 300);
		const { members } = await exportMvr(api, showId);
		const archives = gdtfMembers(members);
		expect(archives).toHaveLength(1);
		expect(Buffer.from(archives[0][1]).equals(Buffer.from(archive))).toBe(true);
		// Native fixture metadata references the profile; it never repeats the archive.
		const fingerprint = base64(archive).slice(0, 120);
		const metadata = Buffer.from(members["tosklight/fixture-metadata.json"]).toString("utf8");
		expect(JSON.parse(metadata).fixtures).toHaveLength(300);
		expect(metadata.includes(fingerprint)).toBe(false);
		// The Patch snapshot shares one retained profile revision; 300 fixtures never repeat the bytes.
		const patch = JSON.stringify(await api.request("GET", "/api/v2/patch"));
		expect(patch.split(fingerprint).length - 1).toBeLessThanOrEqual(1);
		// The authoritative editable revision keeps the original bytes.
		expect(JSON.stringify(imported.source_gdtf ?? null).includes(fingerprint)).toBe(true);
	});

	test("FIXTURE-GDTF-004 @api › MVR import preview binds each fixture to its embedded source without writing the library", async ({
		api,
	}) => {
		const { targetId, archive } = await importedSourceProfile(api, "004");
		const imported = (await api.fixtureProfileRevisions<Record<string, any>>(targetId))[0];
		const showId = await openFreshShow(api, "004");
		await patchProfile(api, showId, { id: targetId, revision: 1, modeId: imported.modes[0].id, footprint: 3 }, 2);
		const { bytes } = await exportMvr(api, showId);
		const before = await librarySignature(api);
		const preview = await previewMvrImport(api, bytes);
		expect(preview.status, JSON.stringify(preview.body)).toBe(200);
		expect(preview.body.token).toEqual(expect.any(String));
		expect(preview.body.missing_profiles).toEqual([]);
		expect(preview.body.fixtures).toHaveLength(2);
		for (const fixture of preview.body.fixtures) expect(fixture).toMatchObject({ gdtf_mode: "Precise", matched: true });
		expect(await librarySignature(api)).toEqual(before);
		expect(archive.byteLength).toBeGreaterThan(0);
	});

	test("FIXTURE-MAPPING-001 @api › a sampled Zoom curve survives .toskfixture export and import unchanged", async ({
		api,
	}) => {
		const { profile, seeded } = zoomProbeProfile(`Zoom Package ${crypto.randomUUID().slice(0, 8)}`, {
			physicalMapping: SAMPLED_MAPPING,
		});
		await saveProfile(api, profile);
		const saved = await latestProfile(api, seeded.id);
		const archive = await exportPackage(api, seeded.id, 1);
		expect(zoomFunction(packageProfile(archive))).toMatchObject({ physical_mapping: SAMPLED_MAPPING });
		await deleteProfileRevision(api, seeded.id, 1);
		await importPackage(api, archive);
		const restored = await latestProfile(api, seeded.id);
		expect(restored).toEqual(saved);
		expect(restored.modes[0].channels[0].resolution).toBe("u8");
		expect(zoomFunction(restored).physical_mapping).toEqual(SAMPLED_MAPPING);
	});

	test("FIXTURE-MAPPING-001 @api › a sampled Zoom curve changes the native Zoom byte fitted for the same programmed degrees", async ({
		api,
		bench,
	}) => {
		const stock = await libraryProfile(api, "Cameo", "AURO SPOT Z300", "20-Channel", 20);
		const zoomChannel = (profile: Record<string, any>) =>
			profile.modes
				.find((mode: { id: string }) => mode.id === stock.modeId)
				.channels.find((channel: { attribute: string }) => channel.attribute === "zoom");
		const curved = await saveShippedCopy(api, stock.profile, `AURO curve ${crypto.randomUUID().slice(0, 6)}`, (copy) => {
			// The stock function is linear 10°→25° over raw 0–255; this curve reaches 22° at raw 128.
			zoomChannel(copy).functions[0].physical_mapping = {
				...zoomChannel(copy).functions[0].physical_mapping,
				quality: "estimated",
				source: "E2E zoom bench curve",
				samples: [
					{ raw: 0, physical: 10 },
					{ raw: 128, physical: 22 },
					{ raw: 255, physical: 25 },
				],
			};
		});
		const mode = stock.profile.modes.find((candidate: { id: string }) => candidate.id === stock.modeId);
		const zoomSlot = derivePrimarySlots(mode).slots.get(zoomChannel(stock.profile).id) ?? 0;
		const { showId, fixtureIds, addresses } = await arrangeFixtures(api, bench, "MAPPING-001 output", [
			stock,
			{ ...stock, id: curved.id, revision: 1 },
		]);
		requireSemanticContract(await publishesSemanticFamily(api, fixtureIds, "focus"), ZOOM_GATE);
		await programZoom(api, showId, fixtureIds, 22);
		await expect
			.poll(async () => {
				await bench.tick(25);
				return Promise.all(addresses.map((address) => emittedSlot(api, address + zoomSlot - 1)));
			})
			// Linear: 12/15 of 255 = 204. Sampled: exactly the authored knot.
			.toEqual([204, 128]);
	});

	test("FIXTURE-MAPPING-002 @api › the library refuses invalid sample curves without publishing a revision", async ({
		api,
	}) => {
		const base = { quality: "estimated", source: "E2E", revision: 1 };
		const curve = (...samples: Array<[number, number]>) => ({
			...base,
			samples: samples.map(([raw, physical]) => ({ raw, physical })),
		});
		const cases: Array<[string, unknown, RegExp, string?]> = [
			["duplicate raw knot", curve([0, 44], [127, 30], [127, 20], [255, 8]), /increasing raw values/u],
			["outside the endpoint direction", curve([0, 44], [127, 50], [255, 8]), /strictly monotonic/u],
			["f32-collapsed physical knot", curve([0, 44], [127, 44.0000001], [255, 8]), /strictly monotonic/u],
			["endpoint mismatch", curve([0, 40], [255, 8]), /exact raw and physical function endpoints/u],
			["out-of-resolution raw", curve([0, 44], [300, 8]), /function endpoints/u],
			["incomplete samples", curve([0, 44]), /function endpoints/u],
			["noninteger raw knot", curve([0, 44], [12.5, 40], [255, 8]), /expected u32/u],
			["measured without a source", { quality: "measured", revision: 1 }, /require a source/u],
			["Beam convention on percent Zoom", { ...base, opening_convention: "beam" }, /explicit degree units/u, "percent"],
		];
		for (const [label, mapping, reason, unit] of cases) {
			const { profile, seeded } = zoomProbeProfile(`Invalid ${crypto.randomUUID().slice(0, 8)}`, {
				physicalMapping: mapping,
			});
			if (unit) {
				const channel = profile.modes[0].channels[0];
				channel.unit = unit;
				channel.functions[0].behavior.unit = unit;
			}
			const response = await rawRequest(api, "POST", "/api/v2/fixture-library", {
				request_id: crypto.randomUUID(),
				action: { type: "save_profile", profile, expected_revision: 0 },
			});
			expect(response.status, label).toBe(400);
			expect(JSON.stringify(response.body), label).toMatch(reason);
			expect(await api.fixtureProfileRevisions(seeded.id), label).toEqual([]);
		}
	});

	test("FIXTURE-MAPPING-003 @api › shipped profiles load without invented measured physical data", async ({ api }) => {
		const { profiles } = await api.fixtureProfilesSnapshot();
		expect(profiles.length).toBeGreaterThan(0);
		const unsupported: string[] = [];
		for (const profile of profiles as Array<Record<string, any>>) {
			expect(profile.revision, profile.name).toBe(1);
			for (const mode of profile.modes)
				for (const channel of mode.channels)
					for (const fn of channel.functions) {
						const mapping = fn.physical_mapping;
						if (!mapping) continue;
						if (["measured", "manufacturer"].includes(mapping.quality) && !mapping.source?.trim())
							unsupported.push(`${profile.manufacturer} ${profile.name} ${mode.name} ${fn.name}`);
					}
		}
		expect(unsupported).toEqual([]);
	});

	test("FIXTURE-INSTALLATION-001 @api › root and copy keep independent Position calibration through unrelated edits and a portable round trip", async ({
		api,
		bench,
	}) => {
		const mover = await libraryProfile(api, MOVER.manufacturer, MOVER.profile, MOVER.mode, MOVER.footprint);
		const { showId, fixtureIds, addresses, copyIds, copyAddresses } = await arrangeFixtures(
			api,
			bench,
			"INSTALLATION-001 copies",
			[{ ...mover, copies: 1 }],
		);
		const [root] = fixtureIds;
		const [copy] = copyIds[0];
		const rootCalibration = positionCalibration(30, -10);
		const copyCalibration = positionCalibration(-5, 2, { quality: "manufacturer", source: "Mounting sheet", revision: 3 });
		for (const [target, calibration] of [
			[null, rootCalibration],
			[copy, copyCalibration],
		] as const) {
			const saved = await updateInstalledFixture(api, showId, root, {
				action: "set_position_calibration",
				calibration,
			}, target);
			expect(saved.status, JSON.stringify(saved.body)).toBe(200);
		}
		const calibrations = async () => {
			const { fixture } = await patchFixture(api, showId, root);
			return {
				root: fixture.position_calibration ?? null,
				copy: fixture.multipatch.find((candidate: { id: string }) => candidate.id === copy)?.position_calibration ?? null,
			};
		};
		expect(await calibrations()).toEqual({ root: rootCalibration, copy: copyCalibration });

		// Unrelated installed edits keep both calibrations exactly.
		for (const [target, action] of [
			[null, { action: "set_pan_tilt", invert_pan: true, invert_tilt: false }],
			[copy, { action: "set_location_axis", axis: "y", millimetres: 1500 }],
			[null, { action: "set_location_axis", axis: "x", millimetres: 2500 }],
		] as const) {
			const edited = await updateInstalledFixture(api, showId, root, action, target);
			expect(edited.status, JSON.stringify(edited.body)).toBe(200);
		}
		expect(await calibrations()).toEqual({ root: rootCalibration, copy: copyCalibration });

		// Invalid evidence and nonfinite values are refused without touching the saved values.
		const refusals: Array<[string, Record<string, unknown>, ((body: Record<string, unknown>) => unknown)?]> = [
			["measured without a source", { calibration: positionCalibration(1, 1, { quality: "measured", source: null }) }],
			["fractional revision", { calibration: positionCalibration(1, 1, { revision: 1.5 }) }],
			[
				"nonfinite offset",
				{ calibration: positionCalibration(1, 1) },
				(body) => new RawJson(JSON.stringify(body).replace('"pan_zero_degrees":1', '"pan_zero_degrees":1e999')),
			],
		];
		for (const [label, payload, encode] of refusals) {
			const refused = await updateInstalledFixture(
				api,
				showId,
				root,
				{ action: "set_position_calibration", ...payload },
				null,
				crypto.randomUUID(),
				encode,
			);
			expect(refused.status, label).toBeGreaterThanOrEqual(400);
			expect(refused.status, label).toBeLessThan(500);
		}
		expect(await calibrations()).toEqual({ root: rootCalibration, copy: copyCalibration });

		// The same programmed degrees reach each instance through its own calibration; the root's
		// inversion mirrors only its wire word and never stacks with the copy.
		await programAngles(api, showId, fixtureIds, 0, 0);
		await expectPanTilt(api, bench, addresses[0], {
			pan: 65535 - u16For(PAN_RANGE, -30),
			tilt: u16For(TILT_RANGE, 10),
		});
		await expectPanTilt(api, bench, copyAddresses[0][0], {
			pan: u16For(PAN_RANGE, 5),
			tilt: u16For(TILT_RANGE, -2),
		});

		// Clearing the copy saves absence for that instance only.
		const cleared = await updateInstalledFixture(api, showId, root, {
			action: "set_position_calibration",
			calibration: null,
		}, copy);
		expect(cleared.status).toBe(200);
		expect(await calibrations()).toEqual({ root: rootCalibration, copy: null });

		// Portable show round trip.
		const portable = await api.downloadShow(showId);
		const reloaded = await api.createShow<{ id: string }>({
			name: `INSTALLATION-001 reload ${crypto.randomUUID()}`,
			data_base64: portable.toString("base64"),
		});
		await api.openShow(reloaded.id, { transition: "hold_current" });
		const { fixture } = await patchFixture(api, reloaded.id, root);
		expect(fixture.position_calibration).toEqual(rootCalibration);
		expect(fixture.multipatch[0].position_calibration ?? null).toBeNull();
	});

	test("FIXTURE-INSTALLATION-001 @api › a bracket angle without an authored hinge never silently parks programmed Position at the profile default", async ({
		api,
		bench,
	}) => {
		test.fail(
			true,
			"BUG: Patch accepts a nonzero bracket_angle on a profile whose Position contract has no Hinge bracket, after which that instance emits the profile default Pan/Tilt instead of the programmed Angles, without any error",
		);
		const { showId, fixtureIds } = await arrangeMovers(api, bench, "INSTALLATION-001 bracket");
		requireSemanticContract(await publishesSemanticPosition(api, fixtureIds), GATE);
		const [mover] = fixtureIds;
		const bracket = await updateInstalledFixture(api, showId, mover, { action: "set_bracket_angle", degrees: 15 });
		await programAngles(api, showId, fixtureIds, 30, 20);
		// Either the unsupported bracket is refused, or the programmed Angles still reach the wire.
		if (bracket.status === 200)
			await expectPanTilt(api, bench, 1, { pan: u16For(PAN_RANGE, 30), tilt: u16For(TILT_RANGE, 20) });
		else expect(bracket.status).toBeLessThan(500);
	});

	test("FIXTURE-INSTALLATION-002 @api › root and copy keep independent installed Color observations, including a zero gain", async ({
		api,
		bench,
	}) => {
		const par = await libraryProfile(api, "Cameo", "ROOT PAR 6", ROOT_PAR_MODE, 7);
		const { showId, fixtureIds, copyIds } = await arrangeFixtures(api, bench, "INSTALLATION-002", [{ ...par, copies: 1 }]);
		const [root] = fixtureIds;
		const [copy] = copyIds[0];
		const identity = (await referencedMode(api, showId, par)).native_color_identities?.[0];
		expect(identity).toMatchObject({ profile_id: par.id, mode_id: par.modeId });
		const emitters = par.profile.modes[0].color_physical.paths[0].source.emitters as Array<{ id: string; name: string }>;
		const red = emitters.find((emitter) => emitter.name === "Red")!.id;
		const observation = (gain: number, quality = "estimated", source: string | null = "E2E spot meter") => ({
			version: 1,
			revision: 1,
			paths: [
				{
					source_identity: identity,
					emitters: [{ emitter_id: red, output_gain: gain, provenance: { quality, source, revision: 1 } }],
					measurements: [],
				},
			],
		});
		const setColor = (calibration: unknown, target: string | null) =>
			updateInstalledFixture(api, showId, root, { action: "set_color_calibration", calibration }, target);
		const observations = async () => {
			const { fixture } = await patchFixture(api, showId, root);
			return {
				root: fixture.color_calibration ?? null,
				copy: fixture.multipatch[0].color_calibration ?? null,
			};
		};

		const sendRoot = await prepareInstalledUpdate(api, showId, root, {
			action: "set_color_calibration",
			calibration: observation(0),
		});
		const first = await sendRoot();
		expect(first.status, JSON.stringify(first.body)).toBe(200);
		const afterFirst = (await patchFixture(api, showId, root)).snapshot.patch_revision;
		// The same request identity replays exactly one sparse update.
		const replay = await sendRoot();
		expect(replay.status, JSON.stringify(replay.body)).toBe(200);
		expect((await patchFixture(api, showId, root)).snapshot.patch_revision).toBe(afterFirst);
		expect((await setColor(observation(0.5), copy)).status).toBe(200);
		const expected = { root: observation(0), copy: observation(0.5) };
		expect(await observations()).toEqual(expected);

		// Unrelated installed edits keep both observations; the library profile is untouched.
		for (const [target, action] of [
			[null, { action: "set_location_axis", axis: "z", millimetres: 4000 }],
			[copy, { action: "set_rotation_axis", axis: "z", degrees: 90 }],
		] as const)
			expect((await updateInstalledFixture(api, showId, root, action, target)).status).toBe(200);
		expect(await observations()).toEqual(expected);
		expect((await api.fixtureProfileRevisions(par.id)).length).toBe(1);

		// Negative output, missing evidence and duplicate emitters are refused without a partial write.
		const duplicate = observation(0.2);
		duplicate.paths[0].emitters.push({ ...duplicate.paths[0].emitters[0] });
		for (const [label, calibration] of [
			["negative output", observation(-0.1)],
			["measured without a source", observation(0.8, "measured", null)],
			["duplicate emitter", duplicate],
		] as const) {
			const refused = await setColor(calibration, null);
			expect(refused.status, label).toBeGreaterThanOrEqual(400);
			expect(refused.status, label).toBeLessThan(500);
		}
		expect(await observations()).toEqual(expected);

		// Clearing the root leaves the copy's observation as it was.
		expect((await setColor(null, null)).status).toBe(200);
		expect(await observations()).toEqual({ root: null, copy: observation(0.5) });
	});

	test("FIXTURE-OPTICAL-UV-001 @api › ROOT PAR 6 keeps a usable UV control with unknown visible appearance", async ({
		api,
		bench,
	}) => {
		const par = await libraryProfile(api, "Cameo", "ROOT PAR 6", ROOT_PAR_MODE, 7);
		const mode = par.profile.modes.find((candidate: { id: string }) => candidate.id === par.modeId);
		const uvChannel = mode.channels.find((channel: { attribute: string }) => channel.attribute === "color.uv");
		expect(uvChannel).toBeDefined();
		const [path] = mode.color_physical.paths;
		expect(path.controls).toContain(uvChannel.id);
		const uv = path.source.emitters.find((emitter: { binding: { channel_id: string } }) => emitter.binding.channel_id === uvChannel.id);
		// The UV purpose is explicit; its visible appearance and spectrum stay unknown, not black.
		expect(uv).toMatchObject({ band: "ultraviolet", spectrum: [], provenance: { quality: "unknown" } });
		expect(uv.xyz ?? null).toBeNull();
		expect(uv.binding.function_id).toBe(uvChannel.functions[0].id);
		// The unmeasured UV emitter does not make the native Color control unsupported.
		const { showId, fixtureIds } = await arrangeFixtures(api, bench, "OPTICAL-UV-001", [par]);
		const projected = await referencedMode(api, showId, par);
		expect(projected.native_color_identities).toEqual([
			expect.objectContaining({ head_id: path.head_id, path_id: path.id }),
		]);
	});

	test("FIXTURE-PHYSICAL-MOTION-001 @api › per-axis overrides on root and copy replace the family pair once and survive a portable round trip", async ({
		api,
		bench,
	}) => {
		const mover = await libraryProfile(api, MOVER.manufacturer, MOVER.profile, MOVER.mode, MOVER.footprint);
		const { showId, fixtureIds, addresses, copyIds, copyAddresses } = await arrangeFixtures(
			api,
			bench,
			"PHYSICAL-MOTION-001",
			[{ ...mover, copies: 1 }],
		);
		requireSemanticContract(await publishesSemanticPosition(api, fixtureIds), GATE);
		const [root] = fixtureIds;
		const [copy] = copyIds[0];
		const identity = (await referencedMode(api, showId, mover)).position_calibration_identity;
		expect(identity).toMatchObject({ profile_id: mover.id, mode_id: mover.modeId });
		const bindings = mover.profile.modes.find((mode: { id: string }) => mode.id === mover.modeId).position_physical
			.bindings as Array<{ node_id: string; role: string }>;
		const node = (role: string) => bindings.find((binding) => binding.role === role)!.node_id;
		const overrides = (pan: [number, boolean], tilt: [number, boolean]) => ({
			version: 1,
			source_identity: identity,
			axes: [
				{ node_id: node("pan"), zero_degrees: pan[0], invert: pan[1] },
				{ node_id: node("tilt"), zero_degrees: tilt[0], invert: tilt[1] },
			],
		});
		// The root's family calibration and inversion would add 30° and a second mirror if stacked.
		expect((await updateInstalledFixture(api, showId, root, { action: "set_pan_tilt", invert_pan: true, invert_tilt: false })).status).toBe(200);
		const rootCalibration = positionCalibration(30, 0, { axis_overrides: overrides([20, true], [0, false]) });
		const copyCalibration = positionCalibration(0, 0, { axis_overrides: overrides([-10, false], [5, false]) });
		for (const [target, calibration] of [
			[null, rootCalibration],
			[copy, copyCalibration],
		] as const) {
			const saved = await updateInstalledFixture(api, showId, root, { action: "set_position_calibration", calibration }, target);
			expect(saved.status, JSON.stringify(saved.body)).toBe(200);
		}
		await programAngles(api, showId, fixtureIds, 0, 0);
		await expectPanTilt(api, bench, addresses[0], {
			pan: 65535 - u16For(PAN_RANGE, -20),
			tilt: u16For(TILT_RANGE, 0),
		});
		await expectPanTilt(api, bench, copyAddresses[0][0], {
			pan: u16For(PAN_RANGE, 10),
			tilt: u16For(TILT_RANGE, -5),
		});

		const portable = await api.downloadShow(showId);
		const reloaded = await api.createShow<{ id: string }>({
			name: `PHYSICAL-MOTION-001 reload ${crypto.randomUUID()}`,
			data_base64: portable.toString("base64"),
		});
		await api.openShow(reloaded.id, { transition: "hold_current" });
		const { fixture } = await patchFixture(api, reloaded.id, root);
		expect(fixture.position_calibration).toEqual(rootCalibration);
		expect(fixture.multipatch[0].position_calibration).toEqual(copyCalibration);
	});

	test("FIXTURE-MAPPING-002 @ui › an invalid sample stays visible and blocks save; an endpoint edit keeps the sample until repaired explicitly", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		await page.setViewportSize({ width: 1496, height: 761 });
		const { profile, seeded } = zoomProbeProfile(`Zoom Invalid ${crypto.randomUUID().slice(0, 8)}`);
		await saveProfile(api, profile);
		await openFixtureLibrary(page, desk, bench.baseUrl);
		const editor = await editProfile(page, seeded);
		const mapping = await openZoomDetails(page, editor);
		const calibration = calibrationSection(mapping);
		await calibration.getByRole("button", { name: "Use sampled mapping", exact: true }).click();
		await calibration.getByRole("button", { name: "Add intermediate sample", exact: true }).click();

		// Outside the descending endpoint direction: precise error, no curve, the value stays.
		await setField(calibration, "Sample 2 physical", "50");
		await expect(calibration.getByRole("alert")).toContainText(
			"Sample physical values must be strictly monotonic in the endpoint direction.",
		);
		await expect(calibration.getByRole("img", { name: "Raw DMX to physical value curve" })).toHaveCount(0);
		await expect(calibration.getByRole("textbox", { name: "Sample 2 physical" })).toHaveValue("50");
		await expect(calibration.getByRole("button", { name: "Add intermediate sample", exact: true })).toBeDisabled();
		await closeMappingAndMode(page);
		await editor.getByRole("button", { name: "Save fixture", exact: true }).click();
		await expect(editor.getByText("Fixture profile needs attention")).toBeVisible();
		expect((await latestProfile(api, seeded.id)).revision).toBe(1);

		// Repair it, then move a function endpoint: the endpoint sample is kept and reported.
		const reopened = await openZoomDetails(page, editor);
		const repaired = calibrationSection(reopened);
		await expect(repaired.getByRole("textbox", { name: "Sample 2 physical" })).toHaveValue("50");
		await setField(repaired, "Sample 2 physical", "30");
		await expect(repaired.getByRole("alert")).toHaveCount(0);
		await setField(reopened, "Function physical minimum", "40");
		await expect(repaired.getByRole("alert")).toContainText(
			"First and last samples must match the function's raw and physical endpoints.",
		);
		await expect(repaired.getByRole("textbox", { name: "Sample 1 physical" })).toHaveValue("44");
		await repaired.getByRole("button", { name: "Use function endpoints", exact: true }).click();
		await expect(repaired.getByRole("textbox", { name: "Sample 1 physical" })).toHaveValue("40");
		await expect(repaired.getByRole("textbox", { name: "Sample 2 physical" })).toHaveValue("30");
		await expect(repaired.getByRole("alert")).toHaveCount(0);
		await closeMappingAndMode(page);
		await saveNewRevision(page, editor);
		expect(zoomFunction(await latestProfile(api, seeded.id))).toMatchObject({
			behavior: { physical_min: 40, physical_max: 8 },
			physical_mapping: {
				samples: [
					{ raw: 0, physical: 40 },
					{ raw: 127, physical: 30 },
					{ raw: 255, physical: 8 },
				],
			},
		});
	});

	for (const viewport of [
		{ width: 1496, height: 761 },
		{ width: 1024, height: 768 },
	])
		test(`FIXTURE-MAPPING-003 @ui › the Mapping details stay reachable inside the modal at ${viewport.width}×${viewport.height}`, async ({
			api,
			bench,
			desk,
			page,
		}) => {
			await page.setViewportSize(viewport);
			const { profile, seeded } = zoomProbeProfile(`Zoom Layout ${crypto.randomUUID().slice(0, 8)}`, {
				physicalMapping: SAMPLED_MAPPING,
			});
			await saveProfile(api, profile);
			await openFixtureLibrary(page, desk, bench.baseUrl);
			const editor = await editProfile(page, seeded);
			const mapping = await openZoomDetails(page, editor);
			const calibration = calibrationSection(mapping);
			// The library refuses invalid curves, so the visible error is entered in the editor.
			await setField(calibration, "Sample 2 physical", "50");
			expect(await pageOverflows(page)).toBe(false);
			const mappingBox = await mapping.boundingBox();
			expect(mappingBox && mappingBox.y >= 0 && mappingBox.y + mappingBox.height <= viewport.height + 1).toBe(true);
			// The function area scrolls inside the modal: every field, error and action can be reached.
			for (const [label, target] of [
				["quality", calibration.getByRole("button", { name: "Mapping quality" })],
				["source", calibration.getByPlaceholder("Manual, measurement or instrument")],
				["sample", calibration.getByRole("textbox", { name: "Sample 3 physical" })],
				[
					"sample keypad",
					calibration.locator(".fixture-physical-mapping-sample").nth(2).getByRole("button", { name: "Open number pad" }).last(),
				],
				["linear", calibration.getByRole("button", { name: "Use linear mapping", exact: true })],
				["clear", calibration.getByRole("button", { name: "Clear mapping calibration", exact: true })],
			] as const) {
				await target.scrollIntoViewIfNeeded();
				await expect(target, label).toBeInViewport();
				expect(await containedIn(target, mapping), label).toBe(true);
			}
			// The error list spans the scrolling function row; its message itself is on screen.
			const error = calibration.getByText("Sample physical values must be strictly monotonic in the endpoint direction.");
			await error.scrollIntoViewIfNeeded();
			await expect(error).toBeInViewport();
			expect(await pageOverflows(page)).toBe(false);
			await page.screenshot({ path: test.info().outputPath(`mapping-details-${viewport.width}x${viewport.height}.png`) });
		});

	test("FIXTURE-OPTICS-001 @ui › opening a mode's Color tab creates no optical path data", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		await page.setViewportSize({ width: 1496, height: 761 });
		const { profile, seeded } = zoomProbeProfile(`Optics Probe ${crypto.randomUUID().slice(0, 8)}`);
		await saveProfile(api, profile);
		const before = await latestProfile(api, seeded.id);
		await openFixtureLibrary(page, desk, bench.baseUrl);
		const editor = await editProfile(page, seeded);
		await editor.getByRole("tab", { name: "Modes", exact: true }).click();
		await page.getByRole("button", { name: "Edit channels for Default", exact: true }).click();
		const mode = page.getByRole("dialog", { name: "Edit Default mode" });
		await mode.getByRole("tab", { name: "Color", exact: true }).click();
		await expect(mode.getByRole("tab", { name: "Color", exact: true })).toHaveAttribute("aria-selected", "true");
		await mode.getByRole("button", { name: "Close mode editor", exact: true }).click();
		await saveNewRevision(page, editor);
		const after = await latestProfile(api, seeded.id);
		expect(after.revision).toBe(2);
		expect(after.modes[0].color_physical ?? null).toBeNull();
		expect(after.modes).toEqual(before.modes);
	});

	test("FIXTURE-INSTALLATION-001 @ui › Pan / Tilt → Position calibration saves root and copy values independently", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const mover = await libraryProfile(api, MOVER.manufacturer, MOVER.profile, MOVER.mode, MOVER.footprint);
		const { showId, fixtureIds } = await arrangeFixtures(api, bench, "INSTALLATION-001 ui", [{ ...mover, copies: 1 }]);
		const [root] = fixtureIds;
		await desk.open(bench.baseUrl);
		await openPatch(page);
		await page.getByRole("button", { name: "SET", exact: true }).click();
		await page.getByRole("button", { name: "Pan and Tilt 1", exact: true }).click();
		await page.getByRole("button", { name: "Position calibration…", exact: true }).click();
		let dialog = page.getByRole("dialog", { name: "Position calibration 1" });
		await expect(dialog).toBeVisible();
		await setField(dialog, "Pan zero offset (°)", "12.5");
		await setField(dialog, "Tilt zero offset (°)", "-4");
		await chooseSelect(dialog, "Calibration quality", "Measured");
		// Measured needs evidence: the draft stays open with a visible error and Save disabled.
		await expect(dialog.getByRole("alert")).toBeVisible();
		await expect(dialog.getByRole("button", { name: "Save", exact: true })).toBeDisabled();
		await chooseSelect(dialog, "Calibration quality", "Estimated");
		await setField(dialog, "Calibration source", "Laser level survey");
		await setField(dialog, "Calibration revision", "2");
		await dialog.getByRole("button", { name: "Save", exact: true }).click();
		await expect(dialog).toBeHidden();
		await expect
			.poll(async () => (await patchFixture(api, showId, root)).fixture.position_calibration ?? null)
			.toMatchObject({ pan_zero_degrees: 12.5, tilt_zero_degrees: -4, quality: "estimated", source: "Laser level survey", revision: 2 });

		// The copy gets its own values; the root keeps its own.
		await page.keyboard.press("Escape");
		await page.getByRole("button", { name: "SET", exact: true }).click();
		await page.getByRole("button", { name: "Pan and Tilt Fixture 1 copy 1", exact: true }).click();
		await page.getByRole("button", { name: "Position calibration…", exact: true }).click();
		dialog = page.getByRole("dialog", { name: "Position calibration Fixture 1 copy 1" });
		await expect(dialog).toBeVisible();
		await expect(dialog.getByRole("textbox", { name: "Pan zero offset (°)" })).toHaveValue("0");
		await setField(dialog, "Pan zero offset (°)", "-7");
		await chooseSelect(dialog, "Calibration quality", "Estimated");
		await setField(dialog, "Calibration source", "Copy survey");
		await dialog.getByRole("button", { name: "Save", exact: true }).click();
		await expect(dialog).toBeHidden();
		await expect
			.poll(async () => {
				const { fixture } = await patchFixture(api, showId, root);
				return [fixture.position_calibration?.pan_zero_degrees, fixture.multipatch[0].position_calibration?.pan_zero_degrees];
			})
			.toEqual([12.5, -7]);
	});
});
