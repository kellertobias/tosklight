import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {useState} from "react";
import {afterEach,describe,expect,it} from "vitest";
import type {FixtureMode} from "../fixtureProfile";
import {blankMode,blankHead} from "../sheet/fixtureProfileModel/defaults";
import {blankChannel} from "../sheet/fixtureProfileModel/channels";
import {colorPhysicalErrors,createHeadOpticalPath,withOpticalPath,opticalPathWarnings,nativeColorFunctionAllowed} from "../sheet/fixtureProfileModel/colorPhysical";
import {setRowLevel,slotRows} from "./channelSlots";
import {HeadsEditor} from "./heads";
import {ColorPhysicalEditor} from "./colorPhysicalEditor";

afterEach(cleanup);
function example() {
 const mode=blankMode(); mode.splits[0].footprint=3;
 for (const [index,name] of ["color.cyan","color.magenta","color.wheel.1"].entries()) {
  const c=blankChannel(mode,1); c.fixture_attribute=name; c.attribute=index===0 ? "color.red" : index===1 ? "color.green" : name;
  c.functions=[{id:crypto.randomUUID(),name,attribute:c.attribute,dmx_from:0,dmx_to:255,priority:0,behavior:{type:"continuous",physical_min:0,physical_max:1,unit:null}}];
  mode.channels.push(c);
 }
 return mode;
}
function Harness() {
 const [mode,setMode]=useState(example);
 return <><ColorPhysicalEditor mode={mode} onChange={setMode}/><output data-testid="mode">{JSON.stringify(mode)}</output></>;
}
const current=()=>JSON.parse(screen.getByTestId("mode").textContent!) as FixtureMode;

