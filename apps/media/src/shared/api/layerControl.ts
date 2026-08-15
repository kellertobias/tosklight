// Layer control: the writes, their optimistic effect, and their rollback.
//
// This lives in the shared API layer rather than inside the layers feature because two features
// write to layers — the layer page and the visualizer picker — and a feature must never reach
// into another feature's internals to do it.
//
// An optimistic change here is not a guess about what the server will do — the projection the
// server returns replaces it either way. It exists so a fader feels attached to the value. The
// rollback matters more than the optimism: when a desk takes an output, the write is refused and
// the control must snap back to what the desk says, not to what the operator dragged.

import { useCallback, useRef, useState } from "react";
import { ApiFailure, api } from "./client";
import type {
	OutputView,
	UpdateLayer,
	UpdateMaster,
} from "./generated/media-wire";
import { KEYS } from "./queries";
import {
	type Resource,
	settleOptimisticResource,
	stageOptimisticResource,
	useResource,
} from "./resource";

/** Outputs change continuously while a desk is driving them. */
export const OUTPUT_POLL_MS = 1_000;

export function useOutputsForControl(): Resource<OutputView[]> {
	return useResource<OutputView[]>(KEYS.outputs, api.outputs, {
		pollMs: OUTPUT_POLL_MS,
	});
}

export interface LayerControl {
	/** The last write that was refused, still on screen so the operator learns why. */
	refusal: ApiFailure | undefined;
	dismissRefusal: () => void;
	update: (
		output: OutputView,
		layer: number,
		change: UpdateLayer,
	) => Promise<void>;
	/** A continuous gesture: keep immediate feedback, but never replay stale drag samples. */
	updateContinuous: (
		output: OutputView,
		layer: number,
		change: UpdateLayer,
	) => Promise<void>;
	updateMaster: (output: OutputView, change: UpdateMaster) => Promise<void>;
	updateMasterContinuous: (
		output: OutputView,
		change: UpdateMaster,
	) => Promise<void>;
	setTakeover: (output: OutputView, takeOver: boolean) => Promise<void>;
	reset: (output: OutputView, layer: number) => Promise<void>;
	busy: boolean;
}

interface ContinuousSlot {
	token: string;
	started: boolean;
	change: UpdateLayer | UpdateMaster;
	waiters: Array<() => void>;
	project: (
		outputs: OutputView[],
		change: UpdateLayer | UpdateMaster,
	) => OutputView[];
	request: (change: UpdateLayer | UpdateMaster) => Promise<OutputView>;
}

