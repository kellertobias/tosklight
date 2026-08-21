// Which surface a window of this app is.
//
// The desktop app opens its CAD editor as a second window of the same bundle, addressed by
// `?surface=cad`. Without this the CAD window would load the editor again, which is a second main
// window rather than the CAD editor the operator asked for.

export type VizEditorSurface = "editor" | "cad";

export function surfaceFromLocation(search: string): VizEditorSurface {
	return new URLSearchParams(search).get("surface") === "cad" ? "cad" : "editor";
}
