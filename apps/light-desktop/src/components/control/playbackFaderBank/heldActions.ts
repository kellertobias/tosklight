import type { PlaybackRuntimeActions } from "../../../features/playbackRuntime/actionWriter";

interface HeldPlaybackButton {
	slot: number;
	playbackNumber: number;
	button: number;
	action: "flash" | "swap";
}

/** Owns physical momentary-button lifetimes beyond any individual DOM node. */
export class HeldPlaybackActions {
	private readonly held = new Map<string, HeldPlaybackButton>();
	private tail: Promise<void> = Promise.resolve();

	constructor(private readonly actions: PlaybackRuntimeActions | null) {}

	press(
		slot: number,
		playbackNumber: number,
		button: number,
		action: HeldPlaybackButton["action"],
	) {
		const id = buttonId(slot, button);
		const current = this.held.get(id);
		if (
			current?.playbackNumber === playbackNumber &&
			current.button === button &&
			current.action === action
		)
			return;
		if (current) this.release(id, current);
		const next = { slot, playbackNumber, button, action };
		this.held.set(id, next);
		this.enqueue(next, true);
	}

	releaseButton(slot: number, button: number) {
		const id = buttonId(slot, button);
		const current = this.held.get(id);
		if (current) this.release(id, current);
	}

	releaseSlot(slot: number) {
		for (const [id, current] of this.held)
			if (current.slot === slot) this.release(id, current);
	}

	releaseAll() {
		for (const [id, current] of this.held) this.release(id, current);
	}

	private release(id: string, current: HeldPlaybackButton) {
		this.held.delete(id);
		this.enqueue(current, false);
	}

	private enqueue(current: HeldPlaybackButton, pressed: boolean) {
		const send = () => this.send(current, pressed);
		const result = this.tail.then(send, send);
		this.tail = result.then(
			() => undefined,
			() => undefined,
		);
	}

	private async send(current: HeldPlaybackButton, pressed: boolean) {
		await this.actions?.poolPlaybackAction(
			current.playbackNumber,
			current.action,
			{
			pressed,
			surface: "physical",
			},
		);
	}
}

function buttonId(slot: number, button: number) {
	return `${slot}:${button}`;
}
