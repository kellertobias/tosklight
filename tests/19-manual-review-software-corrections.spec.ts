import type { Locator, Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";

test.describe("docs/testing/10-desk-lock-and-operator-ui.md", () => {
  test("MANUAL-019 @ui › saved workspaces are Desktops while physical control surfaces remain desks", async ({ api, bench, desk, page }) => {
    const physicalDesk = { ...api.session!.desk };
    const sessionId = api.session!.session_id;
    await desk.open(api.baseUrl);
    await desk.recordStep("TERMINOLOGY", "Saved workspace arrangements use Desktop everywhere; physical and protocol control surfaces retain desk terminology.");

    const desktops = page.getByRole("button", { name: "DESKTOPS", exact: true });
    await expect(desktops).toBeVisible();
    await expect(page.getByRole("button", { name: "DESKS", exact: true })).toHaveCount(0);
    await desktops.click();
    await expect(page.getByRole("button", { name: "New desktop", exact: false })).toBeVisible();

    const programming = dockEntry(page, "Programming");
    await programming.click();
    await expect(page.locator(".desk-pane")).toHaveCount(3);

    await desk.recordStep("DESKTOP SETTINGS", "Long-press the current Desktop, rename it, and clone its complete 24-by-18 pane layout.");
    await longPress(programming);
    let settings = page.getByRole("dialog", { name: "Desktop settings" });
    await expect(settings).toBeVisible();
    await expect(settings.getByRole("heading", { name: "Desktop", exact: true })).toBeVisible();
    await expect(page.getByRole("dialog", { name: "Desk settings" })).toHaveCount(0);
    await settings.getByLabel("Name").fill("Programming Desktop");
    await settings.getByLabel("Name").press("Tab");
    await expect(dockEntry(page, "Programming Desktop")).toBeVisible();
    await settings.getByRole("button", { name: "Clone current desktop" }).click();

    const clone = dockEntry(page, "Desktop 4");
    await expect(clone).toBeVisible();
    await expect(page.locator(".desk-pane")).toHaveCount(3);

    await desk.recordStep("DELETE COPY", "Delete only the cloned Desktop; the original arrangement and its panes remain available.");
    await longPress(clone);
    settings = page.getByRole("dialog", { name: "Desktop settings" });
    await settings.getByRole("button", { name: "Delete desktop" }).click();
    await expect(settings).toContainText("Delete desktop “Desktop 4”?");
    await settings.getByRole("button", { name: "Confirm delete" }).click();
    await expect(clone).toHaveCount(0);
    await expect(dockEntry(page, "Programming Desktop")).toBeVisible();

    const bootstrap = await api.request<any>("GET", "/api/v1/bootstrap", undefined, false);
    expect(api.session).toMatchObject({ session_id: sessionId, desk: physicalDesk });
    expect(bootstrap.desks.find((candidate: any) => candidate.id === physicalDesk.id)).toMatchObject({
      id: physicalDesk.id,
      osc_alias: physicalDesk.osc_alias,
    });

    await desk.recordStep("OSC DESK IDENTITY", "The label-only Desktop edit leaves the physical desk session and OSC alias untouched; an external key still routes into that same desk.");
    const hardware = await bench.osc();
    const clientId = `manual-019-${crypto.randomUUID()}`;
    try {
      await hardware.subscribe(clientId, physicalDesk.osc_alias);
      const mark = hardware.mark();
      await hardware.send(`/light/${physicalDesk.osc_alias}/programmer/digit-1`, [true]);
      await hardware.expectAfter(mark, `/light/${physicalDesk.osc_alias}/feedback/command-line`);
      await hardware.send(`/light/${physicalDesk.osc_alias}/programmer/clear`, [true]);
      await hardware.send(`/light/${physicalDesk.osc_alias}/programmer/clear`, [true]);
    } finally {
      await hardware.send("/light/unsubscribe", [clientId]).catch(() => undefined);
      await hardware.close();
    }

    await desk.recordStep("DESK IS PHYSICAL", "Open Show controls and confirm installation, status, and shutdown still use desk for the physical/logical control surface.");
    await page.locator(".dock-identity").click();
    const show = page.locator(".show-modal");
    await expect(show).toBeVisible();
    await expect(show.getByRole("button", { name: "Desk Status" })).toBeVisible();
    await expect(show.getByRole("button", { name: "Shut Down Desk" })).toBeVisible();
    await show.getByRole("button", { name: "Enter Setup" }).click();
    await expect(page.locator(".ui-window-title")).toHaveText("Desk Setup");
  });

  test("MANUAL-019 @ui › fixture browsers share title-bar search and readable name/detail alignment", async ({ api, desk, page }) => {
    await desk.open(api.baseUrl);
    await desk.recordStep("ADD FIXTURE", "Open the patch fixture browser; search belongs to its title bar and names align opposite their metadata.");

    await page.getByRole("button", { name: /Open show menu/ }).click();
    await page.getByRole("button", { name: "Show Patch", exact: true }).click();
    await page.getByRole("button", { name: "+ Add fixture", exact: true }).click();
    const addFixture = page.locator(".fixture-browser-modal");
    await expect(addFixture).toBeVisible();
    await expect(addFixture.locator("header.fixture-browser-header .console-search")).toBeVisible();
    await expect(addFixture.locator(":scope > .console-search")).toHaveCount(0);
    await expect(addFixture.locator(".fixture-picker-columns > section").nth(0).locator("button span").first()).toHaveCSS("text-align", "left");
    await expect(addFixture.locator(".fixture-picker-columns > section").nth(1).locator("button small").first()).toHaveCSS("text-align", "right");
    await addFixture.getByRole("button", { name: "Close Add fixture" }).click();

    await desk.recordStep("FIXTURE LIBRARY", "Open Fixture library; the same search surface and neighboring actions live in the window title while detail values align right.");
    await page.getByRole("button", { name: /Open show menu/ }).click();
    await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
    const setupNav = page.locator(".setup-window nav");
    await page.getByRole("button", { name: "Open Fixture Library", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Fixture Library" })).toBeVisible();
    const titleActions = page.locator("#setup-section-actions");
    await expect(titleActions.locator(".console-search")).toBeVisible();
    await expect(titleActions.getByRole("button", { name: "Import GDTF" })).toBeVisible();
    await expect(titleActions.getByRole("button", { name: "Create fixture" })).toBeVisible();
    await expect(page.locator(".fixture-library-columns > section").nth(0).locator("button span").first()).toHaveCSS("text-align", "left");
    await expect(page.locator(".fixture-library-columns > section").nth(1).locator("button small").first()).toHaveCSS("text-align", "right");
    await expect(page.locator(".fixture-library-detail dd").first()).toHaveCSS("text-align", "right");
  });

  test("MANUAL-019 @ui › every operator file field opens the confined picker with its own extension contract", async ({ api, desk, page }) => {
    const configuration = await api.request<any>("GET", "/api/v1/configuration");
    await api.request("PUT", "/api/v1/configuration", {
      ...configuration.configuration,
      file_manager_system_picker_fallback: false,
    });
    const prefix = `manual-picker-${crypto.randomUUID()}`;
    const files = {
      invalid: `${prefix}.txt`,
      show: `${prefix}.show`,
      mvr: `${prefix}.mvr`,
      gdtf: `${prefix}.gdtf`,
      wallpaper: `${prefix}.png`,
      scene: `${prefix}.glb`,
    };
    for (const name of Object.values(files)) {
      await api.request("POST", "/api/v1/files/shows/operations", { operation: "create_file", sources: [], destination: "", name });
    }

    await desk.open(api.baseUrl);
    await desk.recordStep("SHOW FILE PICKER", "Show from USB in the Load Show title bar starts ToskLight's confined file manager and accepts only portable .show files; Show from OS opens the operating-system picker directly.");
    await page.getByRole("button", { name: /Open show menu/ }).click();
    await page.getByRole("button", { name: "Load", exact: true }).click();
    const loadShow = page.getByRole("dialog", { name: "Load show" });
    const loadShowTitle = loadShow.locator(".ui-modal-titlebar");
    await expect(loadShowTitle.getByRole("button", { name: "Show from USB", exact: true })).toBeVisible();
    await expect(loadShowTitle.getByRole("button", { name: "Show from OS", exact: true })).toBeVisible();
    const osFileChooser = page.waitForEvent("filechooser");
    await loadShowTitle.getByRole("button", { name: "Show from OS", exact: true }).click();
    await osFileChooser;
    await loadShow.getByRole("button", { name: "Show from USB", exact: true }).click();
    await expectPickerConstraint(page, files.invalid, files.show);

    await desk.recordStep("MVR FILE PICKER", "New Show from MVR reuses the same picker but changes the accepted extension to .mvr.");
    await loadShow.getByRole("button", { name: "Load from MVR", exact: true }).click();
    const mvr = page.getByRole("dialog", { name: "MVR import and export" });
    await mvr.getByRole("button", { name: "Choose MVR file", exact: true }).click();
    await expectPickerConstraint(page, files.invalid, files.mvr);
    await mvr.locator(".modal-close").click();

    await desk.recordStep("GDTF FILE PICKER", "Fixture Library imports use the confined picker with a .gdtf-only selection contract.");
    await page.locator(".show-modal").getByRole("button", { name: "Enter Setup", exact: true }).click();
    const setupNav = page.locator(".setup-window nav");
    await page.getByRole("button", { name: "Open Fixture Library", exact: true }).click();
    await page.getByRole("button", { name: "Import GDTF", exact: true }).click();
    const gdtf = page.locator(".gdtf-import-modal");
    await gdtf.getByRole("button", { name: "Choose GDTF file", exact: true }).click();
    await expectPickerConstraint(page, files.invalid, files.gdtf);
    await gdtf.locator("header button").click();

    await desk.recordStep("FIXTURE ASSET PICKERS", "Fixture Library image and 3D-model fields use the same confined picker while preserving their distinct browser-image and GLB contracts.");
    await page.getByRole("button", { name: "Create fixture", exact: true }).click();
    const fixtureEditor = page.locator(".fixture-profile-editor-modal");
    await fixtureEditor.getByRole("button", { name: "Choose fixture icon", exact: true }).click();
    await expectPickerConstraint(page, files.invalid, files.wallpaper);
    await fixtureEditor.getByRole("button", { name: "Choose visualizer glb model", exact: true }).click();
    await expectPickerConstraint(page, files.invalid, files.scene);
    await fixtureEditor.getByRole("button", { name: "Close fixture editor" }).click();
	await page.getByRole("button", { name: "Close Fixture Library", exact: true }).click();

    await desk.recordStep("WALLPAPER FILE PICKER", "Desk Lock wallpaper selection stays inside configured roots and accepts browser image formats only.");
	await setupNav.getByRole("button", { name: "Screens & playback", exact: true }).click();
	await page.getByRole("button", { name: "Desk Lock", exact: true }).click();
    await page.getByRole("button", { name: "Choose lock wallpaper", exact: true }).click();
    await expectPickerConstraint(page, files.invalid, files.wallpaper);

    await api.request("POST", "/api/v1/files/shows/operations", { operation: "delete", sources: Object.values(files) });
  });

  test("MANUAL-019 @ui › File Manager and Text Editor put their contract actions in the pane header", async ({ api, desk, page }) => {
    const workspace = `manual-header-${crypto.randomUUID()}`;
    const textFile = `manual-editor-${crypto.randomUUID()}.md`;
    await api.request("POST", "/api/v1/files/shows/operations", { operation: "create_folder", sources: [], destination: "", name: workspace });
    await api.request("POST", "/api/v1/files/shows/operations", { operation: "create_file", sources: [], destination: workspace, name: ".hidden-note" });
    await api.request("POST", "/api/v1/files/shows/operations", { operation: "create_file", sources: [], destination: "", name: textFile });
    const empty = await api.request<any>("GET", `/api/v1/files/shows/text?path=${encodeURIComponent(textFile)}`);
    await api.request("PUT", "/api/v1/files/shows/text", { path: textFile, text: "# Operator notes\n\nStand by.\n", revision: empty.revision });

    await desk.open(api.baseUrl);
    await desk.recordStep("FILE MANAGER HEADER", "Edit, Create, View, Back, and Forward belong to the title bar; the live root-relative path sits beside File Manager.");
    let pane = await addPaneToNewDesktop(page, "File Manager");
    let headerActions = page.locator(".file-manager-header-actions");
    await expect(headerActions).toBeVisible();
    for (const action of ["Edit", "Create", "View", "Back", "Forward"]) {
      const button = headerActions.getByRole("button", { name: action, exact: true });
      await expect(button).toBeVisible();
    }
    await expectActionsInsidePaneHeader(headerActions, "File Manager");
    await expect(pane.locator(".file-toolbar").getByRole("button", { name: /^(?:Rename|Copy|Move|Delete|New File|New Folder)$/ })).toHaveCount(0);
    await page.getByRole("button", { name: "Create", exact: true }).click();
    expect(await page.getByRole("menu", { name: "Create menu" }).getByRole("menuitem").allTextContents()).toEqual(["New File", "New Folder"]);
    await page.locator(".file-header-menu-layer").click({ position: { x: 1, y: 1 } });

    await pane.getByRole("button", { name: `${workspace}, folder` }).dblclick();
    await expect(page.locator(".file-manager-header-path")).toHaveText(`Shows: /${workspace}`);
    await expect(pane.getByRole("button", { name: ".hidden-note, file" })).toHaveCount(0);
    await pane.getByRole("button", { name: "Settings", exact: true }).click();
    let settings = page.getByRole("dialog", { name: "Pane Settings" });
    await settings.getByRole("tab", { name: "File Manager", exact: true }).click();
    const hidden = settings.getByRole("switch", { name: "Show Hidden" });
    await hidden.locator("xpath=..").click();
    await settings.getByRole("button", { name: "Close settings" }).click();
    await expect(pane.getByRole("button", { name: ".hidden-note, file" })).toBeVisible();
    await page.getByRole("button", { name: "View", exact: true }).click();
    await page.getByRole("menu", { name: "View menu" }).getByRole("menuitem", { name: "Hide Properties" }).click();
    await expect(pane.getByRole("complementary", { name: "Selection properties" })).toBeHidden();

    await desk.recordStep("TEXT EDITOR HEADER", "Open File, Refresh, Save, and Save As share the pane title bar while status and file identity remain visible beside the title.");
    pane = await addPaneToNewDesktop(page, "Text Editor");
    headerActions = page.locator(".text-editor-header-actions");
    await expect(headerActions).toBeVisible();
    for (const action of ["Open File", "Refresh", "Save", "Save As"]) {
      const button = headerActions.getByRole("button", { name: action, exact: true });
      await expect(button).toBeVisible();
    }
    await expectActionsInsidePaneHeader(headerActions, "Text Editor");
    await expect(pane.locator(".text-editor-toolbar").getByRole("button", { name: /^(?:Open File|Refresh|Save|Save As)$/ })).toHaveCount(0);
    await page.getByRole("button", { name: "Open File", exact: true }).click();
    const picker = page.getByRole("dialog", { name: "Choose files or folders" });
    await picker.getByRole("button", { name: `${textFile}, file` }).click();
    await picker.getByRole("button", { name: "Select", exact: true }).click();
    await expect(page.locator(".text-editor-header-state")).toContainText(`Saved · ${textFile}`);
    await pane.getByLabel("File text").fill("# Operator notes\n\nStand by for beginners.\n");
    await expect(page.locator(".text-editor-header-state")).toContainText("Unsaved");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.locator(".text-editor-header-state")).toContainText("Saved");
    await expect.poll(async () => (await api.request<any>("GET", `/api/v1/files/shows/text?path=${encodeURIComponent(textFile)}`)).text).toContain("beginners");

    await api.request("POST", "/api/v1/files/shows/operations", { operation: "delete", sources: [workspace, textFile] });
  });

  test("MANUAL-019 @ui › the dedicated Cues pane keeps the cue editor visible without a delete action", async ({ api, desk, page }) => {
    const bootstrap = await api.request<any>("GET", "/api/v1/bootstrap", undefined, false);
    const cueListId = crypto.randomUUID();
    await api.request("PUT", `/api/v1/shows/${bootstrap.active_show.id}/objects/cue_list/${cueListId}`, {
      id: cueListId,
      name: "Manual Review Cuelist",
      priority: 0,
      mode: "sequence",
      looped: false,
      chaser_step_millis: 1_000,
      speed_group: null,
      cues: [
        {
          number: 1,
          name: "House Open",
          changes: [],
          group_changes: [],
          fade_millis: 1_500,
          delay_millis: 250,
          trigger: { type: "manual" },
          phasers: [],
        },
      ],
    }, true, 0);

    await desk.open(api.baseUrl);
    await desk.recordStep("CUE EDITOR", "The dedicated Cues pane keeps its right-side cue editor visible and deliberately offers no cue-deletion control.");
    const pane = await addPaneToNewDesktop(page, "Cues · Cuelist");
    const properties = pane.locator(".cue-properties");
    await expect(pane.locator(".sequence-layout.with-cue-properties")).toBeVisible();
    await expect(properties).toBeVisible();
    for (const label of ["Title", "Fade", "Delay", "Trigger"]) {
      await expect(properties.getByText(label, { exact: true }).first()).toBeVisible();
    }
    await expect(properties.getByLabel("Title", { exact: true })).toHaveValue("House Open");
    await expect(pane.getByRole("button", { name: "Delete Cue", exact: true })).toHaveCount(0);
  });

  test("MANUAL-019 @ui › Help stays two-column, DMX is a selected-channel monitor, and Stage Add Element opens a chooser", async ({ api, desk, page }) => {
    const bootstrap = await api.request<any>("GET", "/api/v1/bootstrap", undefined, false);
    const showId = bootstrap.active_show.id as string;
    await api.request("PUT", `/api/v1/shows/${showId}/objects/route/manual-existing`, {
      protocol: "art_net",
      logical_universe: 1,
      destination_universe: 101,
      destination: "127.0.0.1:6454",
      enabled: true,
      minimum_slots: 128,
    }, true, 0);
    const desktop = await desk.enableControllableDesktop();
    await desk.open(api.baseUrl);
    await expect.poll(() => desktop.actions).toContainEqual({ type: "frontend_ready" });

    await desk.recordStep("HELP COLUMNS", "Embedded Help keeps navigation on the left and the selected topic on the right at the same vertical position.");
    const help = await addPaneToNewDesktop(page, "Help");
    const navigationBox = await help.locator(".help-layout > nav").boundingBox();
    const contentBox = await help.locator(".help-layout > .ui-window-scroll-area").boundingBox();
    expect(navigationBox).not.toBeNull();
    expect(contentBox).not.toBeNull();
    expect(contentBox!.x).toBeGreaterThan(navigationBox!.x + navigationBox!.width - 2);
    expect(Math.abs(contentBox!.y - navigationBox!.y)).toBeLessThan(3);

    await desk.recordStep("OUTPUT ROUTES", "Desk Setup > Outputs owns versioned route editing: change an existing route, add another, and explicitly remove the new route.");
    await page.locator(".dock-identity").click();
    await page.locator(".show-modal").getByRole("button", { name: "Enter Setup", exact: true }).click();
    await page.locator(".setup-window nav").getByRole("button", { name: "Outputs", exact: true }).click();
    const outputRoutes = page.getByRole("region", { name: "Output routes" });
    let route = outputRoutes.locator("article").filter({ hasText: "Logical 1 → Art-Net 101" });
    await route.getByRole("button", { name: "Edit route", exact: true }).click();
    let routeEditor = page.getByRole("dialog", { name: "Output route editor" });
    await routeEditor.getByLabel("Destination universe").fill("102");
    await routeEditor.getByRole("button", { name: "Save route", exact: true }).click();
    route = outputRoutes.locator("article").filter({ hasText: "Logical 1 → Art-Net 102" });
    await expect(route).toBeVisible();

    await outputRoutes.getByRole("button", { name: "Add route", exact: true }).click();
    routeEditor = page.getByRole("dialog", { name: "Output route editor" });
    await routeEditor.getByLabel("Logical universe").fill("2");
    await routeEditor.getByLabel("Destination universe").fill("202");
    await routeEditor.getByLabel("Destination", { exact: true }).fill("127.0.0.1:6454");
    await routeEditor.getByRole("button", { name: "Save route", exact: true }).click();
    const createdRoute = outputRoutes.locator("article").filter({ hasText: "Logical 2 → Art-Net 202" });
    await expect(createdRoute).toBeVisible();
    await createdRoute.getByRole("button", { name: "Edit route", exact: true }).click();
    routeEditor = page.getByRole("dialog", { name: "Output route editor" });
    await routeEditor.getByRole("button", { name: "Remove route", exact: true }).click();
    await routeEditor.getByRole("button", { name: "Confirm remove", exact: true }).click();
    await expect(createdRoute).toHaveCount(0);

    await desk.recordStep("DMX MONITOR", "The DMX built-in has no route editor; selecting a patched channel reveals its fixture and raw output controls.");
    const patch = await api.request<any>("GET", "/api/v1/patch", undefined, false);
    const patched = patch.fixtures.find((fixture: any) => fixture.universe != null && fixture.address != null);
    expect(patched).toBeTruthy();
    await openBuiltIn(page, "DMX");
    const dmx = page.locator(".dmx-window");
    await expect(dmx.getByRole("button", { name: "Routes", exact: true })).toHaveCount(0);
    await dmx.getByRole("button", { name: new RegExp(`^Universe ${patched.universe}, address ${patched.address}, value`) }).click();
    await expect(dmx.locator(".dmx-fixture-card")).not.toContainText("Fixture: Empty");
    await expect(dmx.locator(".dmx-fixture-card")).toContainText(String(patched.fixture_number ?? patched.fixture_id));

    await desk.recordStep("STAGE SCENERY MODEL", "Stage no longer has a separate scene-asset workflow; scenery is added as a visual-only Venue fixture in Show Patch.");
    await openBuiltIn(page, "Stage");
    const stage = page.locator(".stage-window");
    await stage.getByRole("button", { name: "Setup positions", exact: true }).click();
    await expect(stage.getByRole("button", { name: "Import scene", exact: true })).toHaveCount(0);
    await expect(stage.getByRole("button", { name: "Add element", exact: true })).toHaveCount(0);
  });

  test("MANUAL-019 @ui › Development stays out of operator panes and remains available through Desk Status", async ({ api, desk, page }) => {
    await desk.open(api.baseUrl);
    await desk.recordStep("OPERATOR PANE CATALOG", "Development is not an operator pane choice on a new Desktop.");
    await page.getByRole("button", { name: "DESKTOPS", exact: true }).click();
    await page.getByRole("button", { name: /New desktop/ }).click();
    await page.locator(".empty-desk").click({ position: { x: 10, y: 10 } });
    const picker = page.getByRole("heading", { name: "Open Window" }).locator("xpath=..");
    await expect(picker.getByRole("button", { name: "Development", exact: true })).toHaveCount(0);
    await picker.getByRole("button", { name: "Cancel", exact: true }).click();
    await page.getByRole("button", { name: "SHIFT", exact: true }).click();
    await page.getByRole("button", { name: "0", exact: true }).click();
    await expect(page.locator(".development-window")).toHaveCount(0);

    await desk.recordStep("DEVELOPER TOOLING", "Desk Status deliberately retains the Development component catalog for diagnostics and help maintenance.");
    await page.locator(".dock-identity").click();
    await page.locator(".show-modal").getByRole("button", { name: "Desk Status", exact: true }).click();
    const status = page.getByRole("dialog", { name: "Desk Status" });
    await status.getByRole("button", { name: /Debug/ }).click();
    await status.getByRole("menuitem", { name: "Open Development", exact: true }).click();
    await expect(page.locator(".development-window")).toBeVisible();
    await expect(page.locator(".ui-window-title")).toHaveText("Development");
  });

  test("MANUAL-019 @ui › Shows & recovery loads a root-confined .show selection with safe blackout", async ({ api, desk, page }) => {
    const bootstrap = await api.request<any>("GET", "/api/v1/bootstrap", undefined, false);
    const source = await fetch(`${api.baseUrl}/api/v1/shows/${bootstrap.active_show.id}/download`, {
      headers: { authorization: `Bearer ${api.session!.token}` },
    });
    expect(source.ok).toBe(true);
    const copyName = `Recovery Browser ${crypto.randomUUID()}`;
    const copy = await api.request<any>("POST", "/api/v1/shows", {
      name: copyName,
      data_base64: Buffer.from(await source.arrayBuffer()).toString("base64"),
      overwrite: false,
    });
    const decoy = `not-a-show-${crypto.randomUUID()}.txt`;
    await api.request("POST", "/api/v1/files/shows/operations", { operation: "create_file", sources: [], destination: "", name: decoy });

    await desk.open(api.baseUrl);
    await desk.recordStep("SHOW RECOVERY FILE MANAGER", "Shows & recovery starts in the confined Shows root, rejects non-show files, and opens the selected indexed show through safe blackout.");
    await page.locator(".dock-identity").click();
    await page.locator(".show-modal").getByRole("button", { name: "Enter Setup", exact: true }).click();
    const browser = page.getByRole("region", { name: "Show file manager" });
    await expect(browser).toBeVisible();
    await expect(browser.getByRole("navigation", { name: "Breadcrumb" })).toContainText("Shows");
    const load = browser.getByRole("button", { name: "Load selected show safely", exact: true });
    await browser.getByRole("button", { name: `${decoy}, file` }).click();
    await expect(load).toBeDisabled();
    await browser.getByRole("button", { name: `${copyName}.show, file` }).click();
    await expect(load).toBeEnabled();
    const openRequest = page.waitForRequest((request) => request.method() === "POST" && request.url().endsWith(`/api/v1/shows/${copy.id}/open`));
    await load.click();
    expect((await openRequest).postDataJSON()).toEqual({ transition: "safe_blackout" });
    await expect(browser.getByRole("status")).toContainText(`${copyName}.show is now open.`);
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).active_show.id).toBe(copy.id);
  });
});

