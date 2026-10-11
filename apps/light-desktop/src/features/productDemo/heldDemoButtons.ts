import type { PlaybackAddressValue } from "../../api/playbackAddress";
import type { PlaybackRuntimeActions } from "../playbackRuntime/actionWriter";
import { HeldPlaybackControls } from "../playbackRuntime/heldPlaybackControls";

/** Keeps each demo button's exact address and source until its paired release. */
export class HeldDemoButtons {
	private readonly held = new HeldPlaybackControls();
	constructor(private readonly actions: PlaybackRuntimeActions | null) {}
	press(slot: number, address: PlaybackAddressValue, button: number) {
		this.held.press(`${slot}:${button}`, address, {
			surface_id: "demo",
			control_id: slot * 10 + button,
		}, (hold, pressed) => this.actions?.playbackAction(hold.address, "button", {
			button, pressed, control_source: hold.controlSource,
		}) ?? Promise.resolve(null));
	}
	releaseButton(slot: number, button: number) { this.held.release(`${slot}:${button}`); }
	releaseAll() { this.held.releaseAll(); }
}
