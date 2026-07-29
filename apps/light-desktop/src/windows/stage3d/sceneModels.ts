import type * as THREE from "three";
import {
	disposeObjectResources,
	mountFixtureModel,
	type Stage3dFixture,
} from "../stage3dScene";
import { refreshImprovedBeamLighting } from "../stage3dScene/improvedBeamLighting";
import type { StageModelCache } from "./modelCache";

export type MountedModelLease = {
	token: symbol;
	release: () => void;
};

export function retainFixtureModel(
	item: Stage3dFixture,
	fixtureObjects: Map<string, THREE.Object3D>,
	mountedModels: Map<string, MountedModelLease>,
	isSelected: (fixtureId: string) => boolean,
	modelCache: StageModelCache,
	onMounted: () => void,
) {
	const instanceId = item.instanceId ?? item.fixture.fixture_id;
	const source = item.fixture.definition.model_asset;
	if (!source) return;
	const lease = modelCache.retain(
		source,
		`${item.fixture.definition.id}:${item.fixture.definition.revision}`,
	);
	const token = Symbol(instanceId);
	mountedModels.set(instanceId, { token, release: lease.release });
	void lease.model.then(
		(model) => {
			if (mountedModels.get(instanceId)?.token !== token) {
				lease.release();
				return;
			}
			const root = fixtureObjects.get(instanceId);
			if (!root) {
				mountedModels.delete(instanceId);
				lease.release();
				return;
			}
			const placeholder = root.getObjectByName("fixture-placeholder");
			if (placeholder) {
				placeholder.removeFromParent();
				disposeObjectResources(placeholder);
			}
			mountFixtureModel(
				root,
				model,
				item.fixture,
				isSelected(item.fixture.fixture_id),
			);
			onMounted();
		},
		() => {
			if (mountedModels.get(instanceId)?.token === token)
				mountedModels.delete(instanceId);
			lease.release();
		},
	);
}

export function updateMountedFixtureModels({
	changedFixtures,
	removedInstanceIds,
	fixtureObjects,
	mountedModels,
	isSelected,
	modelCache,
	onMounted,
}: {
	changedFixtures: Stage3dFixture[];
	removedInstanceIds: string[];
	fixtureObjects: Map<string, THREE.Object3D>;
	mountedModels: Map<string, MountedModelLease>;
	isSelected: (fixtureId: string) => boolean;
	modelCache: StageModelCache;
	onMounted: () => void;
}) {
	const changedIds = new Set(
		changedFixtures.map((item) => item.instanceId ?? item.fixture.fixture_id),
	);
	for (const instanceId of removedInstanceIds) {
		if (changedIds.has(instanceId)) continue;
		mountedModels.get(instanceId)?.release();
		mountedModels.delete(instanceId);
	}
	for (const item of changedFixtures) {
		const instanceId = item.instanceId ?? item.fixture.fixture_id;
		const previousModel = mountedModels.get(instanceId);
		if (item.fixture.definition.model_asset) {
			retainFixtureModel(
				item,
				fixtureObjects,
				mountedModels,
				isSelected,
				modelCache,
				onMounted,
			);
			previousModel?.release();
		} else {
			previousModel?.release();
			mountedModels.delete(instanceId);
		}
	}
}

export function refreshMountedModelLighting(
	scene: THREE.Scene | null,
	renderQuality: Parameters<typeof refreshImprovedBeamLighting>[1],
	invalidate: (() => void) | null,
) {
	if (scene) refreshImprovedBeamLighting(scene, renderQuality);
	invalidate?.();
}
