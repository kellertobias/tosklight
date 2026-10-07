import { Button, CheckboxField, FormLayout, NumberField, SelectField, TextField } from "@tosklight/ui";
import type { FixtureMode, HeadOpticalPath, NativeColorBinding, OpticalProvenance, XyzValue } from "../fixtureProfile";
import { colorPhysicalErrors, createHeadOpticalPath, opticalPathWarnings, nativeColorFunctionAllowed, unknownOpticalProvenance, withOpticalPath } from "../sheet/fixtureProfileModel/colorPhysical";
import { uuid } from "../sheet/fixtureProfileModel/utilities";

function ProvenanceFields({label, value, onChange}: {label: string; value: OpticalProvenance; onChange(value: OpticalProvenance): void}) {
 return <FormLayout columns={3} minColumnWidth={180}>
  <SelectField label={`${label} quality`} ariaLabel={`${label} quality`} value={value.quality}
   options={[{value:"unknown",label:"Unknown"},{value:"estimated",label:"Estimated"},{value:"manufacturer",label:"Manufacturer"},{value:"measured",label:"Measured"}]}
   onChange={(quality) => onChange({...value, quality: quality as OpticalProvenance["quality"]})} />
  <TextField label={`${label} evidence`} value={value.source ?? ""} onChange={(e) => onChange({...value, source:e.target.value || null})} />
  <NumberField label={`${label} revision`} value={value.revision} min={0} max={0xffff_ffff} onChange={(e) => onChange({...value,revision:Number(e.target.value)})} />
 </FormLayout>;
}
function XyzFields({label, value, onChange}: {label: string; value: XyzValue | null; onChange(value: XyzValue | null): void}) {
 return <div>
  <CheckboxField label={`${label} XYZ known`} checked={value != null} onChange={(e) => onChange(e.target.checked ? {x:0,y:0,z:0} : null)} />
  {value && <FormLayout columns={3} minColumnWidth={120}>{(["x","y","z"] as const).map((axis) =>
   <NumberField key={axis} label={`${label} ${axis.toUpperCase()}`} min={0} allowDecimal step={0.001} value={value[axis]} onChange={(e) => onChange({...value,[axis]:Number(e.target.value)})} />
  )}</FormLayout>}
 </div>;
}
function BindingField({mode,path,label,value,onChange,continuous=false}: {mode:FixtureMode;path:HeadOpticalPath;label:string;value:NativeColorBinding;onChange(value:NativeColorBinding):void;continuous?:boolean}) {
 const options = mode.channels.filter((c) => path.controls.includes(c.id)).flatMap((channel) =>
  channel.functions.filter((f) => nativeColorFunctionAllowed(channel,f) && (!continuous || f.behavior.type === "continuous")).map((fn) => ({
   value: `${channel.id}/${fn.id}`, label:`${channel.fixture_attribute} · ${fn.name} · ${fn.dmx_from}–${fn.dmx_to}`,
  })));
 const selected = `${value.channel_id}/${value.function_id}`;
 if (!options.some((o) => o.value === selected)) options.unshift({value:selected,label:"Missing native function — select a replacement"});
 return <SelectField label={label} ariaLabel={label} value={selected} options={options} onChange={(v) => { const [channel_id,function_id] = v.split("/"); onChange({channel_id,function_id}); }} />;
}

