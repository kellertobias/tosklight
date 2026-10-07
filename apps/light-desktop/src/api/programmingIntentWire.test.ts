import { describe, expect, it } from "vitest";
import { decodeAttributeValue } from "./programmerValuesWireProjection";
import type { ProgrammingColorIntent } from "./generated/light-wire";

const id = (part: number) => `00000000-0000-0000-0000-${part.toString().padStart(12, "0")}`;
const semantic = (): ProgrammingColorIntent => ({
 base_xyz: { x: 0, y: 0, z: 0 },
 recipe: { version: 1, rgb: [0, 0, 0], amber: 0, approximate: false },
 white_blend: 0,
 white_target: { kelvin: 6500, duv: 0 },
 uv: { amount: 0.7 }, relative_output: 0, allocation: "preserve_recipe",
});
const decodeColor = (intent: unknown) => decodeAttributeValue({ kind: "color_program", value: { kind: "semantic", intent } }, "$.value");

describe("complete intent wire values", () => {
 it("retains UV-only black and explicit zero visible output", () => {
  const intent = semantic();
  expect(decodeColor(intent)).toEqual({ kind: "color_program", value: { kind: "semantic", intent } });
  const { uv: _uv, ...withoutUv } = intent;
  expect(decodeColor(withoutUv)).toMatchObject({ value: { intent: { uv: { amount: 0 }, relative_output: 0 } } });
 });
 it("preserves exact Direct integers and independent unknown estimates", () => {
  const value = { kind: "color_program", value: { kind: "direct", recipe: {
   source: { profile_id: id(1), profile_revision: 2, profile_digest: "digest", mode_id: id(3), head_id: id(4), path_id: id(5), model_revision: 6, native_layout_signature: "layout" },
   channels: [{ channel_id: id(7), function_id: id(8), raw: 4294967294 }],
   spreads: [{ binding: { channel_id: id(7), function_id: id(8) }, points: [4294967293, 4294967294] }],
  }, portable: { model_revision: 6, visible: null, uv: { amount: 0.8, quality: "estimated" }, quality: "unknown", limitations: ["Visible appearance unavailable"] } } };
  expect(decodeAttributeValue(value, "$.value")).toEqual(value);
  expect(() => decodeAttributeValue({ ...value, value: { ...value.value, portable: { ...value.value.portable, model_revision: 9 } } }, "$")).toThrow(/pinned/);
 });
 it("retains unwrapped angles, signed target offsets and physical zoom", () => {
  for (const value of [
   { kind: "position", value: { kind: "angles", pan_degrees: { kind: "spread", value: [-720, 720] }, tilt_degrees: { kind: "value", value: 270 } } },
   { kind: "position", value: { kind: "target", reference: { kind: "point", point_id: id(9) }, offset_metres: [{ kind: "value", value: -3 }, { kind: "value", value: 2 }, { kind: "value", value: 0 }] } },
   { kind: "zoom", value: { opening_degrees: { kind: "spread", value: [4, 60] }, convention: "beam" } },
  ]) expect(decodeAttributeValue(value, "$" )).toEqual(value);
 });
 it("rejects conflicting representations, malformed ranges and non-finite components", () => {
  expect(() => decodeColor({ ...semantic(), uv: { amount: 1.1 } })).toThrow();
  expect(() => decodeColor({ ...semantic(), base_xyz: { x: 0, y: Infinity, z: 0 } })).toThrow();
  expect(() => decodeColor({ ...semantic(), spreads: [{ component: "red", points: [0, 1] }, { component: "hue", points: [350, 10] }] })).toThrow(/representation/);
  expect(() => decodeColor({ ...semantic(), spreads: [{ component: "uv", points: [0, 1] }, { component: "uv", points: [1, 0] }] })).toThrow(/unique/);
  expect(() => decodeAttributeValue({ kind: "position", value: { kind: "angles", pan_degrees: { kind: "value", value: 0 }, tilt_degrees: { kind: "value", value: 90 }, reference: { kind: "origin" } } }, "$" )).toThrow(/declared wire field/);
 });
});

it("decodes Rust f32 shortest decimal extremes but rejects explicit null defaults", () => {
 const rustJson = '{"kind":"position","value":{"kind":"angles","pan_degrees":{"kind":"value","value":3.4028235e38},"tilt_degrees":{"kind":"value","value":-3.4028235e38}}}';
 expect(decodeAttributeValue(JSON.parse(rustJson), "$")).toEqual(JSON.parse(rustJson));
 expect(() => decodeAttributeValue({ kind: "position", value: { kind: "angles", pan_degrees: { kind: "value", value: 3.5e38 }, tilt_degrees: { kind: "value", value: 0 } } }, "$")).toThrow();
 for (const key of ["uv", "relative_output", "allocation", "spreads", "wheel_constraints"]) {
  expect(() => decodeColor({ ...semantic(), [key]: null })).toThrow();
 }
});

it("rejects a falsely exact Easy recipe without replacing advanced coordinates", () => {
 const intent = semantic();
 intent.recipe.rgb = [1, 1, 1];
 expect(() => decodeColor(intent)).toThrow(/recipe/);
 intent.recipe.approximate = true;
 expect(decodeColor(intent)).toMatchObject({ value: { intent: { base_xyz: { x: 0, y: 0, z: 0 }, recipe: { approximate: true } } } });
});

it("keeps member assignments inside a Group and rejects nesting or different owners", () => {
 const template = { kind: "color_program", value: { kind: "semantic", intent: semantic() } };
 const value = { kind: "group_family", value: { owner: "color", template, members: { [id(42)]: template } } };
 expect(decodeAttributeValue(value, "$", "group")).toEqual(value);
 expect(() => decodeAttributeValue(value, "$")).toThrow(/Group scope/);
 expect(() => decodeAttributeValue({ ...value, value: { ...value.value, template: value } }, "$", "group")).toThrow(/non-nested/);
 expect(() => decodeAttributeValue({ ...value, value: { ...value.value, owner: "position" } }, "$", "group")).toThrow(/declared Group family/);
});

it("requires materialized fixture values but retains universal authoring curves", () => {
 const value = { kind: "position", value: { kind: "angles", pan_degrees: { kind: "spread", value: [-360,360] }, tilt_degrees: { kind: "value", value: 0 } } };
 expect(decodeAttributeValue(value,"$")).toEqual(value);
 expect(() => decodeAttributeValue(value,"$","fixture")).toThrow(/materialized/);
});