export function useLayerControl(): LayerControl {
	const [refusal, setRefusal] = useState<ApiFailure | undefined>(undefined);
	const [busy, setBusy] = useState(false);
	const pending = useRef(0);
	const sequence = useRef(0);
	const queues = useRef(new Map<string, Promise<unknown>>());
	const queueGenerations = useRef(new Map<string, number>());
	const continuousSlots = useRef(new Map<string, ContinuousSlot[]>());

	const begin = useCallback(() => {
		pending.current += 1;
		setBusy(true);
	}, []);
	const end = useCallback(() => {
		pending.current = Math.max(0, pending.current - 1);
		setBusy(pending.current > 0);
	}, []);
	const enqueue = useCallback(
		<T>(outputId: string, operation: () => Promise<T>) => {
			const previous = queues.current.get(outputId) ?? Promise.resolve();
			const current = previous.catch(() => undefined).then(operation);
			queues.current.set(outputId, current);
			const cleanup = () => {
				if (queues.current.get(outputId) === current)
					queues.current.delete(outputId);
			};
			void current.then(cleanup, cleanup);
			return current;
		},
		[],
	);
	const enqueueDiscrete = useCallback(
		<T>(outputId: string, operation: () => Promise<T>) => {
			queueGenerations.current.set(
				outputId,
				(queueGenerations.current.get(outputId) ?? 0) + 1,
			);
			return enqueue(outputId, operation);
		},
		[enqueue],
	);

	const optimisticOutput = useCallback(
		async (
			outputId: string,
			project: (outputs: OutputView[]) => OutputView[],
			request: () => Promise<OutputView>,
		) => {
			const token = `output-${sequence.current++}`;
			stageOptimisticResource(KEYS.outputs, token, project);
			begin();
			try {
				const updated = await enqueueDiscrete(outputId, request);
				settleOptimisticResource<OutputView[]>(KEYS.outputs, token, (current) =>
					replaceOutput(current, updated),
				);
				setRefusal(undefined);
			} catch (error) {
				settleOptimisticResource<OutputView[]>(KEYS.outputs, token);
				setRefusal(asFailure(error));
			} finally {
				end();
			}
		},
		[begin, end, enqueueDiscrete],
	);

	const optimisticContinuousOutput = useCallback(
		(
			outputId: string,
			target: string,
			change: UpdateLayer | UpdateMaster,
			project: ContinuousSlot["project"],
			request: ContinuousSlot["request"],
		): Promise<void> =>
			new Promise((resolve) => {
				const generation = queueGenerations.current.get(outputId) ?? 0;
				const laneKey = `${outputId}:${generation}:${target}`;
				const slots = continuousSlots.current.get(laneKey) ?? [];
				let slot = slots.at(-1);

				// A queued request has not observed its body yet, so replace/merge it in
				// place. Once it starts, retain one newer slot and keep replacing that.
				if (slot && !slot.started) {
					slot.change = { ...slot.change, ...change };
					slot.waiters.push(resolve);
					const queuedSlot = slot;
					stageOptimisticResource<OutputView[]>(
						KEYS.outputs,
						queuedSlot.token,
						(current) => queuedSlot.project(current, queuedSlot.change),
					);
					return;
				}

				slot = {
					token: `output-${sequence.current++}`,
					started: false,
					change,
					waiters: [resolve],
					project,
					request,
				};
				slots.push(slot);
				continuousSlots.current.set(laneKey, slots);
				stageOptimisticResource<OutputView[]>(
					KEYS.outputs,
					slot.token,
					(current) => slot.project(current, slot.change),
				);
				begin();
				void enqueue(outputId, async () => {
					slot.started = true;
					const submitted = slot.change;
					try {
						const updated = await slot.request(submitted);
						settleOptimisticResource<OutputView[]>(
							KEYS.outputs,
							slot.token,
							(current) => replaceOutput(current, updated),
						);
						setRefusal(undefined);
					} catch (error) {
						settleOptimisticResource<OutputView[]>(KEYS.outputs, slot.token);
						setRefusal(asFailure(error));
					} finally {
						const currentSlots = continuousSlots.current.get(laneKey);
						if (currentSlots) {
							const remaining = currentSlots.filter(
								(candidate) => candidate !== slot,
							);
							if (remaining.length > 0)
								continuousSlots.current.set(laneKey, remaining);
							else continuousSlots.current.delete(laneKey);
						}
						for (const waiter of slot.waiters) waiter();
						end();
					}
				});
			}),
		[begin, end, enqueue],
	);

	const update = useCallback(
		async (output: OutputView, layer: number, change: UpdateLayer) => {
			await optimisticOutput(
				output.id,
				(current) => applyLocally(current, output.id, layer, change),
				() => api.updateLayer(output.id, layer, change),
			);
		},
		[optimisticOutput],
	);
	const updateContinuous = useCallback(
		async (output: OutputView, layer: number, change: UpdateLayer) => {
			await optimisticContinuousOutput(
				output.id,
				`layer-${layer}`,
				change,
				(current, pendingChange) =>
					applyLocally(current, output.id, layer, pendingChange as UpdateLayer),
				(pendingChange) =>
					api.updateLayer(output.id, layer, pendingChange as UpdateLayer),
			);
		},
		[optimisticContinuousOutput],
	);

	const reset = useCallback(
		async (output: OutputView, layer: number) => {
			begin();
			try {
				await enqueueDiscrete(output.id, () =>
					api.resetLayer(output.id, layer),
				);
				setRefusal(undefined);
			} catch (error) {
				setRefusal(asFailure(error));
			} finally {
				end();
			}
		},
		[begin, end, enqueueDiscrete],
	);
	const updateMaster = useCallback(
		async (output: OutputView, change: UpdateMaster) => {
			await optimisticOutput(
				output.id,
				(current) => applyMasterLocally(current, output.id, change),
				() => api.updateMaster(output.id, change),
			);
		},
		[optimisticOutput],
	);
	const updateMasterContinuous = useCallback(
		async (output: OutputView, change: UpdateMaster) => {
			await optimisticContinuousOutput(
				output.id,
				"master",
				change,
				(current, pendingChange) =>
					applyMasterLocally(current, output.id, pendingChange as UpdateMaster),
				(pendingChange) =>
					api.updateMaster(output.id, pendingChange as UpdateMaster),
			);
		},
		[optimisticContinuousOutput],
	);
	const setTakeover = useCallback(
		async (output: OutputView, takeOver: boolean) => {
			await optimisticOutput(
				output.id,
				(current) => applyTakeoverLocally(current, output.id, takeOver),
				() => api.setPlaybackTakeover(output.id, takeOver),
			);
		},
		[optimisticOutput],
	);

	return {
		refusal,
		dismissRefusal: useCallback(() => setRefusal(undefined), []),
		update,
		updateContinuous,
		updateMaster,
		updateMasterContinuous,
		setTakeover,
		reset,
		busy,
	};
}

