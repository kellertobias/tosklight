import type { PlaybackAddressValue } from "../../../api/playbackAddress";
import type { PlaybackRuntimeActions } from "../../../features/playbackRuntime/actionWriter";
import { HeldPlaybackControls } from "../../../features/playbackRuntime/heldPlaybackControls";

/** Momentary controls retain the exact address/source until every release completes. */
export class HeldPlaybackActions {
	private readonly held = new HeldPlaybackControls();
	private readonly keys = new Map<number, Set<string>>();
	constructor(
		private readonly actions: PlaybackRuntimeActions | null,
		private readonly surfaceId: string,
	) {}
	press(
		position: number,
		address: PlaybackAddressValue,
		button: number,
		action: "flash" | "swap",
	) {
		const key = `${position}:${button}`;
		const keys = this.keys.get(position) ?? new Set<string>();
		keys.add(key);
		this.keys.set(position, keys);
		this.held.press(
			key,
			address,
			{ surface_id: this.surfaceId, control_id: position * 10 + button },
			async (hold, pressed) => {
				await this.actions?.playbackAction(hold.address, action, {
					pressed,
					control_source: hold.controlSource,
				});
			},
		);
	}
	releaseButton(position: number, button: number) {
		this.held.release(`${position}:${button}`);
	}
	releaseSlot(position: number) {
		for (const key of this.keys.get(position) ?? []) this.held.release(key);
		this.keys.delete(position);
	}
	releaseAll() {
		this.held.releaseAll();
		this.keys.clear();
	}
}
