import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QuickSetupModal } from "./QuickSetupModal";

vi.mock("../../features/deskLock/DeskLockActionsProvider", () => ({
  useDeskLockActions: () => ({ lockDesk: mocks.lockDesk }),
}));

const mocks = vi.hoisted(() => ({
  lifecycleAvailable: true,
  lockDesk: vi.fn(),
  dispatch: vi.fn(),
  fileContent: vi.fn(),
  openFileManagerPicker: vi.fn(),
  files: {
    fileContent: vi.fn(),
    fileRoots: vi.fn().mockResolvedValue([{id:"shows",label:"Shows",writable:true,removable:false}]),
    fileEntries: vi.fn().mockResolvedValue({entries:[{name:"tour.show",path:"tour.show",kind:"file",modified_millis:0},{name:"festival.show",path:"festival.show",kind:"file",modified_millis:0}]}),
    fileOperation: vi.fn(),
  },
  server: {
    status: "connected" as const,
    bootstrap: {
      active_show: {
        id: "copy",
        name: "Tour-rev-3-2026-07-17",
        revision: 1,
        updated_at: "2026-07-17T12:00:00Z",
        created_at: "2026-07-01T10:00:00Z",
        last_loaded_at: "2026-07-16T09:00:00Z",
        path: "copy.show",
        revision_copy: {
          show_id: "original",
          show_name: "Tour",
          revision: 3,
          revision_name: "Approved focus",
          copied_at: "2026-07-17T11:30:00Z",
        },
      },
      users: [{ id: "operator", name: "Operator", enabled: true }],
    } as any,
    session: { user: { id: "operator", name: "Operator" } },
    shows: [
      { id: "original", name: "Tour", revision: 4, updated_at: "", path: "tour.show" },
      { id: "copy", name: "Tour-rev-3-2026-07-17", revision: 1, updated_at: "", path: "copy.show" },
      { id: "other", name: "Festival", revision: 2, updated_at: "", path: "festival.show" },
    ] as any[],
    error: null as string | null,
    listShowRevisions: vi.fn(),
    openShowRevision: vi.fn(),
    overwriteShow: vi.fn(),
    saveShowRevision: vi.fn(),
    saveShowAs: vi.fn(),
    saveShowCopy: vi.fn(),
    exportMvrFile: vi.fn(),
    networkSaveFolders: vi.fn(),
    openShow: vi.fn().mockResolvedValue(true),
    networkShows: vi.fn().mockResolvedValue({peers:[]}),
    prepareShowRevision: vi.fn(),
    prepareShowFile: vi.fn(),
    importRemoteShow: vi.fn(),
    openShowFile: vi.fn(),
    uploadShow: vi.fn(),
    discoveredVisualizers: vi.fn(),
    loadFromVisualizer: vi.fn(),
    initializeEmptyShow: vi.fn(),
    saveScreen: vi.fn(),
    changeUser: vi.fn(),
    shutdownServer: vi.fn(),
    previewMvr: vi.fn(),
    applyMvr: vi.fn(),
    downloadShow: vi.fn(),
	selectiveImportCatalog: vi.fn(),
	previewSelectiveImport: vi.fn(),
	applySelectiveImport: vi.fn(),
  },
}));



vi.mock("../../api/ServerContext", () => ({ useServer: () => mocks.server }));
vi.mock("../../features/deskSnapshot/DeskSnapshotState", async (importOriginal) => ({
	...(await importOriginal<object>()),
	useBootstrapSnapshot: () => mocks.server.bootstrap,
	useSessionSnapshot: () => mocks.server.session ?? null,
}));
vi.mock(
	"../../features/showLifecycle/ShowLifecycleContext",
	async (importOriginal) => ({
		...(await importOriginal<object>()),
		useShowLifecycle: () => mocks.lifecycleAvailable ? mocks.server : null,
	}),
);
vi.mock("../../features/selectiveImport/SelectiveImportContext", () => ({
	useSelectiveImport: () => ({
		catalog: mocks.server.selectiveImportCatalog,
		preview: mocks.server.previewSelectiveImport,
		apply: mocks.server.applySelectiveImport,
	}),
}));
vi.mock("../../features/screens/ScreensContext", () => ({
	useScreens: () => ({
		screens: null,
		saveScreen: mocks.server.saveScreen,
	}),
}));
vi.mock("../../features/files/FilesContext", () => ({
  useFiles: () => mocks.files,
}));
vi.mock("../../windows/FileManagerPickerHost", () => ({
	openFileManagerPicker: mocks.openFileManagerPicker,
}));
vi.mock("../../state/AppContext", () => ({
  useApp: () => ({
    state: { setupOpen: true, desks: [], activeDeskId: "" },
    dispatch: mocks.dispatch,
  }),
}));

