// The fixture-profile editor is shared with ToskLight Architect so both desks author the same
// profiles. It lives in @tosklight/patch; this module keeps the desk's import path and binds the
// desk's own Stage renderer and root-confined file picker to the editor's host ports.
import {
	FixtureProfileEditor as SharedFixtureProfileEditor,
	type FixtureProfileEditorPorts,
	type FixtureProfileEditorProps as SharedFixtureProfileEditorProps,
} from "@tosklight/patch/library";
import { RootConfinedFilePickerButton } from "../files/RootConfinedFilePickerButton";
import {
	buildFixtureProfileGeometryPreview,
	disposeScene,
} from "../../windows/stage3dScene";
import type * as THREE from "three";

export {
	applyCanonicalChannelAttribute,
	replaceFunctionBehavior,
	replaceHeadColorSystem,
} from "@tosklight/patch/library";

export type FixtureProfileEditorProps = Omit<
	SharedFixtureProfileEditorProps,
	"ports"
>;

/** The desk keeps file choosing inside its configured roots, wherever a profile asset comes from. */
const deskPorts: FixtureProfileEditorPorts = {
	buildGeometryPreview: buildFixtureProfileGeometryPreview,
	disposeScene: (scene) => disposeScene(scene as THREE.Scene),
	AssetPicker: RootConfinedFilePickerButton,
};

export function FixtureProfileEditor(props: FixtureProfileEditorProps) {
	return <SharedFixtureProfileEditor {...props} ports={deskPorts} />;
}
