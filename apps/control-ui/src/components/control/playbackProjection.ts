import type { PlaybackPage } from "../../api/types";
export function playbackSlotNumbers(page: PlaybackPage | undefined, firstSlot: number, count: number): Array<number | undefined> { return Array.from({length:count},(_,index)=>page?.slots[String(firstSlot+index)]); }
