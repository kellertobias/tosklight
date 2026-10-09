import {act,cleanup,renderHook,render,screen,fireEvent} from "@testing-library/react";
import {afterEach,describe,expect,it,vi} from "vitest";
import {MvrImportPreview} from "./QuickSetupDialogs";
import {MvrInspectionProgress} from "./MvrInspectionProgress";
import {useMvrController} from "./QuickSetupModal";
import type {ShowLifecycleActions} from "../../features/showLifecycle/ShowLifecycleContext";

afterEach(()=>{cleanup();vi.useRealTimers();});
function deferred<T>() {let resolve!:(value:T)=>void;const promise=new Promise<T>(done=>resolve=done);return {promise,resolve};}
const preview={token:"late",fixtures:[],address_conflicts:[],missing_profiles:[],scenery:0,warnings:[]};
function setup(promise:Promise<unknown>) {
    const lifecycle={previewMvr:vi.fn().mockReturnValue(promise),applyMvr:vi.fn()};
    const hook=renderHook(()=>useMvrController(lifecycle as unknown as ShowLifecycleActions));
    act(()=>hook.result.current.setMvrMode("new"));
    return {...hook,lifecycle};
}
describe("MVR inspection lifecycle",()=>{
    it("exposes honest active inspection state and passes an abort signal",async()=>{
        const task=deferred<typeof preview>();const test=setup(task.promise);
        let running!:Promise<void>;
        act(()=>{running=test.result.current.inspectMvr(new File(["archive"],"Large rig.mvr"));});
        expect(test.result.current).toMatchObject({mvrBusy:true,mvrOperation:"inspect",mvrInspectionFile:{name:"Large rig.mvr",size:7}});
        expect(test.lifecycle.previewMvr.mock.calls[0][2]).toBeInstanceOf(AbortSignal);
        await act(async()=>{task.resolve(preview);await running;});
    });
    it("cancels inspection and ignores its late result without applying or changing shows",async()=>{
        const task=deferred<typeof preview>();const test=setup(task.promise);let running!:Promise<void>;
        act(()=>{running=test.result.current.inspectMvr(new File(["rig"],"Rig.mvr"));});
        act(()=>test.result.current.setMvrMode(null));
        await act(async()=>{task.resolve(preview);await running;});
        expect(test.result.current.mvrPreview).toBeNull();
        expect(test.result.current.mvrBusy).toBe(false);
        expect(test.lifecycle.applyMvr).not.toHaveBeenCalled();
    });
    it("keeps failed inspection local and offers a clean retry",async()=>{
        const test=setup(Promise.reject(new Error("invalid archive")));
        await act(async()=>{await test.result.current.inspectMvr(new File(["bad"],"Bad.mvr")).catch(()=>{});});
        expect(test.result.current).toMatchObject({mvrBusy:false,mvrError:expect.stringMatching(/invalid archive/)});
        test.lifecycle.previewMvr.mockResolvedValueOnce(preview);
        await act(async()=>{await test.result.current.inspectMvr(new File(["good"],"Good.mvr"));});
        expect(test.result.current.mvrPreview?.token).toBe("late");
        expect(test.result.current).toMatchObject({mvrError:""});
    });
});

