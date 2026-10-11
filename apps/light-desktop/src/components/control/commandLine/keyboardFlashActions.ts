import type { PlaybackAddressValue } from "../../../api/playbackAddress";
import type { PlaybackRuntimeActions } from "../../../features/playbackRuntime/actionWriter";
import { HeldPlaybackControls } from "../../../features/playbackRuntime/heldPlaybackControls";
export type KeyboardHeldAction = "flash" | "swap";

/** Key release always uses the original address and runtime authority. */
export class KeyboardHeldActions {
	private held = new HeldPlaybackControls();
	private keys = new Set<string>();
	private authority: PlaybackRuntimeActions | null | undefined;
	syncAuthority(actions: PlaybackRuntimeActions | null) {
		if (this.authority !== undefined && actions !== this.authority) this.releaseAll();
		this.authority = actions;
	}
	press(code: string, address: PlaybackAddressValue, action: KeyboardHeldAction, actions: PlaybackRuntimeActions | null, surfaceId: string) {
		if (this.keys.has(code)) return;
		this.keys.add(code);
		const controlId = [...code].reduce((value, char) => value * 31 + char.charCodeAt(0), 0) >>> 0;
		this.held.press(code, address, {surface_id: surfaceId, control_id: controlId}, async (hold, pressed) => {
			await actions?.playbackAction(hold.address, action, {pressed, control_source: hold.controlSource});
		});
	}
	release(code: string) { if (!this.keys.delete(code)) return false; this.held.release(code); return true; }
	releaseAll() { this.held.releaseAll(); this.keys.clear(); }
}
