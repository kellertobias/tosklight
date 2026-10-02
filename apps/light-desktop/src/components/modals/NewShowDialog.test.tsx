import { cleanup,fireEvent,render,screen,waitFor,within } from "@testing-library/react";
import { afterEach,expect,it,vi } from "vitest";
import { NewShowDialog } from "./NewShowDialog";
import type { QuickSetupModel } from "./QuickSetupModal";

afterEach(cleanup);

function fixture() {
    const lifecycle={
        shows:[{id:"base",name:"House rig",is_base_show:true,description:"Permanent positions",updated_at:"2026-09-27T13:00:00Z"},
            {id:"ordinary",name:"Ordinary show",updated_at:"2026-09-27T13:00:00Z"}],
        openCleanDefaultShow:vi.fn().mockResolvedValue(true),initializeEmptyShow:vi.fn().mockResolvedValue(true),
        setShowDescription:vi.fn().mockResolvedValue(undefined),
    };
    const model={authorities:{lifecycle},dialogs:{newShowOpen:true,setNewShowOpen:vi.fn()},mvr:{openMvrImport:vi.fn()}} as unknown as QuickSetupModel;
    return {model,lifecycle};
}

it("offers clean/default actions and creates an independent show from the table row",async()=>{
    const {model,lifecycle}=fixture();render(<NewShowDialog model={model}/>);
    const dialog=screen.getByRole("dialog",{name:"New show"});
    expect(within(dialog).getByRole("button",{name:"Load from MVR"})).toBeVisible();
    expect(within(dialog).getByRole("button",{name:"Load Clean Built-in Default"})).toBeVisible();
    expect(within(dialog).getByRole("button",{name:"Create Empty Show"})).toBeVisible();
    expect(within(dialog).getByRole("heading",{name:"Start from a base show"})).toBeVisible();
    const table=within(dialog).getByRole("table",{name:"Base shows"});
    expect(within(table).getByRole("columnheader",{name:"Description"})).toBeVisible();
    expect(within(table).getByText("Permanent positions")).toBeVisible();
    expect(within(table).queryByText("Ordinary show")).toBeNull();
    fireEvent.click(within(table).getByRole("button",{name:"Create show from House rig"}));
    await waitFor(()=>expect(lifecycle.initializeEmptyShow).toHaveBeenCalledWith("base"));
    expect(model.dialogs.setNewShowOpen).toHaveBeenCalledWith(false);
});

it("edits descriptions only after explicit save and exposes a failed save",async()=>{
    const {model,lifecycle}=fixture();lifecycle.setShowDescription.mockRejectedValue(new Error("Desk disconnected"));
    render(<NewShowDialog model={model}/>);
    fireEvent.click(screen.getByRole("button",{name:"Edit description for House rig"}));
    const editor=screen.getByRole("dialog",{name:"Description for House rig"});
    fireEvent.change(within(editor).getByRole("textbox",{name:"Show description"}),{target:{value:"House rig for 48 fixtures"}});
    expect(lifecycle.setShowDescription).not.toHaveBeenCalled();
    fireEvent.click(within(editor).getByRole("button",{name:"Save description"}));
    await waitFor(()=>expect(lifecycle.setShowDescription).toHaveBeenCalledWith("base","House rig for 48 fixtures"));
    expect(await within(editor).findByRole("alert")).toHaveTextContent("Desk disconnected");
    expect(screen.getByRole("dialog",{name:"Description for House rig"})).toBeVisible();
});
