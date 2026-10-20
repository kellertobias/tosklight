import type { BuiltInWindow } from "../../types";

const tiledKinds: ReadonlySet<BuiltInWindow> = new Set([
	"groups", "presets", "cuelists", "cuelist_pool", "qlists", "qlist_pool",
	"playback", "playback_pool", "dynamics", "macros", "timecode", "playbacks", "media",
]);

/** Pane contents that present an addressable pool of tiles. Tables and Stage stay unchanged. */
export function isTiledPane(kind: BuiltInWindow): boolean {
	return tiledKinds.has(kind);
}
