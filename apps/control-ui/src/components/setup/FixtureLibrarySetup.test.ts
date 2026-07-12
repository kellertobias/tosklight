import { describe, expect, it } from "vitest";
import JSZip from "jszip";
import { importGdtfData, parseHeadDrafts } from "./FixtureLibrarySetup";

describe("fixture library editor", () => {
  it("builds sequential multi-head channels with physical and gobo metadata", () => {
    const result = parseHeadDrafts([
      { name: "Master", master: true, channels: "dimmer,pan:16[-270,270,deg]" },
      { name: "Layer 1", master: false, channels: "gobo{Open=0-31|Dots=32-63},tilt:16[-135,135,deg]" },
    ]);
    expect(result.footprint).toBe(6);
    expect(result.heads.map((head) => [head.name, head.shared])).toEqual([["Master", true], ["Layer 1", false]]);
    expect(result.heads[0].parameters[1].components.map((component) => component.offset)).toEqual([1, 2]);
    expect(result.heads[0].parameters[1].metadata).toMatchObject({ physical_min: -270, physical_max: 270, unit: "deg" });
    expect(result.heads[1].parameters[0].capabilities).toEqual([
      { name: "Open", dmx_from: 0, dmx_to: 31, preset_family: "gobo" },
      { name: "Dots", dmx_from: 32, dmx_to: 63, preset_family: "gobo" },
    ]);
    expect(result.heads[1].parameters[1].components.map((component) => component.offset)).toEqual([4, 5]);
  });

  it("imports GDTF modes with heads, physical ranges, capabilities, and embedded geometry", async () => {
    const zip = new JSZip();
    zip.file("description.xml", `<GDTF><FixtureType Manufacturer="Acme" Name="Orbit Wash" ShortName="Orbit"><PhysicalDescriptions><Emitters><Emitter Name="Red" Color="0.64,0.33,1"/><Emitter Name="Green" Color="0.30,0.60,1"/><Emitter Name="Blue" Color="0.15,0.06,1"/></Emitters></PhysicalDescriptions><DMXModes><DMXMode Name="Extended"><DMXChannels>
      <DMXChannel Offset="1,2" Geometry="Master" Default="32768/1"><LogicalChannel Attribute="Pan"><ChannelFunction PhysicalFrom="-270" PhysicalTo="270" /></LogicalChannel></DMXChannel>
      <DMXChannel Offset="3" Geometry="Cell 1"><LogicalChannel Attribute="Gobo1"><ChannelFunction><ChannelSet Name="Open" DMXFrom="0/1" DMXTo="31/1"/><ChannelSet Name="Dots" DMXFrom="32/1" DMXTo="63/1"/></ChannelFunction></LogicalChannel></DMXChannel>
    </DMXChannels></DMXMode></DMXModes></FixtureType></GDTF>`);
    zip.file("models/orbit.glb", new Uint8Array([1, 2, 3]));
    const definitions = await importGdtfData(await zip.generateAsync({ type: "uint8array" }), "orbit.gdtf");
    expect(definitions).toHaveLength(1);
    expect(definitions[0]).toMatchObject({ manufacturer: "Acme", model: "Orbit", mode: "Extended", footprint: 3, device_type: "spot mover", physical: { pan_range_degrees: 540 } });
    expect(definitions[0].heads.map((head) => head.name)).toEqual(["Master", "Cell 1"]);
    expect(definitions[0].heads[0].parameters[0].default).toBeCloseTo(.5, 3);
    expect(definitions[0].heads[1].parameters[0].capabilities[1]).toMatchObject({ name: "Dots", dmx_from: 32, dmx_to: 63, preset_family: "gobo" });
    expect(definitions[0].model_asset).toMatch(/^data:model\/gltf-binary;base64,/);
    expect((definitions[0].color_calibration as { emitters: unknown[] }).emitters).toHaveLength(3);
  });
});
