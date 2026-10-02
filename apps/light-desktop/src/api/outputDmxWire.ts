import type { OutputDmxSnapshot } from "./generated/light-wire";
import type { DmxSnapshot } from "./types";

/** Native simulation lanes are consumed by Stage, not DMX diagnostics. */
export function decodeOutputDmxSnapshot(snapshot: OutputDmxSnapshot): DmxSnapshot {
    return { revision: snapshot.revision, universes: snapshot.universes, overrides: snapshot.overrides ?? [] };
}
