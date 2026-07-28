import { ApiRequestError } from "../../api/ApiRequestError";
import type {
	DynamicDefinitionProjection,
	DynamicUpdateIntent,
	ShowObjectActionOutcome,
} from "../../api/generated/light-wire";
import type { ShowObject } from "../showObjects/contracts";
import type { ShowObjectsStore } from "../showObjects/store";
import { applyDynamicUpdateIntent } from "./dynamicUpdateIntent";

export interface DynamicMutationApi {
	object<T>(showId: string, kind: string, id: string): Promise<{
		id: string;
		revision: number;
		updated_at: string;
		body: T;
	}>;
	updateDynamic(
		showId: string,
		id: string,
		expectedRevision: number,
		intent: DynamicUpdateIntent,
		mutationGroup?: string,
	): Promise<ShowObjectActionOutcome>;
}

interface QueuedMutation {
	showId: string;
	objectId: string;
	intent: DynamicUpdateIntent;
	mutationGroup?: string;
	token: string;
	authorityGeneration: number;
}

/**
 * Owns optimistic Dynamic-definition projection and serializes writes per
 * object. Encoder bursts therefore never reuse one stale object revision.
 */
export class DynamicMutationWriter {
	private readonly tails = new Map<string, Promise<void>>();

	constructor(
		private readonly store: ShowObjectsStore,
		private readonly api: DynamicMutationApi,
	) {}

	update(
		showId: string,
		objectId: string,
		intent: DynamicUpdateIntent,
		mutationGroup?: string,
	): Promise<void> {
		const snapshot = this.store.getSnapshot();
		const current = dynamicFrom(snapshot.dynamics, showId, objectId);
		const body = applyDynamicUpdateIntent(current.body, intent);
		const mutation: QueuedMutation = {
			showId,
			objectId,
			intent,
			mutationGroup,
			token: this.store.beginOptimistic(
				showId,
				"dynamic",
				objectId,
				body,
			),
			authorityGeneration: snapshot.authorityGeneration,
		};
		const key = `${showId}:${objectId}`;
		const previous = this.tails.get(key) ?? Promise.resolve();
		const execution = previous
			.catch(() => undefined)
			.then(() => this.persist(mutation));
		const tail = execution.finally(() => {
			if (this.tails.get(key) === tail) this.tails.delete(key);
		});
		this.tails.set(key, tail);
		return tail;
	}

	private async persist(mutation: QueuedMutation) {
		try {
			let current = dynamicFrom(
				this.store.getSnapshot().dynamics,
				mutation.showId,
				mutation.objectId,
			);
			let outcome: ShowObjectActionOutcome;
			try {
				outcome = await this.api.updateDynamic(
					mutation.showId,
					mutation.objectId,
					current.revision,
					mutation.intent,
					mutation.mutationGroup,
				);
			} catch (cause) {
				if (!(cause instanceof ApiRequestError) || cause.status !== 409)
					throw cause;
				current = (await this.api.object<DynamicDefinitionProjection>(
					mutation.showId,
					"dynamic",
					mutation.objectId,
				)) as ShowObject<"dynamic">;
				this.store.installObject(
					mutation.showId,
					"dynamic",
					current,
				);
				outcome = await this.api.updateDynamic(
					mutation.showId,
					mutation.objectId,
					current.revision,
					mutation.intent,
					mutation.mutationGroup,
				);
			}
			this.store.settlePending(
				mutation.token,
				{
					objectId: mutation.objectId,
					revision: outcome.object.revision,
					object: outcome.object as ShowObject<"dynamic">,
				},
				outcome.show_revision,
				outcome.event_sequence ?? null,
				mutation.authorityGeneration,
			);
		} catch (cause) {
			const error = cause instanceof Error ? cause : new Error(String(cause));
			this.store.rollback(mutation.token, error);
			throw error;
		}
	}
}

function dynamicFrom(
	dynamics: readonly ShowObject<"dynamic">[],
	showId: string,
	objectId: string,
) {
	const current = dynamics.find((dynamic) => dynamic.id === objectId);
	if (!current)
		throw new Error(
			`Dynamic ${objectId} is not available in active show ${showId}.`,
		);
	return current;
}
