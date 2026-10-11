import { describe, expect, it, vi } from "vitest";
import { BrowserCrossSurface } from "./crossSurfaceScenario";
import type { ApiDriver, Session } from "../core/api";

function rig() {
    const session = { session_id: "session", desk: { id: "desk" } } as Session;
    const request = vi.fn().mockResolvedValue({ command_page: 7, selected_surface_id: "pane:display" });
    const api = { session, request } as unknown as ApiDriver;
    const helper = new BrowserCrossSurface(api, {} as never, {} as never, {} as never, {} as never, () => "show");
    return { request, session, helper: helper as unknown as {
        setDeskPage(session: Session, page: number): Promise<void>;
        activePlayback(address: {page:number;position:number}): Promise<unknown>;
    } };
}

describe("cross-surface bench canonical authority", () => {
    it("changes the visibly selected surface page instead of a retired global page", async () => {
        const { request, session, helper } = rig();
        await helper.setDeskPage(session, 99);
        expect(request.mock.calls[1]).toEqual(["POST", "/api/v2/playback-surfaces/pane%3Adisplay/page", {page:99}, true, undefined, {showId:"show",deskId:"desk"}]);
    });
    it("queries the original exact Playback address, not a scalar or active owner's address", async () => {
        const { request, helper } = rig();
        request.mockResolvedValue({ projections: [{ runtime: {enabled:true,current:{number:"2"}} }] });
        expect(await helper.activePlayback({page:99,position:1000})).toMatchObject({enabled:true,current_cue_number:"2"});
        expect(request.mock.calls[0]?.[2]).toEqual({identities:[{kind:"playback",page:99,position:1000}]});
    });
    it.each([0,100,1.5])("rejects page %s before any request", async page => {
        const { request,session,helper } = rig();
        await expect(helper.setDeskPage(session,page)).rejects.toThrow("1–99");
        expect(request).not.toHaveBeenCalled();
    });
});
