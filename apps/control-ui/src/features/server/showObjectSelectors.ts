import type { ShowObject } from "../showObjects/contracts";
import {
	selectPortableGroups,
	selectPresets,
} from "../showObjects/selectors";
import type { ServerController } from "./model";
import { projectRuntimeGroupMasters } from "./groupRuntimeProjection";

type ShowObjectModel = Pick<ServerController, "playbacks" | "showObjectsStore">;

export function currentPortableGroups(
	model: Pick<ShowObjectModel, "showObjectsStore">,
): readonly ShowObject<"group">[] {
	return selectPortableGroups(model.showObjectsStore.getSnapshot());
}

export function currentPresets(
	model: Pick<ShowObjectModel, "showObjectsStore">,
): readonly ShowObject<"preset">[] {
	return selectPresets(model.showObjectsStore.getSnapshot());
}

export function currentGroups(
	model: ShowObjectModel,
): readonly ShowObject<"group">[] {
	return projectRuntimeGroupMasters(
		currentPortableGroups(model),
		model.playbacks?.authoritative_controls?.groups,
	);
}