function dockEntry(page: Page, name: string): Locator {
  return page.locator(".dock-entry").filter({ hasText: name }).first();
}

async function longPress(target: Locator): Promise<void> {
  await target.dispatchEvent("pointerdown", { pointerId: 1, pointerType: "mouse", button: 0 });
  await target.page().waitForTimeout(700);
  await target.dispatchEvent("pointerup", { pointerId: 1, pointerType: "mouse", button: 0 });
}

async function addPaneToNewDesktop(page: Page, name: "File Manager" | "Text Editor" | "Help" | "Cues · Cuelist"): Promise<Locator> {
  await page.getByRole("button", { name: "DESKTOPS", exact: true }).click();
  await page.getByRole("button", { name: /New desktop/ }).click();
  const grid = page.locator(".desk-grid");
  const box = await grid.boundingBox();
  expect(box).not.toBeNull();
  await page.mouse.click(box!.x + box!.width * 0.15, box!.y + box!.height * 0.15);
  await page.getByRole("button", { name, exact: true }).click();
  const pane = page.locator(".desk-pane").filter({ hasText: name === "Cues · Cuelist" ? "Cues" : name }).first();
  const gridBox = await grid.boundingBox();
  const resizeBox = await pane.locator(".pane-resize-handle").boundingBox();
  expect(gridBox).not.toBeNull();
  expect(resizeBox).not.toBeNull();
  await page.mouse.move(resizeBox!.x + resizeBox!.width / 2, resizeBox!.y + resizeBox!.height / 2);
  await page.mouse.down();
  await page.mouse.move(gridBox!.x + gridBox!.width * 0.96, gridBox!.y + gridBox!.height * 0.9, { steps: 5 });
  await page.mouse.up();
  await expect.poll(async () => (await pane.boundingBox())?.width ?? 0).toBeGreaterThan(900);
  return pane;
}