function HeadPathEditor({mode,path,onChange}:{mode:FixtureMode;path:HeadOpticalPath;onChange(value:HeadOpticalPath):void}) {
 const source = path.source;
 const available = mode.channels.filter((c) => c.head_id === path.head_id || mode.heads.some((h) => h.id === c.head_id && h.master_shared));
 const bound = new Set([
  ...(source.type === "additive" ? source.emitters.map((e) => `${e.binding.channel_id}/${e.binding.function_id}`) : []),
  ...path.filters.map((f) => `${f.binding.channel_id}/${f.binding.function_id}`),
 ]);
 const candidate = (continuous: boolean) => available.filter((c) => path.controls.includes(c.id)).flatMap((c) =>
  c.functions.filter((f) => nativeColorFunctionAllowed(c,f) && (!continuous || f.behavior.type === "continuous") && !bound.has(`${c.id}/${f.id}`)).map((f) => ({channel_id:c.id,function_id:f.id})),
 )[0];
 return <div className="fixture-optical-path">
  <details><summary>Native Color controls ({path.controls.length})</summary>
   <p className="field-hint">Include every emitter, filter, wheel and color mode control. Functions retain their own identities; service/reset functions cannot be captured as Color.</p>
   <div className="fixture-optical-controls">{available.map((c) => <CheckboxField key={c.id} label={`${c.fixture_attribute} · ${c.resolution.toUpperCase()} · ${c.id.slice(0,8)}`} checked={path.controls.includes(c.id)} onChange={(e) => onChange({...path,controls:e.target.checked ? [...path.controls,c.id] : path.controls.filter((id) => id !== c.id)})} />)}
    {path.controls.filter((id) => !available.some((c) => c.id === id)).map((id) => <CheckboxField key={id} label={`Missing or foreign channel ${id} — remove reference`} checked onChange={() => onChange({...path,controls:path.controls.filter((value) => value !== id)})} />)}
   </div>
  </details>
  <SelectField label="Optical source" ariaLabel="Optical source" value={source.type} options={[{value:"unknown",label:"Unknown source"},{value:"fixed",label:"Fixed lamp source"},{value:"additive",label:"Additive emitters"}]}
   onChange={(type) => { if (type === source.type) return; onChange({...path,source:type === "fixed" ? {type,xyz:null,spectrum:[],provenance:unknownOpticalProvenance()} : type === "additive" ? {type,emitters:[]} : {type:"unknown"}}); }} />
  {source.type === "fixed" && <>
   <XyzFields label="Source" value={source.xyz} onChange={(xyz) => onChange({...path,source:{...source,xyz}})} />
   <ProvenanceFields label="Source" value={source.provenance} onChange={(provenance) => onChange({...path,source:{...source,provenance}})} />
   {!!source.spectrum.length && <p>Imported source spectrum: {source.spectrum.length} samples retained.</p>}
  </>}
  {source.type === "additive" && <>
   {source.emitters.map((emitter,index) => {
    const set = (patch: Partial<typeof emitter>) => onChange({...path,source:{...source,emitters:source.emitters.map((e,i) => i === index ? {...e,...patch} : e)}});
    return <fieldset key={emitter.id}><legend>Emitter {index+1}</legend>
     <TextField label={`Emitter ${index+1} name`} value={emitter.name} onChange={(e) => set({name:e.target.value})} />
     <BindingField mode={mode} path={path} label={`Emitter ${index+1} native function`} value={emitter.binding} continuous onChange={(binding) => set({binding})} />
     <XyzFields label={`Emitter ${index+1}`} value={emitter.xyz} onChange={(xyz) => set({xyz})} />
     <FormLayout columns={3} minColumnWidth={160}>
      <NumberField label={`Emitter ${index+1} maximum drive`} min={0} max={1} step={0.01} allowDecimal value={emitter.maximum_level} onChange={(e) => set({maximum_level:Number(e.target.value)})} />
      <NumberField label={`Emitter ${index+1} response exponent`} min={0} step={0.01} allowDecimal value={emitter.response_exponent} onChange={(e) => set({response_exponent:Number(e.target.value)})} />
      <SelectField label={`Emitter ${index+1} purpose`} ariaLabel={`Emitter ${index+1} purpose`} value={emitter.band} options={[{value:"visible",label:"Visible"},{value:"ultraviolet",label:"UV effect"},{value:"infrared",label:"Infrared"},{value:"other_non_visible",label:"Other non-visible"}]} onChange={(band) => set({band:band as typeof emitter.band})} />
      <CheckboxField label={`Emitter ${index+1} reversed native direction`} checked={emitter.native_reversed ?? false} onChange={(e) => set({native_reversed:e.target.checked})} />
     </FormLayout>
     {emitter.band === "ultraviolet" && <p className="field-hint">UV control does not require color measurements. XYZ describes any visible violet output; leave it unknown unless supported by data. UV strength and fluorescence cannot be inferred from XYZ.</p>}
     <ProvenanceFields label={`Emitter ${index+1}`} value={emitter.provenance} onChange={(provenance) => set({provenance})} />
     {!!emitter.spectrum.length && <p>Imported emitter spectrum: {emitter.spectrum.length} samples retained.</p>}
     <Button onClick={() => onChange({...path,source:{...source,emitters:source.emitters.filter((e) => e.id !== emitter.id)}})}>Remove emitter {index+1}</Button>
    </fieldset>;
   })}
   <Button disabled={!candidate(true)} onClick={() => { const binding = candidate(true); if (binding) onChange({...path,source:{...source,emitters:[...source.emitters,{id:uuid(),name:"Emitter",binding,xyz:null,spectrum:[],band:"visible",native_reversed:false,maximum_level:1,response_exponent:1,provenance:unknownOpticalProvenance()}]}}); }}>Add optical emitter</Button>
  </>}
  <h4>Filters in beam order</h4>
  <p className="field-hint">Add CMY flags, correction filters and wheels in physical order. Missing transmission stays unknown; isolated wheel colors are not filter measurements.</p>
  {path.filters.map((filter,index) => {
   const set = (patch: Partial<typeof filter>) => onChange({...path,filters:path.filters.map((f,i) => i === index ? {...f,...patch} : f)});
   const move = (delta: number) => {const filters=[...path.filters]; [filters[index],filters[index+delta]]=[filters[index+delta],filters[index]]; onChange({...path,filters});};
   return <fieldset key={filter.id}><legend>Filter {index+1}</legend>
    <TextField label={`Filter ${index+1} name`} value={filter.name} onChange={(e) => set({name:e.target.value})} />
    <BindingField mode={mode} path={path} label={`Filter ${index+1} native function`} value={filter.binding} onChange={(binding) => set({binding})} />
    <p>Transmission: {filter.transmission.type === "unknown" ? "Unknown" : `${filter.transmission.samples.length} measured raw ranges retained`}</p>
    <ProvenanceFields label={`Filter ${index+1}`} value={filter.provenance} onChange={(provenance) => set({provenance})} />
    <div className="fixture-optical-actions"><Button disabled={!index} onClick={() => move(-1)}>Move filter {index+1} earlier</Button><Button disabled={index === path.filters.length-1} onClick={() => move(1)}>Move filter {index+1} later</Button>
     <Button onClick={() => onChange({...path,filters:path.filters.filter((f) => f.id !== filter.id)})}>Remove filter {index+1}</Button></div>
   </fieldset>;
  })}
  <Button disabled={!candidate(false)} onClick={() => {const binding=candidate(false);if (binding) onChange({...path,filters:[...path.filters,{id:uuid(),name:"Filter",binding,transmission:{type:"unknown"},provenance:unknownOpticalProvenance()}]});}}>Add optical filter</Button>
  {!!path.measurements?.length && <p>{path.measurements.length} whole-path recipe measurements retained.</p>}
  {opticalPathWarnings(path,mode).map((warning) => <p className="field-hint" key={warning}>{warning}</p>)}
 </div>;
}