export function applyMasterLocally(
	outputs: OutputView[],
	outputId: string,
	change: UpdateMaster,
): OutputView[] {
	return outputs.map((output) =>
		output.id === outputId
			? {
					...output,
					master: {
						...output.master,
						dimmer: change.dimmer ?? output.master.dimmer,
						volume: change.volume ?? output.master.volume,
						tintRed: change.tintRed ?? output.master.tintRed,
						tintGreen: change.tintGreen ?? output.master.tintGreen,
						tintBlue: change.tintBlue ?? output.master.tintBlue,
						flipMirror: change.flipMirror ?? output.master.flipMirror,
						mask: {
							...output.master.mask,
							folder: change.maskFolder ?? output.master.mask.folder,
							file: change.maskFile ?? output.master.mask.file,
						},
					},
				}
			: output,
	);
}

export function applyTakeoverLocally(
	outputs: OutputView[],
	outputId: string,
	takeOver: boolean,
): OutputView[] {
	return outputs.map((output) =>
		output.id === outputId ? { ...output, playbackTakeover: takeOver } : output,
	);
}

function asFailure(error: unknown): ApiFailure {
	return error instanceof ApiFailure
		? error
		: new ApiFailure("unexpected-error", String(error), 0);
}

/** The same change the server will make, applied to the projection we already hold. */
export function applyLocally(
	outputs: OutputView[],
	outputId: string,
	layer: number,
	change: UpdateLayer,
): OutputView[] {
	return outputs.map((output) =>
		output.id === outputId
			? {
					...output,
					layers: output.layers.map((candidate) =>
						candidate.index === layer
							? withChange(candidate, change)
							: candidate,
					),
				}
			: output,
	);
}