beforeEach(() => {
    mocks.lifecycleAvailable = true;
    mocks.server.bootstrap.active_show.id = "copy";
    mocks.server.bootstrap.active_show.name = "Tour-rev-3-2026-07-17";
    mocks.server.bootstrap.active_show.revision_copy = {
      show_id: "original",
      show_name: "Tour",
      revision: 3,
      revision_name: "Approved focus",
      copied_at: "2026-07-17T11:30:00Z",
    };
    mocks.server.shows = [
      { id: "original", name: "Tour", revision: 4, updated_at: "", path: "tour.show" },
      { id: "copy", name: "Tour-rev-3-2026-07-17", revision: 1, updated_at: "", path: "copy.show" },
      { id: "other", name: "Festival", revision: 2, updated_at: "", path: "festival.show" },
    ];
    mocks.files.fileRoots.mockReset().mockResolvedValue([{id:"shows",label:"Shows",writable:true,removable:false}]);
    mocks.files.fileEntries.mockReset().mockResolvedValue({entries:[{name:"tour.show",path:"tour.show",kind:"file",modified_millis:0},{name:"festival.show",path:"festival.show",kind:"file",modified_millis:0}]});
    mocks.server.networkShows.mockReset().mockResolvedValue({peers:[]});
    mocks.server.saveShowCopy.mockReset().mockResolvedValue({name:"Folder copy"});
    mocks.server.exportMvrFile.mockReset().mockResolvedValue({root_id:"shows",path:"Folder copy.mvr",summary:{fixtures:0,scenery:0,embedded_profiles:0,missing_profiles:[],omitted:[],warnings:[]}});
    mocks.server.networkSaveFolders.mockReset().mockResolvedValue({root_id:"shows",roots:[{id:"shows",label:"Desk shows",writable:true,removable:false}],entries:[]});
    mocks.server.listShowRevisions.mockReset().mockImplementation(async (id: string) => id === "original" ? [{ show_id: id, revision: 3, name: "Approved focus", created_at: "2026-07-16T10:00:00Z" }] : []);
    mocks.server.openShowRevision.mockReset().mockResolvedValue(true);
    mocks.server.overwriteShow.mockReset().mockResolvedValue(true);
    mocks.server.saveShowAs.mockReset().mockResolvedValue(true);
    mocks.server.discoveredVisualizers.mockReset().mockResolvedValue([]);
    mocks.server.loadFromVisualizer.mockReset().mockResolvedValue(true);
	mocks.openFileManagerPicker.mockReset().mockResolvedValue(null);
});

afterEach(() => {
    cleanup();
    vi.clearAllMocks();
});

