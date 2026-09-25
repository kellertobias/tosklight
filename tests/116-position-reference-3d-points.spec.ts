import type {
	PatchFixtureInput,
	PatchFixturesOutcome,
	PatchSnapshot,
} from "../apps/light-desktop/src/api/generated/light-wire";
import { expect, test } from "./bench/core/fixtures";
import { loadCanonicalCopy } from "./support/catalog";
import { openPatch } from "./support/foundational/ui";
import { readPatchSnapshot } from "./support/operator/patch";

/**
 * docs/testing/30-position-reference-3d-points.md: the Position Reference column appears with the
 * first 3D Point, any fixture or Venue object can take a point as its reference through the
 * table, the reference is stored and survives an unrelated edit, and the desk states every
 * point's pose to the Stage.
 */

const POINT_ID = "9a3d0000-0000-4000-8000-000000000901";
const TRUSS_ID = "9a3d0000-0000-4000-8000-000000000001";

type LibraryProfile = {
	id: string;
	revision: number;
	manufacturer: string;
	name: string;
	modes: Array<{ id: string; name: string }>;
};

async function libraryProfile(
	api: { request<T>(method: string, path: string): Promise<T> },
	manufacturer: string,
	name: string,
	mode?: string,
) {
	const library = await api.request<{ profiles: LibraryProfile[] }>(
		"GET",
		"/api/v2/fixture-library/profiles",
	);
	const profile = library.profiles.find(
		(candidate) =>
			candidate.manufacturer === manufacturer && candidate.name === name,
	);
	if (!profile) throw new Error(`the library has no ${manufacturer} ${name}`);
	const chosen = mode
		? profile.modes.find((candidate) => candidate.name === mode)
		: profile.modes[0];
	if (!chosen) throw new Error(`${name} has no mode ${mode}`);
	return { profile, mode: chosen };
}

function fixtureInput(
	profile: LibraryProfile,
	modeId: string,
	overrides: Partial<PatchFixtureInput> & Pick<PatchFixtureInput, "fixture_id" | "name">,
): PatchFixtureInput {
	return {
		fixture_number: null,
		virtual_fixture_number: null,
		profile_id: profile.id,
		profile_revision: profile.revision,
		mode_id: modeId,
		split_patches: [{ split: 1, universe: null, address: null }],
		layer_id: "default",
		direct_control: null,
		location: { x: 0, y: 4000, z: 6000 },
		rotation: { x: 0, y: 0, z: 0 },
		multipatch: [],
		move_in_black_enabled: true,
		move_in_black_delay_millis: 0,
		highlight_overrides: [],
		...overrides,
	};
}

