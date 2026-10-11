import type { PlaybackAddressValue } from "../../../api/playbackAddress";
import type { PlaybackRuntimeActions } from "../../../features/playbackRuntime/actionWriter";
import { HeldPlaybackControls } from "../../../features/playbackRuntime/heldPlaybackControls";

type HoldGesture = { pointerId: number } | { key: string };
const gestureKey = (gesture: HoldGesture) =>
	"pointerId" in gesture
		? `pointer:${gesture.pointerId}`
		: `key:${gesture.key}`;

/** Holds belong to gestures, not rendered page cells; release always uses captured identity. */
export class HeldPlaybackActions {
	private readonly held = new HeldPlaybackControls();
	private readonly gestures = new Map<string, string>();
	constructor(private readonly actions: PlaybackRuntimeActions | null) {}
	press(
		controlSourceId: string,
		address: PlaybackAddressValue,
		button: number,
		action: "flash" | "swap",
		gesture: HoldGesture,
	) {
		const control = `${controlSourceId}:${button}`;
		// One physical button owns one hold even if a second pointer hits it.
		if (this.gestures.has(control)) return;
		this.gestures.set(control, gestureKey(gesture));
		this.held.press(
			control,
			address,
			{ surface_id: controlSourceId, control_id: button },
			async (hold, pressed) => {
				await this.actions?.playbackAction(hold.address, action, {
					pressed,
					control_source: hold.controlSource,
				});
			},
		);
	}
	releaseGesture(gesture: HoldGesture) {
		const identity = gestureKey(gesture);
		for (const [control, captured] of this.gestures) {
			if (captured !== identity) continue;
			this.gestures.delete(control);
			this.held.release(control);
		}
	}
	listen(target: Window): () => void {
		const pointerRelease = (event: PointerEvent) =>
			this.releaseGesture({ pointerId: event.pointerId });
		const keyRelease = (event: KeyboardEvent) =>
			this.releaseGesture({ key: event.key });
		target.addEventListener("pointerup", pointerRelease, true);
		target.addEventListener("pointercancel", pointerRelease, true);
		target.addEventListener("keyup", keyRelease, true);
		return () => {
			target.removeEventListener("pointerup", pointerRelease, true);
			target.removeEventListener("pointercancel", pointerRelease, true);
			target.removeEventListener("keyup", keyRelease, true);
			this.releaseAll();
		};
	}
	releaseAll() {
		this.held.releaseAll();
		this.gestures.clear();
	}
}