describe("QuickSetupModal show workflows", () => {
  it("shows the current show, history, connection state, patch size, address, and build", () => {
    render(<QuickSetupModal />);
    const menu = screen.getByRole("dialog", { name: "Show" });
    expect(menu).toHaveTextContent("Current show: Tour-rev-3-2026-07-17");
    expect(menu).toHaveTextContent("Created:");
    expect(menu).toHaveTextContent("Previously loaded:");
    expect(menu).toHaveTextContent("Last saved:");
    expect(menu).toHaveTextContent("Last named revision:");
    expect(menu).toHaveTextContent("Server disconnected · Hardware disconnected");
    expect(menu).toHaveTextContent("DMX universes 0");
    expect(menu).toHaveTextContent("IP address localhost");
    expect(menu).toHaveTextContent("Parameters sent 0");
    expect(menu).toHaveTextContent("Software build");
  });
  it("identifies the active copy and requires confirmation before overwriting the original", async () => {
    render(<QuickSetupModal />);
    const menu = screen.getByRole("dialog", { name: "Show" });
    expect(menu).toHaveTextContent("Separate revision copy");
    expect(menu).toHaveTextContent("Tour, Revision 3 · Approved focus");
    expect(menu).toHaveTextContent("Current changes are autosaved to this copy, not to Tour");

    fireEvent.click(within(menu).getByRole("button", { name: "Save" }));
    const save = screen.getByRole("dialog", { name: "Save revision copy" });
    expect(save).toHaveTextContent("Autosave already protects this copy");
    fireEvent.click(within(save).getByRole("button", { name: "Overwrite Original Show" }));

    const confirmation = screen.getByRole("alertdialog", { name: "Confirm overwrite Tour" });
    expect(confirmation).toHaveTextContent("identity and named revisions are preserved");
    expect(mocks.server.overwriteShow).not.toHaveBeenCalled();
    fireEvent.click(within(confirmation).getByRole("button", { name: "Replace Tour Latest Autosave" }));
    await waitFor(() => expect(mocks.server.overwriteShow).toHaveBeenCalledWith("original"));
  });

  it("keeps revisions and Latest Autosave out of Save As", async () => {
    render(<QuickSetupModal />);
    fireEvent.click(screen.getByRole("button", { name: "Save As" }));
    const dialog = screen.getByRole("dialog", { name: "Save show" });
    expect(dialog).not.toHaveTextContent("Latest Autosave");
    expect(dialog).not.toHaveTextContent("Original show");
    expect(dialog).not.toHaveTextContent("Approved focus");
    expect(within(dialog).getByRole("switch", {name: "Save as Template"})).not.toBeChecked();
  });

  it("initializes the save designation from an existing base", async () => {
    mocks.server.bootstrap.active_show.is_base_show = true;
    render(<QuickSetupModal />);
    fireEvent.click(screen.getByRole("button", {name: "Save As"}));
    await waitFor(() => expect(screen.getByRole("switch", {name: "Save as Template"})).toBeChecked());
    delete mocks.server.bootstrap.active_show.is_base_show;
  });

  it("saves and exports into the selected folder with Source in the title", async () => {
    mocks.files.fileEntries.mockImplementation(async (_root: string, path: string) => ({entries:path ? [] : [{kind:"folder",name:"Tour folder",path:"Tour folder"}]}));
    render(<QuickSetupModal />);
    fireEvent.click(screen.getByRole("button", {name:"Save As"}));
    const dialog = screen.getByRole("dialog", {name:"Save show"});
    const source = within(dialog).getByRole("button", {name:"Source: Internal"});
    expect(source.closest(".ui-title-chrome")).not.toBeNull();
    expect(within(dialog).getByRole("switch", {name:"Save as Template"})).not.toBeChecked();
    const location = await within(dialog).findByRole("button", {name:/Location: Shows/});
    expect(within(dialog).getByRole("textbox", {name:"Show name"})).toHaveValue(mocks.server.bootstrap.active_show.name);
    expect(mocks.files.fileEntries).not.toHaveBeenCalled();
    expect(within(dialog).getByRole("button", {name:"Export MVR"}).closest(".ui-title-chrome")).not.toBeNull();
    expect(location).toHaveAttribute("aria-expanded", "false");
    expect(within(dialog).queryByRole("table")).not.toBeInTheDocument();
    fireEvent.click(location);
    fireEvent.click(await within(dialog).findByRole("button", {name:"📁 Tour folder"}));
    await waitFor(() => expect(location).toHaveTextContent("Shows / Tour folder"));
    fireEvent.click(within(dialog).getByRole("button", {name:"↑ Up one folder"}));
    await waitFor(() => expect(location).toHaveTextContent("Shows /"));
    expect(mocks.files.fileEntries).toHaveBeenCalledTimes(2);
    expect(within(dialog).queryByRole("button", {name:"↑ Up one folder"})).not.toBeInTheDocument();
    fireEvent.click(await within(dialog).findByRole("button", {name:"📁 Tour folder"}));
    fireEvent.change(within(dialog).getByRole("textbox",{name:"Show name"}),{target:{value:"Folder copy"}});
    await waitFor(() => expect(within(dialog).getByRole("button", {name:"Save as New Show"})).toBeEnabled());
    fireEvent.click(within(dialog).getByRole("switch",{name:"Save as Template"}));
    expect(within(dialog).getByRole("switch",{name:"Save as Template"})).toBeChecked();
    fireEvent.click(within(dialog).getByRole("button",{name:"Save as New Show"}));
    await waitFor(() => expect(mocks.server.saveShowCopy).toHaveBeenCalledWith("Folder copy",{rootId:"shows",path:"Tour folder"},true));
    await waitFor(() => expect(within(dialog).getByRole("button",{name:"Export MVR"})).toBeEnabled());
    fireEvent.click(within(dialog).getByRole("button",{name:"Export MVR"}));
    await waitFor(() => expect(mocks.server.exportMvrFile).toHaveBeenCalledWith("Folder copy",{rootId:"shows",path:"Tour folder"}));
    expect(mocks.server.saveShowAs).not.toHaveBeenCalled();
  });

  it("shows the server's export summary and every warning until dismissed or another action starts", async () => {
    mocks.server.exportMvrFile.mockResolvedValue({root_id:"shows",path:"Rig.mvr",summary:{fixtures:12,scenery:3,embedded_profiles:12,missing_profiles:["Acme · Spot (revision 2)"],
      omitted:["cues, presets, playbacks, users, and desk layouts"],
      warnings:["ToskLight generated GDTF files from the current fixture profiles","Acme · Spot (revision 2): GDTF could not be generated: curve. Export its native .toskfixture package to preserve the complete profile."]}});
    render(<QuickSetupModal />);
    fireEvent.click(screen.getByRole("button", {name:"Save As"}));
    const dialog = screen.getByRole("dialog", {name:"Save show"});
    await waitFor(() => expect(within(dialog).getByRole("button",{name:"Export MVR"})).toBeEnabled());
    fireEvent.click(within(dialog).getByRole("button",{name:"Export MVR"}));
    const summary = await within(dialog).findByRole("status",{name:"MVR export summary"});
    expect(summary).toHaveTextContent("Exported MVR to Shows / Rig.mvr");
    expect(summary).toHaveTextContent("12 fixtures · 3 scenery objects");
    expect(summary).toHaveTextContent("Not included: cues, presets, playbacks, users, and desk layouts");
    expect(summary).toHaveTextContent("2 export warnings");
    const warnings = within(summary).getAllByRole("listitem");
    expect(warnings.map(item => item.textContent)).toEqual([
      "ToskLight generated GDTF files from the current fixture profiles",
      "Acme · Spot (revision 2): GDTF could not be generated: curve. Export its native .toskfixture package to preserve the complete profile.",
    ]);
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {configurable:true,value:{writeText}});
    fireEvent.click(within(summary).getByRole("button",{name:"Copy warnings"}));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith(warnings.map(item => item.textContent).join("\n")));
    expect(await within(summary).findByRole("button",{name:"Warnings copied"})).toBeInTheDocument();
    fireEvent.click(within(summary).getByRole("button",{name:"Dismiss"}));
    expect(within(dialog).queryByRole("status",{name:"MVR export summary"})).not.toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole("button",{name:"Export MVR"}));
    await within(dialog).findByRole("status",{name:"MVR export summary"});
    fireEvent.click(within(dialog).getByRole("button",{name:"Save as New Show"}));
    await waitFor(() => expect(within(dialog).queryByRole("status",{name:"MVR export summary"})).not.toBeInTheDocument());
  });

  it("keeps a copyable error and no summary when the MVR export fails", async () => {
    mocks.server.exportMvrFile.mockRejectedValue(new Error("show not found"));
    render(<QuickSetupModal />);
    fireEvent.click(screen.getByRole("button", {name:"Save As"}));
    const dialog = screen.getByRole("dialog", {name:"Save show"});
    await waitFor(() => expect(within(dialog).getByRole("button",{name:"Export MVR"})).toBeEnabled());
    fireEvent.click(within(dialog).getByRole("button",{name:"Export MVR"}));
    const alert = await within(dialog).findByRole("alert");
    expect(alert).toHaveTextContent("show not found");
    expect(within(alert).getByRole("button",{name:"Copy error"})).toBeInTheDocument();
    expect(within(dialog).queryByRole("status",{name:"MVR export summary"})).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Exporting MVR…")).not.toBeInTheDocument();
  });

  it.each(["Load", "Save As"])("lists each USB drive in the %s Source menu and replaces denied folders with an error", async (action) => {
    mocks.files.fileRoots.mockResolvedValue([
      {id:"shows",label:"Shows",writable:true,removable:false},
      {id:"usb-a",label:"Tour USB",writable:true,removable:true},
      {id:"usb-b",label:"Backup USB",writable:true,removable:true},
    ]);
    mocks.files.fileEntries.mockImplementation(async (root: string) => {
      if (root === "usb-b") throw new Error("Operation not permitted");
      return {entries:[]};
    });
    render(<QuickSetupModal />);
    fireEvent.click(screen.getByRole("button", {name:action}));
    const dialog = screen.getByRole("dialog", {name:action === "Load" ? "Load show" : "Save show"});
    const source = within(dialog).getByRole("button", {name:"Source: Internal"});
    await waitFor(() => expect(source).toBeEnabled());
    fireEvent.click(source);
    expect(await screen.findByRole("menuitem", {name:"USB: Tour USB"})).toBeInTheDocument();
    fireEvent.click(screen.getByRole("menuitem", {name:"USB: Backup USB"}));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent("Operation not permitted");
    expect(within(dialog).queryByRole("table")).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("combobox")).not.toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", {name:"Source: USB: Backup USB"}));
    fireEvent.click(screen.getByRole("menuitem", {name:"USB: Tour USB"}));
    await waitFor(() => expect(within(dialog).getByRole("table")).toBeInTheDocument());
    expect(within(dialog).queryByRole("alert")).not.toBeInTheDocument();
  });

  it("offers network shares and Control desks as save destinations", async () => {
    mocks.files.fileRoots.mockResolvedValue([{id:"shows",label:"Shows",writable:true,removable:false},{id:"share",label:"Show share",writable:true,removable:false,network:true}]);
    mocks.server.networkShows.mockResolvedValue({peers:[{instance:"desk-one",name:"Lighting desk",role:"desk",shows:[],error:null},{instance:"architect-one",name:"Architect",role:"architect",shows:[],error:null}]});
    render(<QuickSetupModal />);
    fireEvent.click(screen.getByRole("button",{name:"Save As"}));
    const dialog = screen.getByRole("dialog",{name:"Save show"});
    await waitFor(() => expect(within(dialog).getByRole("button",{name:"Source: Internal"})).toBeEnabled());
    fireEvent.click(within(dialog).getByRole("button",{name:"Source: Internal"}));
    fireEvent.click(screen.getByRole("menuitem",{name:"Network"}));
    await waitFor(() => expect(within(dialog).getByRole("button",{name:"Lighting desk"})).toBeEnabled());
    expect(within(dialog).getByRole("button",{name:"Show share"})).toBeInTheDocument();
    expect(within(dialog).getByRole("button",{name:"Architect"})).toBeDisabled();
    fireEvent.click(within(dialog).getByRole("button",{name:"Lighting desk"}));
    fireEvent.change(within(dialog).getByRole("textbox",{name:"Show name"}),{target:{value:"Tour-rev-3-2026-07-17"}});
    await waitFor(() => expect(within(dialog).getByRole("button",{name:"Save as New Show"})).toBeEnabled());
    fireEvent.click(within(dialog).getByRole("button",{name:"Save as New Show"}));
    await waitFor(() => expect(mocks.server.saveShowCopy).toHaveBeenCalledWith("Tour-rev-3-2026-07-17",{rootId:"shows",path:"",instance:"desk-one"},false));
  });

  it("offers saved bases through New Show without opening the original", async () => {
    mocks.server.shows[2].is_base_show = true;
    render(<QuickSetupModal />);
    fireEvent.click(screen.getByRole("button", {name: "New Show"}));
    const dialog = screen.getByRole("dialog", {name: "New show"});
    fireEvent.click(within(dialog).getByRole("button", {name: "Create show from Festival"}));
    await waitFor(() => expect(mocks.server.initializeEmptyShow).toHaveBeenCalledWith("other"));
    expect(mocks.server.openShow).not.toHaveBeenCalled();
    delete mocks.server.shows[2].is_base_show;
  });

  it("closes the top dialog on Escape before closing the Show menu", () => {
    render(<QuickSetupModal />);
    fireEvent.click(screen.getByRole("button", { name: "Save As" }));

    fireEvent.keyDown(window, { key: "Escape" });

    expect(screen.queryByRole("dialog", { name: "Save show" })).not.toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: "Show" })).toBeVisible();
    expect(mocks.dispatch).not.toHaveBeenCalledWith({
      type: "SET_MODAL",
      modal: "setupOpen",
      value: false,
    });
  });

  it("names an autosaved empty show by renaming the existing identity", async () => {
    mocks.server.bootstrap.active_show.id = "empty";
    mocks.server.bootstrap.active_show.name = "New Empty Show 2";
    mocks.server.bootstrap.active_show.revision_copy = undefined;
    mocks.server.shows = [
      { id: "empty", name: "New Empty Show 2", revision: 1, updated_at: "", path: "New Empty Show 2.show" },
      { id: "other", name: "Festival", revision: 2, updated_at: "", path: "festival.show" },
    ];
    render(<QuickSetupModal />);
    fireEvent.click(screen.getByRole("button", { name: "Save As" }));
    const dialog = screen.getByRole("dialog", { name: "Save show" });
    expect(within(dialog).getByRole("heading", {name:"Name Empty Show"})).toBeInTheDocument();
    const titleBar = dialog.querySelector(".ui-modal-titlebar") as HTMLElement;
    expect(within(titleBar).getByRole("button", { name: "Name Empty Show" })).toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "Rename Show" })).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "Cancel" })).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Or replace an existing Latest Autosave")).not.toBeInTheDocument();
    fireEvent.change(within(dialog).getByLabelText("Show name"), { target: { value: "Opening Night" } });
    await waitFor(() => expect(within(titleBar).getByRole("button", {name:"Name Empty Show"})).toBeEnabled());
    fireEvent.click(within(titleBar).getByRole("button", { name: "Name Empty Show" }));
    await waitFor(() => expect(mocks.server.saveShowAs).toHaveBeenCalledWith("Opening Night", {baseShow: false, latest: false}));
  });

  it("opens a separate revision chooser and loads named history as a copy", async () => {
    render(<QuickSetupModal />);
    fireEvent.click(screen.getByRole("button", {name:/^Load$/}));
    fireEvent.click(await screen.findByRole("button", {name:"Revisions for Tour"}));
    const revisions = screen.getByRole("dialog", {name:"Revisions for Tour"});
    await waitFor(() => expect(within(revisions).getByRole("row", {name:/Approved focus/})).toBeInTheDocument());
    expect(revisions.querySelector("tbody tr:first-child")).toHaveTextContent("Latest Autosave");
    for (const row of revisions.querySelectorAll("tbody tr")) {
      expect(within(row as HTMLElement).getByRole("button", {name:/^Load$/})).toBeInTheDocument();
      expect(within(row as HTMLElement).getByRole("button", {name:"Partial Load"})).toBeInTheDocument();
    }
    fireEvent.click(within(within(revisions).getByRole("row", {name:/Approved focus/})).getByRole("button", {name:/^Load$/}));
    await waitFor(() => expect(mocks.server.openShowRevision).toHaveBeenCalledWith("original",3));
  });

  it("offers sources in the title without folder creation or MVR and keeps MVR in New Show", async () => {
    render(<QuickSetupModal />);
    fireEvent.click(screen.getByRole("button", {name:/^Load$/}));
    const browser = screen.getByRole("dialog", {name:"Load show"});
    await waitFor(() => expect(within(browser).getByRole("button", {name:"Source: Internal"})).toBeEnabled());
    expect(within(browser).getByRole("heading", {name:"Shows /"})).toBeInTheDocument();
    const source = within(browser).getByRole("button", {name:"Source: Internal"});
    expect(source.closest(".ui-title-chrome")).not.toBeNull();
    expect(within(browser).queryByRole("button", {name:"Create New Folder"})).not.toBeInTheDocument();
    fireEvent.click(source);
    for (const name of ["Internal", "USB (No drives connected)", "Network"]) expect(screen.getByRole("menuitem", {name})).toBeInTheDocument();
    fireEvent.click(screen.getByRole("menuitem", {name:"Internal"}));
    expect(within(browser).queryByRole("button",{name:/MVR/})).not.toBeInTheDocument();
    fireEvent.click(within(browser).getByRole("button", {name:"Close Load Show"}));
    fireEvent.click(screen.getByRole("button", {name:"New Show"}));
    expect(within(screen.getByRole("dialog",{name:"New show"})).getByRole("button",{name:"Load from MVR"})).toBeInTheDocument();
  });

  it("keeps an orphaned revision copy usable without an overwrite-original action", () => {
    mocks.server.shows = mocks.server.shows.filter((show) => show.id !== "original");
    render(<QuickSetupModal />);
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    const save = screen.getByRole("dialog", { name: "Save revision copy" });
    expect(save).toHaveTextContent("original show is no longer available");
    expect(within(save).queryByRole("button", { name: "Overwrite Original Show" })).not.toBeInTheDocument();
  });
});

