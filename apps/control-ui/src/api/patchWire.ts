import type {
	PatchDelta,
	PatchFixtureProjection,
	PatchProfileRevisionProjection,
} from "./generated/light-wire";
import type {
	PatchChange,
	PatchDirectControlEndpoint as FeatureDirectControlEndpoint,
	PatchError as FeaturePatchError,
	PatchEventMessage,
	PatchFixtureProjection as FeatureFixtureProjection,
	PatchMutationOutcome,
	PatchProfileRevision as FeatureProfileRevision,
	PatchSnapshot as FeaturePatchSnapshot,
} from "../features/patch/contracts";
import {
	validatePatchErrorResponse,
	validatePatchEventServerMessage,
	validatePatchFixturesOutcome,
	validatePatchSnapshot,
} from "./patchWireValidation";
import { WireValidationError } from "./wireValidation";

export function decodePatchSnapshot(value: unknown): FeaturePatchSnapshot {
	const snapshot = validatePatchSnapshot(value);
	return {
		showId: snapshot.show_id,
		showRevision: snapshot.show_revision,
		patchRevision: snapshot.patch_revision,
		cursor: snapshot.cursor.sequence,
		fixtures: snapshot.fixtures.map(mapFixtureProjection),
		profileRevisions: snapshot.profile_revisions.map(mapProfileRevision),
	};
}

export function decodePatchFixturesOutcome(value: unknown): PatchMutationOutcome {
	const outcome = validatePatchFixturesOutcome(value);
	return {
		requestId: outcome.request_id,
		replayed: outcome.replayed,
		changed: outcome.changed,
		...mapChange(outcome),
	};
}

export function decodePatchErrorResponse(value: unknown): FeaturePatchError {
	const error = validatePatchErrorResponse(value);
	return {
		error: error.error,
		currentRevision: error.current_revision ?? null,
		retryable: error.retryable,
	};
}

export function decodePatchEventServerMessage(
	value: unknown,
): PatchEventMessage {
	const message = validatePatchEventServerMessage(value);
	switch (message.type) {
		case "ready":
		case "repaired":
			return { type: message.type, cursor: message.cursor.sequence };
		case "gap":
			return {
				type: "gap",
				afterSequence: message.gap.after_sequence,
				oldestAvailable: message.gap.oldest_available,
				latestSequence: message.gap.latest_sequence,
			};
		case "event": {
			if (message.event.payload.type !== "show_patch_changed")
				throw new WireValidationError(
					"$.event.payload.type",
					"show_patch_changed",
					message.event.payload.type,
				);
			return {
				type: "event",
				sequence: message.event.sequence,
				change: mapChange(message.event.payload.delta),
			};
		}
		case "error":
			return { type: "error", error: message.error };
	}
}

function mapChange(delta: PatchDelta): PatchChange {
	return {
		showId: delta.show_id,
		showRevision: delta.show_revision,
		patchRevision: delta.patch_revision,
		eventSequence: delta.event_sequence ?? null,
		fixtures: delta.fixtures.map(mapFixtureProjection),
		removedFixtureIds: [...delta.removed_fixture_ids],
		profileRevisions: delta.profile_revisions.map(mapProfileRevision),
	};
}

function mapFixtureProjection(
	fixture: PatchFixtureProjection,
): FeatureFixtureProjection {
	return {
		fixtureId: fixture.fixture_id,
		fixtureRevision: fixture.fixture_revision,
		fixtureNumber: fixture.fixture_number,
		virtualFixtureNumber: fixture.virtual_fixture_number,
		name: fixture.name,
		profileId: fixture.profile_id,
		profileRevision: fixture.profile_revision,
		modeId: fixture.mode_id,
		splitPatches: fixture.split_patches.map((split) => ({ ...split })),
		layerId: fixture.layer_id,
		directControl: mapDirectControl(fixture.direct_control),
		location: { ...fixture.location },
		rotation: { ...fixture.rotation },
		logicalHeads: fixture.logical_heads.map((head) => ({
			profileHeadId: head.profile_head_id,
			headIndex: head.head_index,
			fixtureId: head.fixture_id,
		})),
		multipatch: fixture.multipatch.map((instance) => ({
			id: instance.id,
			name: instance.name,
			splitPatches: instance.split_patches.map((split) => ({ ...split })),
			location: { ...instance.location },
			rotation: { ...instance.rotation },
		})),
		moveInBlackEnabled: fixture.move_in_black_enabled,
		moveInBlackDelayMillis: fixture.move_in_black_delay_millis,
		highlightOverrides: fixture.highlight_overrides.map((override) => ({
			channelId: override.channel_id,
			rawValue: override.raw_value,
		})),
	};
}

function mapDirectControl(
	endpoint: PatchFixtureProjection["direct_control"],
): FeatureDirectControlEndpoint | null {
	return endpoint
		? {
				protocol: endpoint.protocol,
				ipAddress: endpoint.ip_address,
				port: endpoint.port,
			}
		: null;
}

function mapProfileRevision(
	profile: PatchProfileRevisionProjection,
): FeatureProfileRevision {
	return {
		profileId: profile.profile_id,
		profileRevision: profile.profile_revision,
		contentDigest: profile.content_digest,
		manufacturer: profile.manufacturer,
		name: profile.name,
		fixtureType: profile.fixture_type,
		patchPolicy: profile.patch_policy,
		referencedModes: profile.referenced_modes.map((mode) => ({
			modeId: mode.mode_id,
			name: mode.name,
			splits: mode.splits.map((split) => ({ ...split })),
		})),
	};
}