function withChange(
	layer: OutputView["layers"][number],
	change: UpdateLayer,
): OutputView["layers"][number] {
	return {
		...layer,
		dimmer: change.dimmer ?? layer.dimmer,
		playModeDmx: change.playModeDmx ?? layer.playModeDmx,
		scaleX: change.scaleX ?? layer.scaleX,
		scaleY: change.scaleY ?? layer.scaleY,
		scalingMode: change.scalingMode ?? layer.scalingMode,
		positionX: change.positionX ?? layer.positionX,
		positionY: change.positionY ?? layer.positionY,
		rotation: change.rotation ?? layer.rotation,
		volume: change.volume ?? layer.volume,
		tintRed: change.tintRed ?? layer.tintRed,
		tintGreen: change.tintGreen ?? layer.tintGreen,
		tintBlue: change.tintBlue ?? layer.tintBlue,
		grayscale: change.grayscale ?? layer.grayscale,
		speedMultiplierDmx: change.speedMultiplierDmx ?? layer.speedMultiplierDmx,
		playbackBpm:
			change.playbackBpm === 0
				? null
				: (change.playbackBpm ?? layer.playbackBpm),
		effects: applyEffectLocally(layer.effects, change),
		address: {
			...layer.address,
			folder: change.folder ?? layer.address.folder,
			file: change.file ?? layer.address.file,
		},
		mask: {
			...layer.mask,
			address: {
				...layer.mask.address,
				folder: change.maskFolder ?? layer.mask.address.folder,
				file: change.maskFile ?? layer.mask.address.file,
			},
			scaleX: change.maskScaleX ?? layer.mask.scaleX,
			scaleY: change.maskScaleY ?? layer.mask.scaleY,
			invert: change.maskInvert ?? layer.mask.invert,
			opacity: change.maskOpacity ?? layer.mask.opacity,
		},
	};
}

function applyEffectLocally(
	effects: OutputView["layers"][number]["effects"],
	change: UpdateLayer,
) {
	if (change.effectSlot == null) return effects;
	return effects.map((effect) => {
		if (effect.index !== change.effectSlot) return effect;
		let next = effect;
		if (change.effectType === "none") {
			next = {
				...effect,
				effectType: null,
				label: "None",
				enabled: false,
				mix: 0,
				parameters: [],
			};
		} else if (change.effectType === "analog-tv") {
			next = {
				...effect,
				effectType: "analog-tv",
				label: "Analog TV",
				enabled: true,
				mix: 1,
				supported: true,
				capabilityDetail: null,
				parameters: [
					["tv-curvature", "TV curvature", 0.3],
					["distortion", "Distortion", 0.18],
					["image-grain", "Image grain", 0.2],
					["glitching", "Glitching", 0.08],
				].map(([id, label, value]) => ({
					id: String(id),
					label: String(label),
					value: Number(value),
					defaultValue: Number(value),
				})),
			};
		} else if (change.effectType === "digital-tv") {
			next = {
				...effect,
				effectType: "digital-tv",
				label: "Digital TV",
				enabled: true,
				mix: 1,
				supported: true,
				capabilityDetail: null,
				parameters: [
					["compression-damage", "Compression damage", 0.35],
					["block-size", "Block size", 0.35],
					["tile-displacement", "Tile displacement", 0.25],
					["chroma-damage", "Chroma damage", 0.2],
					["glitching", "Glitching", 0.15],
				].map(([id, label, value]) => ({
					id: String(id),
					label: String(label),
					value: Number(value),
					defaultValue: Number(value),
				})),
			};
		}
		const values: Record<string, number | null | undefined> = {
			"tv-curvature": change.tvCurvature,
			distortion: change.effectDistortion,
			"image-grain": change.imageGrain,
			"compression-damage": change.compressionDamage,
			"block-size": change.blockSize,
			"tile-displacement": change.tileDisplacement,
			"chroma-damage": change.chromaDamage,
			glitching: change.effectGlitching,
		};
		return {
			...next,
			enabled: change.effectEnabled ?? next.enabled,
			mix: change.effectMix ?? next.mix,
			parameters: next.parameters.map((parameter) => ({
				...parameter,
				value: values[parameter.id] ?? parameter.value,
			})),
		};
	});
}

function replaceOutput(
	outputs: OutputView[],
	updated: OutputView,
): OutputView[] {
	return outputs.map((output) => (output.id === updated.id ? updated : output));
}