async function expectActionsInsidePaneHeader(actions: Locator, title: string): Promise<void> {
  const page = actions.page();
  const headerTitle = page.locator(".ui-window-title", { hasText: title }).first();
  await expect(headerTitle).toHaveText(title);
  const titleBox = await headerTitle.boundingBox();
  const actionsBox = await actions.boundingBox();
  expect(titleBox).not.toBeNull();
  expect(actionsBox).not.toBeNull();
  expect(actionsBox!.x).toBeGreaterThan(titleBox!.x + titleBox!.width);
  expect(Math.abs(actionsBox!.y + actionsBox!.height / 2 - (titleBox!.y + titleBox!.height / 2))).toBeLessThan(3);
}

async function openBuiltIn(page: Page, name: "DMX" | "Stage") {
  await page.getByRole("button", { name: "BUILT-INS", exact: true }).click();
  await page.locator(".dock-entry").filter({ hasText: name }).first().click();
}

async function expectPickerConstraint(page: Page, invalidName: string, allowedName: string): Promise<void> {
  const picker = page.getByRole("dialog", { name: "Choose files or folders" });
  await expect(picker).toBeVisible();
  await expect(picker.getByRole("button", { name: "Open system file picker", exact: true })).toHaveCount(0);
  const select = picker.getByRole("button", { name: "Select", exact: true });
  await picker.getByRole("button", { name: `${invalidName}, file`, exact: true }).click();
  await expect(select).toBeDisabled();
  await picker.getByRole("button", { name: `${allowedName}, file`, exact: true }).click();
  await expect(select).toBeEnabled();
  await picker.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(picker).toHaveCount(0);
}
