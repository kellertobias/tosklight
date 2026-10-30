import { expect, test } from "./bench/core/fixtures";

test("@ui Show Health finds a broken Group, fixes once, undoes and opens the exact Group", async ({
	api,
	bench,
	desk,
	page,
	show,
}) => {
	const missing = crypto.randomUUID();
	await api.seedShowObject(show.id, "group", "75", {
		id: "75",
		name: "Health test",
		fixtures: [show.fixtureIds[0], missing],
		source: { type: "explicit", fixture_ids: [show.fixtureIds[0], missing] },
		future: { preserve: true },
	});
	await api.seedShowObject(show.id, "group", "76", {
		id: "76",
		name: "Intentionally empty",
		fixtures: [],
	});
	await desk.open(bench.baseUrl);
	await page.getByRole("button", { name: /^Open show menu/ }).click();
	await page.getByRole("button", { name: /Show Health/ }).click();
	const health = page.getByRole("dialog", { name: "Show Health", exact: true });
	await expect(health).toBeVisible();
	const broken = health.locator("li").filter({ hasText: missing });
	await expect(broken).toHaveCount(1);
	await expect(health.getByText(/Intentionally empty/)).toHaveCount(0);
	await broken.getByRole("button", { name: "Remove missing fixtures" }).click();
	await expect(
		health.getByText("Removed missing fixture references from Group 75."),
	).toBeVisible();
	await expect(broken).toHaveCount(0);
	expect(
		(await api.showObject<any>(show.id, "group", "75"))!.body.source
			.fixture_ids,
	).toEqual([show.fixtureIds[0]]);
	await health.getByRole("button", { name: "Undo last cleanup" }).click();
	await expect(health.getByText("Undid cleanup of Group 75.")).toBeVisible();
	await expect(broken).toHaveCount(1);
	await broken.getByRole("button", { name: "Go to group" }).click();
	await expect(
		page
			.getByRole("dialog")
			.filter({ has: page.getByRole("heading", { name: /Group 75/ }) }),
	).toBeVisible();
});

test("@ui Show Health reveals Fixtures from a cached Media tab and opens the exact location editor", async ({api,bench,desk,page,show}) => {
    const record = (await api.showObject<Record<string,unknown>>(show.id,"patched_fixture",show.fixtureIds[0]))!;
    const body = {...record.body, location: {x:0,y:0,z:0}};
    await api.seedShowObject(show.id,"patched_fixture",show.fixtureIds[0],body,record.revision);
    await desk.open(bench.baseUrl);
    await page.getByRole("button",{name:/^Open show menu/}).click();
    await page.getByRole("button",{name:"Show Patch",exact:true}).click();
    await page.getByRole("tab",{name:"Media Servers",exact:true}).click();
    await page.getByRole("button",{name:/^Open show menu/}).click();
    await page.getByRole("button",{name:/Show Health/}).click();
    const health=page.getByRole("dialog",{name:"Show Health",exact:true});
    await health.locator("li").filter({hasText:"Fixture 1 Dimmer 1 is at the Stage origin"}).first().getByRole("button",{name:"Go to fixture"}).click();
    await expect(page.getByRole("heading",{name:"Set fixture location",exact:true})).toBeVisible();
    await expect(page.locator(`tr[data-fixture-id="${show.fixtureIds[0]}"]`)).toHaveClass(/selected/);
    await expect(page.getByRole("button",{name:"Cancel fixture location"})).toBeVisible();
});