export function ColorPhysicalEditor({mode,onChange}:{mode:FixtureMode;onChange(mode:FixtureMode):void}) {
 const errors = colorPhysicalErrors(mode);
 return <section className="fixture-color-physical" aria-label="Physical optical paths">
  <h3>Physical optical paths</h3>
  <p>Describe the source and the filters light passes through, per head. These settings are saved with the fixture; they do not yet change live color matching or Stage simulation.</p>
  {mode.color_physical && <NumberField label="Optical model revision" value={mode.color_physical.revision} min={0} max={0xffff_ffff} onChange={(e) => onChange({...mode,color_physical:{...mode.color_physical!,revision:Number(e.target.value)}})} />}
  {mode.heads.map((head) => {
   const path = mode.color_physical?.paths.find((p) => p.head_id === head.id);
   return <details key={head.id}><summary>{head.name} · {path ? "Physical path configured" : "No physical path"}</summary>
    {path ? <>
     <HeadPathEditor mode={mode} path={path} onChange={(p) => onChange(withOpticalPath(mode,p))} />
     <Button onClick={() => {const paths=mode.color_physical!.paths.filter((p) => p.id !== path.id);onChange({...mode,color_physical:paths.length ? {...mode.color_physical!,paths} : null});}}>Remove physical path</Button>
    </> : <Button onClick={() => onChange(withOpticalPath(mode,createHeadOpticalPath(mode,head.id)))}>Create physical path for {head.name}</Button>}
   </details>;
  })}
  {!!errors.length && <ul role="alert" className="fixture-physical-mapping-errors">{errors.map((e) => <li key={e}>{e}</li>)}</ul>}
 </section>;
}
