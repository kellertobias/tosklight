import type { VersionedObject } from "../../api/types";
import type {
	ShowObjectBodies,
	ShowObjectKind,
	ShowObjectMutationResponse,
} from "../showObjects/contracts";
import type { ServerController } from "./model";

type MutationModel = Pick<
	ServerController,
	"client" | "setError" | "showObjectsStore"
>;

export async function runOptimisticShowObjectMutation<
	K extends ShowObjectKind,
>(
	model: MutationModel,
	showId: string,
	kind: K,
	objectId: string,
	body: ShowObjectBodies[K],
	write: () => Promise<ShowObjectMutationResponse>,
) {
	let token: string | null = null;
	try {
		token = model.showObjectsStore.beginOptimistic(
			showId,
			kind,
			objectId,
			body,
		);
		const response = await write();
		model.showObjectsStore.commit(
			token,
			response.revision,
			response.event_sequence,
		);
		model.setError(null);
		return true;
	} catch (reason) {
		const error = asError(reason);
		if (token) model.showObjectsStore.rollback(token, error);
		model.setError(error.message);
		return false;
	}
}

export async function reconcileShowObject<K extends ShowObjectKind>(
	model: MutationModel,
	showId: string,
	kind: K,
	objectId: string,
	minimumEventSequence?: number | null,
) {
	try {
		const object = await model.client.object<ShowObjectBodies[K]>(
			showId,
			kind,
			objectId,
		);
		model.showObjectsStore.installObject(
			showId,
			kind,
			object as VersionedObject<ShowObjectBodies[K]>,
			minimumEventSequence,
		);
		model.setError(null);
		return true;
	} catch (reason) {
		model.setError(asError(reason).message);
		return false;
	}
}

function asError(reason: unknown): Error {
	return reason instanceof Error ? reason : new Error(String(reason));
}
