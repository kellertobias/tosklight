import type { CaptureMask } from "./types";
import type { PlaybackAddress } from "../../apps/light-desktop/src/features/playbackTopology/contracts";
import type { PlaybackPreloadScope } from "../../apps/light-desktop/src/api/playbackConfigurationModels";
/** Two eligible positions, four address-based masks, independently combined with programmer capture. */
export function captureMasks(): CaptureMask[] {
 const scopes:PlaybackPreloadScope[]=[
 {position_ranges:[{start:51,end:52}],pages:[99]},
 {position_ranges:[{start:51,end:51}],pages:null},
 {position_ranges:[{start:52,end:52}],pages:[1]},
 {position_ranges:[{start:1,end:1000}],pages:null}];
 return scopes.flatMap(playbackScope=>[false,true].map(programmer=>({programmer,playbackScope})));
}
export function scopeContains(scope:PlaybackPreloadScope,address:PlaybackAddress) {
 return (scope.pages===null||scope.pages.includes(address.page))&&scope.position_ranges.some(range=>address.position>=range.start&&address.position<=range.end);
}
