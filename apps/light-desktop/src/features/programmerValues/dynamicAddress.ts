import type { ProgrammingComponent } from "../../api/familyEncoderModels";
import type { ProgrammerDynamicValue, ProgrammerDynamicValueAddress } from "./contracts";

/** Keep snapshot identity and delta removal identity identical. Whole-family controls use
 * the legacy owner key; component masks have distinct tracks inside that owner. */
export function programmingComponentKey(component: ProgrammingComponent | null | undefined): string {
 if (!component) return "whole";
 switch (component.kind) {
  case "native_color": return `native_color:${component.component.channel_id}:${component.component.function_id}`;
  case "color": case "color_wheel": return `${component.kind}:${component.component}`;
  default: return component.kind;
 }
}

export function dynamicStoreAddress(entry: ProgrammerDynamicValue | ProgrammerDynamicValueAddress): string {
 const value = "value" in entry ? entry.value : null;
 const instance = value ? (value.type === "dynamic_on" || value.type === "dynamic_off" ? value.instance_link : null) : (entry as ProgrammerDynamicValueAddress).instanceLink;
 const lane = value ? (value.type === "dynamic_on" ? value.lane_id : null) : (entry as ProgrammerDynamicValueAddress).laneId;
 const component = value ? (value.type === "programming_fix_at" ? value.mask.address.component : value.type === "programming_release" ? value.component : null) : (entry as ProgrammerDynamicValueAddress).component;
 return `${entry.fixtureId}\u0000${entry.attribute}\u0000${instance ?? "static"}\u0000${lane ?? "all"}\u0000${programmingComponentKey(component)}`;
}