describe("physical optical path authoring",()=>{
 it("requires newly added White outside the legacy systems to join Color ownership",()=>{
  const mode=example(),path=createHeadOpticalPath(mode,mode.heads[0].id);
  const white=blankChannel(mode,1);white.fixture_attribute="color.white";white.attribute="color.white";white.default_raw=255;
  white.functions=[{...mode.channels[0].functions[0],id:crypto.randomUUID(),attribute:"color.white"}];
  mode.channels.push(white);
  expect(colorPhysicalErrors(withOpticalPath(mode,path))).toContain("Physical path omits a declared native Color channel or existing color system control");
 });
 it("requires explicit Color classification for fixed fixture-control ranges",()=>{
  const mode=example(),path=createHeadOpticalPath(mode,mode.heads[0].id),channel=mode.channels[2],fn=channel.functions[0];
  channel.fixture_attribute="fixture.control";channel.attribute="fixture.control";fn.attribute="fixture.control";
  fn.behavior={type:"fixed",semantic_id:"reset",label:"Reset",raw_value:128};
  path.filters=[{id:crypto.randomUUID(),name:"Ambiguous control",binding:{channel_id:channel.id,function_id:fn.id},transmission:{type:"unknown"},provenance:{quality:"unknown",revision:0}}];
  expect(nativeColorFunctionAllowed(channel,fn)).toBe(false);
  expect(colorPhysicalErrors(withOpticalPath(mode,path))).toContain("Service or unclassified fixture-control functions cannot be optical color bindings");
  fn.attribute="color.temperature";fn.behavior={type:"fixed",semantic_id:"balance6500",label:"6500 K",raw_value:128};
  expect(nativeColorFunctionAllowed(channel,fn)).toBe(true);
  expect(colorPhysicalErrors(withOpticalPath(mode,path))).toEqual([]);
 });
 it("does not assume shared plate RGB affects a separate white beam head",()=>{
  const mode=example();mode.heads[0].master_shared=true;
  const head=blankHead(1);mode.heads.push(head);
  const path=createHeadOpticalPath(mode,head.id);
  expect(path.controls).toEqual([]);
  expect(colorPhysicalErrors(withOpticalPath(mode,path))).toEqual([]);
 });

 it("authors native controls without pretending unknown serial appearance is measured",()=>{
  render(<Harness/>);
  expect(current().color_physical).toBeUndefined();
  expect(screen.getByText(/do not yet change live color matching/)).toBeInTheDocument();
  fireEvent.click(screen.getByText("Main · No physical path"));
  fireEvent.click(screen.getByRole("button",{name:"Create physical path for Main"}));
  const path=current().color_physical!.paths[0];
  expect(path.controls).toHaveLength(3); expect(path.source).toEqual({type:"unknown"});
  expect(colorPhysicalErrors(current())).toEqual([]);
  expect(screen.getByText("Source appearance is unknown.")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button",{name:"Add optical filter"}));
  fireEvent.change(screen.getByLabelText("Filter 1 name"),{target:{value:"Cyan flag"}});
  fireEvent.click(screen.getByRole("button",{name:"Add optical filter"}));
  fireEvent.change(screen.getByLabelText("Filter 2 name"),{target:{value:"Magenta flag"}});
  const before=current().color_physical!.paths[0].filters;
  fireEvent.click(screen.getByRole("button",{name:"Move filter 2 earlier"}));
  expect(current().color_physical!.paths[0].filters.map((f)=>f.id)).toEqual([before[1].id,before[0].id]);
  expect(screen.getByText(/Filter transmission is unknown/)).toBeInTheDocument();
  expect(colorPhysicalErrors(current())).toEqual([]);
 });
 it("validates measured provenance and preserves unknown XYZ until explicitly authored",async()=>{
  render(<Harness/>);fireEvent.click(screen.getByText("Main · No physical path"));
  fireEvent.click(screen.getByRole("button",{name:"Create physical path for Main"}));
  fireEvent.click(screen.getByRole("button",{name:/Optical source/}));fireEvent.click(await screen.findByRole("option",{name:"Fixed lamp source"}));
  expect(current().color_physical!.paths[0].source).toMatchObject({xyz:null,provenance:{quality:"unknown"}});
  fireEvent.click(screen.getByLabelText("Source XYZ known"));
  fireEvent.change(screen.getByLabelText("Source Y"),{target:{value:"1"}});
  fireEvent.click(screen.getByRole("button",{name:/Source quality/}));fireEvent.click(await screen.findByRole("option",{name:"Measured"}));
  expect(screen.getByRole("alert")).toHaveTextContent("provenance is invalid");
  fireEvent.change(screen.getByLabelText("Source evidence"),{target:{value:"Bench spectrometer"}});
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Optical model revision"),{target:{value:"7"}});
  expect(current().color_physical!.revision).toBe(7);
 });
 it("rejects deleted/native foreign functions and service recipes instead of falling back to aliases",()=>{
  const mode=example(),path=createHeadOpticalPath(mode,mode.heads[0].id);
  path.filters=[{id:crypto.randomUUID(),name:"Wheel",binding:{channel_id:mode.channels[2].id,function_id:mode.channels[0].functions[0].id},transmission:{type:"unknown"},provenance:{quality:"unknown",revision:0}}];
  expect(colorPhysicalErrors(withOpticalPath(mode,path))).toContain("Optical binding references a missing native function");
  path.filters=[]; path.measurements=[{xyz:{x:0,y:0,z:0},provenance:{quality:"unknown",revision:0},recipe:mode.channels.map((c)=>({channel_id:c.id,function_id:c.functions[0].id,raw:0}))}];
  expect(colorPhysicalErrors(withOpticalPath(mode,path))).toEqual([]);
  mode.channels[0].functions[0].behavior={type:"control",action_id:"reset"};
  expect(colorPhysicalErrors(withOpticalPath(mode,path))).toContain("Service control functions cannot be captured as native color");
  mode.channels.pop();expect(colorPhysicalErrors(withOpticalPath(mode,path))).toContain("Physical color control references a missing channel");
 });
 it("does not silently change calibrated raw-byte meaning when joining channel rows",()=>{
  const mode=example();const path=createHeadOpticalPath(mode,mode.heads[0].id);const configured=withOpticalPath(mode,path);
  const result=setRowLevel(configured,1,slotRows(configured,1)[0],1);
  expect(result.error).toContain("physical Color path"); expect(result.mode).toBeUndefined();
  expect(configured.channels[0].resolution).toBe("u8");
 });
 it("warns about unmodeled native ownership even when a source has known XYZ",()=>{
  const mode=example(),path=createHeadOpticalPath(mode,mode.heads[0].id);
  path.source={type:"fixed",xyz:{x:1,y:1,z:1},spectrum:[],provenance:{quality:"estimated",revision:0}};
  expect(opticalPathWarnings(path,mode)).toContain("Some native Color controls or functions have no physical appearance model; their output remains unknown.");
 });
 it("keeps channel-less heads with physical paths reachable for repair",()=>{
  const mode=example();const head=blankHead(1);mode.heads.push(head);
  const configured=withOpticalPath(mode,createHeadOpticalPath(mode,head.id));
  render(<HeadsEditor mode={configured} onChange={()=>{throw new Error("must not delete referenced head");}}/>);
  expect(screen.getByRole("button",{name:`Remove ${head.name}`})).toBeDisabled();
  expect(screen.getByText(/physical optical path in Color before deleting/)).toBeInTheDocument();
 });
 it("rejects positive emitter drive values that underflow Rust storage",()=>{
  const mode=example(),path=createHeadOpticalPath(mode,mode.heads[0].id);
  path.source={type:"additive",emitters:[{id:crypto.randomUUID(),name:"Red",binding:{channel_id:mode.channels[0].id,function_id:mode.channels[0].functions[0].id},xyz:null,spectrum:[],band:"visible",maximum_level:1e-46,response_exponent:1,provenance:{quality:"unknown",revision:0}}]};
  expect(colorPhysicalErrors(withOpticalPath(mode,path))).toContain("Optical emitter identity, data or provenance is invalid");
 });
 it("keeps spectral filters and exact U32 recipes through a serialized round trip",()=>{
  const mode=example(),path=createHeadOpticalPath(mode,mode.heads[0].id);const c=mode.channels[2];c.resolution="u32";c.functions[0].dmx_to=0xffff_ffff;
  path.filters=[{id:crypto.randomUUID(),name:"Wheel",binding:{channel_id:c.id,function_id:c.functions[0].id},provenance:{quality:"measured",source:"Test",revision:2},transmission:{type:"spectral",samples:[{raw_from:0xffff_fffe,raw_to:0xffff_ffff,spectrum:[{wavelength_nm:380,value:0.2},{wavelength_nm:780,value:0.8}]}]}}];
  const value=withOpticalPath(mode,path);expect(colorPhysicalErrors(value)).toEqual([]);
  const roundtrip=JSON.parse(JSON.stringify(value)) as FixtureMode;expect(roundtrip.color_physical).toEqual(value.color_physical);
  const filter=roundtrip.color_physical!.paths[0].filters[0];if(filter.transmission.type==="spectral")filter.transmission.samples[0].spectrum[1].value=1.1;
  expect(colorPhysicalErrors(roundtrip)).toContain("Filter spectra need sorted non-overlapping native ranges and valid transmission");
 });
});


it("keeps UV control identity separate from unknown or visible appearance during template adoption", () => {
 const mode=example(),channel=mode.channels[0];
 channel.fixture_attribute="color.uv"; channel.attribute="color.uv"; channel.functions[0].attribute="color.uv";
 for (const [visible,measured] of [[false,false],[true,false],[false,true]]) {
  mode.color_systems=[{head_id:mode.heads[0].id,correction_matrix:[[1,0,0],[0,1,0],[0,0,1]],calibration:{status:measured ? "measured" : "nominal",revision:0,source:"Synthetic template"},system:{type:"additive",emitters:[{channel_id:channel.id,name:"UV",xyz:visible ? {x:0.01,y:0.001,z:0.04} : {x:0,y:0,z:0},maximum_level:1,response_curve:1,visible}]}}];
  const path=createHeadOpticalPath(mode,mode.heads[0].id);
  expect(path.source.type).toBe("additive"); if(path.source.type!=="additive")throw new Error("missing additive source");
  expect(path.source.emitters[0].band).toBe("ultraviolet");
  expect(path.source.emitters[0].xyz).toEqual(visible ? {x:0.01,y:0.001,z:0.04} : measured ? {x:0,y:0,z:0} : null);
  expect(path.source.emitters[0].binding).toEqual({channel_id:channel.id,function_id:channel.functions[0].id});
  expect(colorPhysicalErrors(withOpticalPath(mode,path))).toEqual([]);
  const restored=JSON.parse(JSON.stringify(path));expect(restored.source.emitters).toEqual(path.source.emitters);
 }
});
