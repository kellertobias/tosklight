import type { DynamicValueAddressProjection, ProgrammingAttributeValue, ProgrammingNativeColorIdentity } from "./generated/light-wire";
import { WireValidationError } from "./wireValidation";

/** A component mask stores the complete materialized family; no scalar shadow or destination
 * profile can replace its original portable Color/Position intent. */
export function validateProgrammingMask(address: DynamicValueAddressProjection, family: ProgrammingAttributeValue, path: string): void {
 const rep = address.representation;
 let valid = false;
 switch (rep.kind) {
  case "angles": valid = family.kind === "position" && family.value.kind === "angles" && family.value.pan_degrees.kind === "value" && family.value.tilt_degrees.kind === "value"; break;
  case "target": valid = family.kind === "position" && family.value.kind === "target" && family.value.offset_metres.every(axis => axis.kind === "value") &&
   (rep.reference === null || rep.reference.kind === family.value.reference.kind && (rep.reference.kind === "origin" || family.value.reference.kind === "point" && rep.reference.point_id === family.value.reference.point_id)); break;
  case "focus": valid = family.kind === "normalized"; break;
  case "zoom": valid = family.kind === "zoom" && family.value.opening_degrees.kind === "value" && family.value.convention === rep.convention; break;
  case "semantic_color": valid = family.kind === "color_program" && family.value.kind === "semantic" && !family.value.intent.spreads?.length; break;
  case "direct_color": {
   if (family.kind !== "color_program" || family.value.kind !== "direct") break;
   const recipe = family.value.recipe;
   valid = !recipe.spreads?.length && (Object.keys(rep.source) as (keyof ProgrammingNativeColorIdentity)[]).every(key => rep.source[key] === recipe.source[key]);
   if (address.component?.kind === "native_color") {
    const binding = address.component.component;
    valid &&= recipe.channels.some(channel => channel.channel_id === binding.channel_id && channel.function_id === binding.function_id);
   }
   break;
  }
 }
 if (!valid) throw new WireValidationError(path, "complete materialized family matching its mask address", family);
}
