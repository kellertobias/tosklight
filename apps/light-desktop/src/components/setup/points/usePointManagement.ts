import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { PsnBinding } from "../../../api/client/psn";
import type { PatchedFixture } from "../../../api/types";
import { useFixtureLibrary } from "../../../features/fixtureLibrary/FixtureLibraryContext";
import { useOptionalPatch } from "../../../features/patch/PatchContext";
import { usePsn } from "../../../features/psn/PsnContext";
import { mergeFixtureDefinitions } from "../fixtureProfileModel";
import { positionPoints } from "../fixturePatch/positionReference";
import {
	movedPointLocation,
	newPointCandidate,
	type PointAxis,
	pointDefinition,
} from "./pointManagement";

const NO_FIXTURES: readonly PatchedFixture[] = [];

export interface PointManagement {
	/** Every 3D Point of the show, in Patch table order. */
	points: readonly PatchedFixture[];
	/** The tracker binding of each Point, by its fixture UUID. */
	bindings: ReadonlyMap<string, PsnBinding>;
	/** The Point created last from this view, highlighted until the next one. */
	createdId: string | null;
	busy: boolean;
	/** Progress or the reason the last action failed; never silent. */
	message: string | null;
	error: boolean;
	/** Whether the Patch and the 3D Point profile are both available. */
	canCreate: boolean;
	create(): Promise<string | null>;
	rename(point: PatchedFixture, name: string): Promise<void>;
	move(point: PatchedFixture, axis: PointAxis, metres: number): Promise<void>;
	remove(point: PatchedFixture): Promise<void>;
}

/**
 * TL-651: create, name, place and delete the show's Points. Every change is an ordinary Patch
 * change of a 3D Point fixture, so it is validated, undoable and saved with the show exactly as
 * the Fixtures view's changes are.
 */
export function usePointManagement(active: boolean): PointManagement {
	// Show Patch always sits under the desk's Patch boundary; without one there is nothing to manage.
	const patch = useOptionalPatch();
	const library = useFixtureLibrary();
	const psn = usePsn();
	const definitions = useMemo(
		() =>
			mergeFixtureDefinitions(
				library?.fixtureProfiles ?? [],
				library?.fixtureLibrary ?? [],
			),
		[library?.fixtureProfiles, library?.fixtureLibrary],
	);
	const definition = pointDefinition(definitions);
	const fixtures = patch?.fixtures ?? NO_FIXTURES;
	const points = useMemo(() => positionPoints(fixtures), [fixtures]);
	const [bindings, setBindings] = useState<ReadonlyMap<string, PsnBinding>>(
		() => new Map(),
	);
	const [createdId, setCreatedId] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);
	const [message, setMessage] = useState<{ text: string; error: boolean } | null>(
		null,
	);

	useEffect(() => {
		if (!active || !psn) return;
		let current = true;
		psn
			.snapshot()
			.then((snapshot) => {
				if (current)
					setBindings(
						new Map(
							snapshot.configuration.bindings.map((binding) => [
								binding.pointFixtureId,
								binding,
							]),
						),
					);
			})
			.catch(() => undefined);
		return () => {
			current = false;
		};
	}, [active, psn]);

	const run = useCallback(
		async (progress: string, action: () => Promise<boolean>, failure: string) => {
			setBusy(true);
			setMessage({ text: progress, error: false });
			try {
				if (await action()) setMessage(null);
				else setMessage({ text: patch?.error ?? failure, error: true });
			} catch (cause) {
				setMessage({
					text: cause instanceof Error && cause.message ? cause.message : failure,
					error: true,
				});
			} finally {
				setBusy(false);
			}
		},
		[patch?.error],
	);

	const create = useCallback(async () => {
		if (!definition) {
			setMessage({
				text: "The fixture library holds no ToskLight 3D Point profile, so no Point can be created.",
				error: true,
			});
			return null;
		}
		if (!patch) return null;
		const candidate = newPointCandidate(definition, patch.fixtures);
		if (!candidate) {
			setMessage({ text: "No free fixture ID is left for a new Point.", error: true });
			return null;
		}
		let created: string | null = null;
		await run(
			"Creating Point…",
			async () => {
				const result = await patch.patchFixtures([candidate]);
				created = result?.[0]?.fixtureId ?? null;
				return Boolean(created);
			},
			"The Point could not be created. Review the Patch status and try again.",
		);
		setCreatedId(created);
		return created;
	}, [definition, patch, run]);

	return {
		points,
		bindings,
		createdId,
		busy,
		message: message?.text ?? null,
		error: message?.error ?? false,
		canCreate: Boolean(definition) && patch?.status === "ready",
		create,
		rename: async (point, name) => {
			const next = name.trim();
			if (!next || next === point.name) return;
			await run(
				"Renaming Point…",
				async () => Boolean(await patch?.updateFixture(point.fixture_id, { name: next })),
				"The Point could not be renamed.",
			);
		},
		move: async (point, axis, metres) => {
			if (!Number.isFinite(metres)) return;
			const location = movedPointLocation(point, axis, metres);
			if (location[axis] === (point.location?.[axis] ?? 0)) return;
			await run(
				"Moving Point…",
				async () => Boolean(await patch?.updateFixture(point.fixture_id, { location })),
				"The Point could not be moved.",
			);
		},
		remove: async (point) => {
			await run(
				"Deleting Point…",
				async () => Boolean(await patch?.deleteFixture(point.fixture_id)),
				"The Point could not be deleted.",
			);
			if (createdId === point.fixture_id) setCreatedId(null);
		},
	};
}

/**
 * Runs one **Create Point** asked for from elsewhere (the Point encoder's picker) once the Patch
 * and the 3D Point profile are available. Each request number creates exactly one Point, so a
 * remount or a re-render never creates a second one.
 */
export function useCreatePointRequest(points: PointManagement, request: number) {
	const handled = useRef(0);
	const { canCreate, create } = points;
	useEffect(() => {
		if (!request || request <= handled.current || !canCreate) return;
		handled.current = request;
		void create();
	}, [request, canCreate, create]);
}