describe("QuickSetupModal operator actions", () => {
  it("locks the desk directly from the Show menu and closes the menu", async () => {
    mocks.lockDesk.mockResolvedValue(undefined);
    render(<QuickSetupModal />);

    fireEvent.click(screen.getByRole("button", { name: "Lock Desk" }));

    await waitFor(() => expect(mocks.lockDesk).toHaveBeenCalledOnce());
    expect(mocks.dispatch).toHaveBeenCalledWith({
      type: "SET_MODAL",
      modal: "setupOpen",
      value: false,
    });
  });

  it("opens the hidden DMX built-in from the Show menu", () => {
    render(<QuickSetupModal />);

    fireEvent.click(screen.getByRole("button", { name: "DMX" }));

    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "OPEN_BUILTIN", kind: "dmx" });
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_MODAL", modal: "setupOpen", value: false });
  });

	it("keeps show navigation ordered as Patch, DMX, Scheduler, then Setup", () => {
		render(<QuickSetupModal />);
		const navigation = screen
			.getByRole("button", { name: "Show Patch" })
			.closest(".show-navigation-primary");
		expect(navigation).not.toBeNull();
		expect(
			within(navigation as HTMLElement)
				.getAllByRole("button")
				.map((button) => button.textContent?.trim()),
		).toEqual(["▦Show Patch", "◉DMX", "▣Scheduler", "⚙Enter Setup"]);

		fireEvent.click(screen.getByRole("button", { name: "Scheduler" }));
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "OPEN_BUILTIN",
			kind: "scheduler",
		});
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_MODAL",
			modal: "setupOpen",
			value: false,
		});
	});
	it("defers outer Quick Setup Escape to the MVR read guard and keeps inspection cancellation real", async () => {
		let finishRead: (content: Blob) => void = () => {};
		let finishPreview: (value: unknown) => void = () => {};
		mocks.openFileManagerPicker.mockResolvedValue([
			{ rootId: "shows", entry: { name: "rig.mvr", path: "rig.mvr" } },
		]);
		mocks.files.fileContent.mockImplementation(
			() =>
				new Promise((resolve) => {
					finishRead = resolve;
				}),
		);
		mocks.server.previewMvr.mockImplementation(
			() =>
				new Promise((resolve) => {
					finishPreview = resolve;
				}),
		);
		render(<QuickSetupModal />);
		fireEvent.click(screen.getByRole("button", { name: "New Show" }));
		fireEvent.click(screen.getByRole("button", { name: "Load from MVR" }));
		const progress = await screen.findByRole("status", {
			name: "MVR operation progress",
		});
		expect(progress).toHaveTextContent("Loading selected MVR file…");
		fireEvent.keyDown(progress, { key: "Escape" });
		expect(
			screen.getByRole("dialog", { name: "MVR import" }),
		).toBeInTheDocument();
		expect(mocks.dispatch).not.toHaveBeenCalledWith({
			type: "SET_MODAL",
			modal: "setupOpen",
			value: false,
		});
		expect(mocks.server.previewMvr).not.toHaveBeenCalled();
		finishRead(new Blob(["archive"]));
		await waitFor(() => expect(mocks.server.previewMvr).toHaveBeenCalledOnce());
		await waitFor(() =>
			expect(
				screen.getByRole("button", { name: "Cancel inspection" }),
			).toBeVisible(),
		);
		const signal = mocks.server.previewMvr.mock.calls[0][2] as AbortSignal;
		fireEvent.keyDown(
			screen.getByRole("status", { name: "MVR operation progress" }),
			{ key: "Escape" },
		);
		expect(signal.aborted).toBe(true);
		expect(screen.queryByRole("dialog", { name: "MVR import" })).toBeNull();
		finishPreview({
			token: "late",
			fixtures: [],
			address_conflicts: [],
			missing_profiles: [],
			scenery: 0,
			warnings: [],
		});
	});

});

function loadDeferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason: Error) => void;
	const promise = new Promise<T>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
}

describe("Show Load waiting lifecycle", () => {
	it("cancels a deferred catalogue through actual outer Escape and ignores its late result", async () => {
		const pending = loadDeferred<{ entries: never[] }>();
		mocks.files.fileEntries.mockReturnValue(pending.promise);
		render(<QuickSetupModal />);
		fireEvent.click(screen.getByRole("button", { name: /^Load$/ }));
		expect(
			screen.getByRole("status", { name: "Reading show catalogue" }),
		).toBeVisible();
		expect(
			screen.getByRole("progressbar", { name: "Reading show catalogue" }),
		).not.toHaveAttribute("value");
		fireEvent.keyDown(document, { key: "Escape" });
		expect(screen.queryByRole("dialog", { name: "Load show" })).toBeNull();
		await act(async () => pending.resolve({ entries: [] }));
		expect(screen.queryByRole("dialog", { name: "Load show" })).toBeNull();
		expect(mocks.server.openShow).not.toHaveBeenCalled();
	});
	it("protects one committing Load against actual outer Escape, title close and duplicate clicks", async () => {
		const pending = loadDeferred<boolean>();
		mocks.server.openShow.mockReset().mockReturnValue(pending.promise);
		render(<QuickSetupModal />);
		fireEvent.click(screen.getByRole("button", { name: /^Load$/ }));
		const browser = screen.getByRole("dialog", { name: "Load show" });
		const load = await within(browser).findAllByRole("button", {
			name: "Load Latest",
		});
		fireEvent.click(load[0]);
		fireEvent.click(load[0]);
		expect(
			screen.getByRole("status", { name: "Loading selected show" }),
		).toBeVisible();
		expect(
			within(browser).getByRole("button", { name: "Close Load Show" }),
		).toBeDisabled();
		fireEvent.keyDown(document, { key: "Escape" });
		fireEvent.click(
			within(browser).getByRole("button", { name: "Close Load Show" }),
		);
		expect(browser).toBeInTheDocument();
		expect(mocks.server.openShow).toHaveBeenCalledOnce();
		await act(async () => pending.resolve(true));
		await waitFor(() =>
			expect(screen.queryByRole("dialog", { name: "Load show" })).toBeNull(),
		);
	});
	it("offers local Retry for a failed load without changing the selected source", async () => {
		mocks.server.openShow
			.mockReset()
			.mockRejectedValueOnce(new Error("Selected show could not be read"))
			.mockResolvedValueOnce(true);
		render(<QuickSetupModal />);
		fireEvent.click(screen.getByRole("button", { name: /^Load$/ }));
		const browser = screen.getByRole("dialog", { name: "Load show" });
		const load = await within(browser).findAllByRole("button", {
			name: "Load Latest",
		});
		fireEvent.click(load[0]);
		expect(await within(browser).findByRole("alert")).toHaveTextContent(
			"Selected show could not be read",
		);
		fireEvent.click(within(browser).getByRole("button", { name: "Retry" }));
		await waitFor(() => expect(mocks.server.openShow).toHaveBeenCalledTimes(2));
		expect(mocks.server.openShow.mock.calls[1]).toEqual(
			mocks.server.openShow.mock.calls[0],
		);
	});
});

