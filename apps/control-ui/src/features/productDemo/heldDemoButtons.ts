import type { PlaybackRuntimeActions } from "../playbackRuntime/actionWriter";

interface HeldDemoButton {
	slot: number;
	playbackNumber: number;
	button: number;
}

/**
 * Owns demo momentary-button lifetimes beyond any individual DOM node, so a
 * held button keeps its original Playback number and button through topology
 * changes and releases only after its own press has settled.
 */
export class HeldDemoButtons {
	private readonly held = new Map<string, HeldDemoButton>();
	private tail: Promise<void> = Promise.resolve();

	constructor(private readonly actions: PlaybackRuntimeActions | null) {}

	press(slot: number, playbackNumber: number, button: number) {
		const id = buttonId(slot, button);
		const current = this.held.get(id);
		if (current?.playbackNumber === playbackNumber) return;
		if (current) this.release(id, current);
		const next = { slot, playbackNumber, button };
		this.held.set(id, next);
		this.enqueue(next, true);
	}

	releaseButton(slot: number, button: number) {
		const id = buttonId(slot, button);
		const current = this.held.get(id);
		if (current) this.release(id, current);
	}

	releaseAll() {
		for (const [id, current] of this.held) this.release(id, current);
	}

	private release(id: string, current: HeldDemoButton) {
		this.held.delete(id);
		this.enqueue(current, false);
	}

	private enqueue(current: HeldDemoButton, pressed: boolean) {
		const send = () => this.send(current, pressed);
		const result = this.tail.then(send, send);
		this.tail = result.then(
			() => undefined,
			() => undefined,
		);
	}

	private async send(current: HeldDemoButton, pressed: boolean) {
		await this.actions?.poolPlaybackAction(current.playbackNumber, "button", {
			button: current.button,
			pressed,
			surface: "physical",
		});
	}
}

function buttonId(slot: number, button: number) {
	return `${slot}:${button}`;
}
