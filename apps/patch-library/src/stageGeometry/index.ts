/**
 * How the Stage draws a fixture profile's own geometry: its parts, their transforms, and the beams
 * its emitters throw.
 *
 * The desk's Stage and every fixture-profile editor's live preview both build from here, so a
 * profile looks the same in the ToskLight Stage, in its editor on the desk, and in the Architect.
 */
export * from "./attributeValues";
export * from "./emitterGeometry";
export * from "./installedAppearance";
export * from "./profileGeometry";
export * from "./renderStyle";
export * from "./resources";
export * from "./sceneObjects";
export * from "./shaperAppearance";
export type * from "./types";