describe("Show Load read and preparation recovery", () => {
	it("recovers a failed catalogue locally without losing its source", async () => {
		mocks.files.fileRoots.mockRejectedValueOnce(
			new Error("Catalogue temporarily unavailable"),
		);
		render(<QuickSetupModal />);
		fireEvent.click(screen.getByRole("button", { name: /^Load$/ }));
		const browser = screen.getByRole("dialog", { name: "Load show" });
		expect(await within(browser).findByRole("alert")).toHaveTextContent(
			"Catalogue temporarily unavailable",
		);
		expect(
			within(browser).getByRole("button", { name: "Source: Internal" }),
		).toBeEnabled();
		fireEvent.click(within(browser).getByRole("button", { name: "Retry" }));
		expect(
			await within(browser).findAllByRole("button", { name: "Load Latest" }),
		).toHaveLength(2);
	});
	it("reports an absent desk instead of leaving Network perpetually busy", async () => {
		mocks.lifecycleAvailable = false;
		render(<QuickSetupModal />);
		fireEvent.click(screen.getByRole("button", { name: /^Load$/ }));
		const browser = screen.getByRole("dialog", { name: "Load show" });
		await waitFor(() =>
			expect(
				within(browser).getByRole("button", { name: "Source: Internal" }),
			).toBeEnabled(),
		);
		fireEvent.click(
			within(browser).getByRole("button", { name: "Source: Internal" }),
		);
		fireEvent.click(screen.getByRole("menuitem", { name: "Network" }));
		expect(await within(browser).findByRole("alert")).toHaveTextContent(
			"Desk is disconnected",
		);
		expect(
			within(browser).getByRole("button", { name: "Retry" }),
		).toBeEnabled();
		expect(within(browser).queryByRole("progressbar")).toBeNull();
	});
	it("cancels revision reading and ignores the stale response", async () => {
		const pending =
			loadDeferred<
				Array<{
					revision: number;
					name: string;
					created_at: string;
					show_id: string;
				}>
			>();
		mocks.server.listShowRevisions.mockReturnValue(pending.promise);
		render(<QuickSetupModal />);
		fireEvent.click(screen.getByRole("button", { name: /^Load$/ }));
		fireEvent.click(
			await screen.findByRole("button", { name: "Revisions for Tour" }),
		);
		expect(
			screen.getByRole("status", { name: "Reading named revisions" }),
		).toBeVisible();
		fireEvent.keyDown(document, { key: "Escape" });
		expect(
			screen.queryByRole("dialog", { name: "Revisions for Tour" }),
		).toBeNull();
		await act(async () =>
			pending.resolve([
				{
					revision: 9,
					name: "Stale revision",
					created_at: "",
					show_id: "original",
				},
			]),
		);
		expect(
			within(screen.getByRole("dialog", { name: "Load show" })).queryByText(
				/Stale revision/,
			),
		).toBeNull();
		expect(
			screen.getByRole("dialog", { name: "Load show" }),
		).toBeInTheDocument();
	});
	it("guards Partial Load preparation and opens its preview once only after completion", async () => {
		const pending = loadDeferred<{ id: string; name: string; path: string }>();
		mocks.server.prepareShowRevision
			.mockReset()
			.mockReturnValue(pending.promise);
		render(<QuickSetupModal />);
		fireEvent.click(screen.getByRole("button", { name: /^Load$/ }));
		fireEvent.click(
			await screen.findByRole("button", { name: "Revisions for Tour" }),
		);
		const revisions = screen.getByRole("dialog", {
			name: "Revisions for Tour",
		});
		const row = await within(revisions).findByRole("row", {
			name: /Approved focus/,
		});
		const partial = within(row).getByRole("button", { name: "Partial Load" });
		fireEvent.click(partial);
		fireEvent.click(partial);
		expect(
			screen.getByRole("status", { name: "Preparing selected show" }),
		).toBeVisible();
		fireEvent.keyDown(document, { key: "Escape" });
		expect(revisions).toBeInTheDocument();
		expect(mocks.server.prepareShowRevision).toHaveBeenCalledOnce();
		await act(async () =>
			pending.resolve({
				id: "prepared-copy",
				name: "Prepared",
				path: "prepared.show",
			}),
		);
		await waitFor(() =>
			expect(screen.queryByRole("dialog", { name: "Load show" })).toBeNull(),
		);
	});
});