describe("cancelled and superseded MVR previews",()=>{
    it("cannot apply a cancelled preview through a retained callback",async()=>{
        const test=setup(Promise.resolve(preview));
        await act(async()=>{await test.result.current.inspectMvr(new File(["rig"],"Rig.mvr"));});
        const staleApply=test.result.current.applyMvr;
        act(()=>test.result.current.setMvrMode(null));
        await act(async()=>{await staleApply();});
        expect(test.lifecycle.applyMvr).not.toHaveBeenCalled();
    });
    it("keeps the newer inspection busy when cancelled work completes late",async()=>{
        const first=deferred<typeof preview>();const second=deferred<typeof preview>();const test=setup(first.promise);
        let oldRun!:Promise<void>;let newRun!:Promise<void>;
        act(()=>{oldRun=test.result.current.inspectMvr(new File(["old"],"Old.mvr"));});
        const oldSignal=test.lifecycle.previewMvr.mock.calls[0][2];
        act(()=>test.result.current.setMvrMode(null));
        expect(oldSignal.aborted).toBe(true);
        test.lifecycle.previewMvr.mockReturnValueOnce(second.promise);
        act(()=>{test.result.current.setMvrMode("new");});
        act(()=>{newRun=test.result.current.inspectMvr(new File(["new"],"New.mvr"));});
        await act(async()=>{first.resolve(preview);await oldRun;});
        expect(test.result.current).toMatchObject({mvrBusy:true,mvrOperation:"inspect",mvrPreview:null});
        await act(async()=>{second.resolve({...preview,token:"new"});await newRun;});
        expect(test.result.current.mvrPreview?.token).toBe("new");
    });
});
describe("MVR visible progress",()=>{
    it("shows honest elapsed indeterminate progress and permits inspection cancellation",()=>{
        vi.useFakeTimers();vi.setSystemTime(10_000);const cancel=vi.fn();
        render(<MvrInspectionProgress operation="inspect" startedAt={10_000} file={{name:"Rig.mvr",size:7_441_493}} onCancel={cancel}/>);
        expect(screen.getByRole("progressbar").hasAttribute("value")).toBe(false);
        act(()=>vi.advanceTimersByTime(3000));
        expect(screen.getByText(/Rig.mvr.*7.44 MB.*Elapsed 3 s/)).toBeTruthy();
        fireEvent.click(screen.getByRole("button",{name:"Cancel inspection"}));
        expect(cancel).toHaveBeenCalledOnce();
    });
    it("does not offer cancellation during committed application",()=>{
        render(<MvrInspectionProgress operation="apply" startedAt={Date.now()} file={null} onCancel={vi.fn()}/>);
        expect(screen.queryByRole("button",{name:"Cancel inspection"})).toBeNull();
        expect(screen.getByText(/Wait for completion/)).toBeTruthy();
    });
});


describe("explicit immutable MVR profile identity consent",()=>{
    const conflicting={...preview, profile_conflicts:[{profile_id:"profile",revision:1,name:"JB-Lighting JBLED A7",fixtures:["a","b"]}]};
    it("requires explicit consent and sends that intent without changing fixture address decisions",async()=>{
        const test=setup(Promise.resolve(conflicting));
        await act(async()=>{await test.result.current.inspectMvr(new File(["rig"],"Rig.mvr"));});
        expect(test.result.current.copyConflictingProfiles).toBe(false);
        await act(async()=>{await test.result.current.applyMvr();});
        expect(test.lifecycle.applyMvr).not.toHaveBeenCalled();
        act(()=>test.result.current.setCopyConflictingProfiles(true));
        await act(async()=>{await test.result.current.applyMvr();});
        expect(test.lifecycle.applyMvr).toHaveBeenCalledWith("late",expect.objectContaining({copy_conflicting_profiles:true,resolutions:{}}));
    });
    it("cannot reuse consent or a retained apply callback after cancellation and reinspection",async()=>{
        const test=setup(Promise.resolve(conflicting));
        await act(async()=>{await test.result.current.inspectMvr(new File(["first"],"First.mvr"));});
        act(()=>test.result.current.setCopyConflictingProfiles(true));
        const staleApply=test.result.current.applyMvr;
        act(()=>test.result.current.setMvrMode(null));
        await act(async()=>{await staleApply();});
        expect(test.lifecycle.applyMvr).not.toHaveBeenCalled();
        act(()=>test.result.current.setMvrMode("new"));
        test.lifecycle.previewMvr.mockResolvedValueOnce({...conflicting,token:"new"});
        await act(async()=>{await test.result.current.inspectMvr(new File(["second"],"Second.mvr"));});
        expect(test.result.current.copyConflictingProfiles).toBe(false);
        await act(async()=>{await test.result.current.applyMvr();});
        expect(test.lifecycle.applyMvr).not.toHaveBeenCalled();
    });
});


it("renders immutable collision details and a disabled apply button until checked",()=>{
    const consent=vi.fn();
    const model={mvr:{mvrPreview:{...preview,profile_conflicts:[{profile_id:"profile",revision:1,name:"JB-Lighting JBLED A7",fixtures:["a","b"]}]},mvrMode:"new",mvrName:"Imported rig",mvrBusy:false,copyConflictingProfiles:false,setCopyConflictingProfiles:consent,setMvrName:vi.fn(),applyMvr:vi.fn()}};
    render(<MvrImportPreview model={model as unknown as Parameters<typeof MvrImportPreview>[0]["model"]}/>);
    expect(screen.getByText(/JB-Lighting JBLED A7.*revision 1.*2 fixtures/)).toBeTruthy();
    expect(screen.getByText(/Existing profiles and unrelated fixtures stay unchanged/)).toBeTruthy();
    expect(screen.getByRole("button",{name:"Create and Open Show"})).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox",{name:"Import conflicting profiles as new identities"}));
    expect(consent).toHaveBeenCalledWith(true);
});
