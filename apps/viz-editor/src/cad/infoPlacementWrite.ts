/** Match each Info edit to its authoritative gesture/history path. */
import type { PatchFixtureProjection } from "@tosklight/patch";
import { TauriPatchTransport } from "../document/transport";
import { cadSession } from "./session";

const transport = new TauriPatchTransport();
export async function persistInfoFixture(
	next: PatchFixtureProjection,
	editedEntityId: string,
	sceneRevision: number,
	numeric: boolean,
	confirmPosition = false,
) {
	if (numeric) {
		const pose =
			editedEntityId === next.fixtureId
				? next
				: next.multipatch.find((copy) => copy.id === editedEntityId);
		if (!pose) throw new Error("The selected placement no longer exists");
		await cadSession.setNumericTransforms(sceneRevision, [
			{
				id: editedEntityId,
				...(confirmPosition ? { stagePositionSet: true } : {}),
				positionMillimetres: [
					pose.location.x,
					pose.location.y,
					pose.location.z,
				],
				rotationDegrees: [
					pose.rotation.x,
					pose.rotation.y,
					pose.rotation.z,
				],
			},
		]);
	} else if (next.sceneryOptions?.architecture) {
		await cadSession.patchVenue(sceneRevision, [next]);
	} else
		await transport.patchFixtures("", 0, {
			requestId: crypto.randomUUID(),
			fixtures: [next],
			removeFixtureIds: [],
		});
}