test("POSITION-REFERENCE-001 @ui › the column appears with a 3D Point, stores the reference and reports the point's pose", async ({
	api,
	bench,
	desk,
	page,
}) => {
	const show = await loadCanonicalCopy(
		api,
		bench,
		"position-reference-001",
		"default-stage",
	);
	const patchFixtures = async (
		fixtures: PatchFixtureInput[],
		remove: string[] = [],
	) => {
		const snapshot = await readPatchSnapshot(api, show.id);
		return api.request<PatchFixturesOutcome>(
			"POST",
			"/api/v2/patch/fixtures",
			{ request_id: crypto.randomUUID(), fixtures, remove_fixture_ids: remove },
			true,
			snapshot.patch_revision,
			{ showId: show.id },
		);
	};
	const fixtureNamed = (snapshot: PatchSnapshot, id: string) => {
		const fixture = snapshot.fixtures.find((candidate) => candidate.fixture_id === id);
		if (!fixture) throw new Error(`fixture ${id} is not in the patch`);
		return fixture;
	};

	await desk.open(api.baseUrl);
	await openPatch(page);
	const header = page.locator(".patch-table thead");
	await expect(header).not.toContainText("Position Reference");

	// A truss to hang on the point, then the point itself, both unpatched.
	const truss = await libraryProfile(api, "Venue", "Four-Point Truss");
	await patchFixtures([
		fixtureInput(truss.profile, truss.mode.id, {
			fixture_id: TRUSS_ID,
			virtual_fixture_number: 1,
			name: "Main truss",
		}),
	]);
	await expect(page.locator(".patch-table tbody")).toContainText("Main truss");
	await expect(header).not.toContainText("Position Reference");

	const point = await libraryProfile(api, "ToskLight", "3D Point", "Full 16 bit");
	await patchFixtures([
		fixtureInput(point.profile, point.mode.id, {
			fixture_id: POINT_ID,
			fixture_number: 901,
			name: "Truss point",
		}),
	]);
	await expect(page.locator(".patch-table tbody")).toContainText("Truss point");
	const headers = page.locator(".patch-table thead th");
	await expect(headers.filter({ hasText: "Position Reference" })).toHaveCount(1);
	const labels = await headers.evaluateAll((cells) =>
		cells.map((cell) => cell.getAttribute("aria-label") ?? cell.textContent ?? ""),
	);
	expect(labels.indexOf("Position Reference")).toBe(labels.indexOf("Scale") + 1);
	expect(labels.indexOf("Layer")).toBe(labels.indexOf("Position Reference") + 1);

	// The truss reads None; the point's own cell is a dash and offers no editor.
	const trussCell = page.getByRole("button", { name: "Position Reference 0.1" });
	await expect(trussCell).toHaveText("None");
	await expect(
		page.getByRole("button", { name: "Position Reference 901" }),
	).toHaveCount(0);

	// The literal path: SET, touch the cell, choose the point, Set.
	await page.getByRole("button", { name: "SET", exact: true }).click();
	await trussCell.click();
	const dialog = page.locator(".patch-edit-modal");
	await expect(
		dialog.getByRole("heading", { name: "Set fixture Position Reference" }),
	).toBeVisible();
	// The Select's trigger shows the chosen value; it opens a list of options.
	const select = dialog.locator(".ui-select-trigger");
	await expect(select).toHaveText("None");
	await select.click();
	await expect(page.getByRole("option")).toHaveText(["None", "901 · Truss point"]);
	await page.getByRole("option", { name: "901 · Truss point", exact: true }).click();
	await expect(select).toHaveText("901 · Truss point");
	await dialog.getByRole("button", { name: "Set", exact: true }).click();
	await expect(trussCell).toHaveText("901 · Truss point");
	await expect
		.poll(async () =>
			fixtureNamed(await readPatchSnapshot(api, show.id), TRUSS_ID).position_master,
		)
		.toBe(POINT_ID);

	// An unrelated edit keeps the reference: setting the truss's scale through the table writes
	// the whole fixture back, and the write carries the point with it.
	await page.getByRole("button", { name: "SET", exact: true }).click();
	await page.getByRole("button", { name: "Scale 0.1" }).click();
	const keypad = page.locator('[aria-label="Number input keypad"]');
	await keypad.getByRole("button", { name: "2", exact: true }).click();
	await keypad.getByRole("button", { name: "ENTER", exact: true }).click();
	await expect(page.getByRole("button", { name: "Scale 0.1" })).toHaveText("2×");
	await expect
		.poll(async () =>
			fixtureNamed(await readPatchSnapshot(api, show.id), TRUSS_ID).position_master,
		)
		.toBe(POINT_ID);

	// A reference to something that is not a 3D Point is refused before anything changes.
	const before = await readPatchSnapshot(api, show.id);
	const refused = await api
		.request<PatchFixturesOutcome>(
			"POST",
			"/api/v2/patch/fixtures",
			{
				request_id: crypto.randomUUID(),
				fixtures: [
					fixtureInput(point.profile, point.mode.id, {
						fixture_id: POINT_ID,
						fixture_number: 901,
						name: "Truss point",
						position_master: TRUSS_ID,
					}),
				],
				remove_fixture_ids: [],
			},
			true,
			before.patch_revision,
			{ showId: show.id },
		)
		.then(
			() => null,
			(error: unknown) => String(error),
		);
	expect(refused).toMatch(/cannot take a position reference/);
	expect((await readPatchSnapshot(api, show.id)).patch_revision).toBe(
		before.patch_revision,
	);

	// The desk states the point's pose to the Stage beside its universes: at rest, no offset.
	const output = await api.request<{
		points: Array<{
			fixture_id: string;
			offset_metres: [number, number, number];
			rotation_degrees: [number, number, number];
		}>;
	}>("GET", "/api/v2/output/dmx");
	const pose = output.points.find((candidate) => candidate.fixture_id === POINT_ID);
	expect(pose).toBeDefined();
	for (const value of [...(pose?.offset_metres ?? []), ...(pose?.rotation_degrees ?? [])])
		expect(Math.abs(value)).toBeLessThan(0.01);

	// Removing the point takes the column with it; the truss is drawn against the stage again.
	await patchFixtures([], [POINT_ID]);
	await expect(header).not.toContainText("Position Reference");
	await expect(page.locator(".patch-table tbody")).not.toContainText("Truss point");
});
