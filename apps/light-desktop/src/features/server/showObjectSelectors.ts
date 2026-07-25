import type { ShowObject } from "../showObjects/contracts";
import {
	selectPortableGroups,
	selectPresets,
} from "../showObjects/selectors";
import type { ServerController } from "./model";

type ShowObjectModel = Pick<ServerController, "showObjectsStore">;

export function currentPortableGroups(
	model: ShowObjectModel,
): readonly ShowObject<"group">[] {
	return selectPortableGroups(model.showObjectsStore.getSnapshot());
}

export function currentPresets(
	model: ShowObjectModel,
): readonly ShowObject<"preset">[] {
	return selectPresets(model.showObjectsStore.getSnapshot());
}
