import type { PlaybackRuntimeActions } from "../../../features/playbackRuntime/actionWriter";

export type KeyboardHeldAction = "flash" | "swap";

interface HeldPlaybackAction {
	playbackNumber: number;
	action: KeyboardHeldAction;
	/** The writer that owns the press; its release must pair with it. */
	actions: PlaybackRuntimeActions | null;
}

/**
 * Owns momentary keyboard Playback lifetimes beyond any single key event. A
 * held key keeps its action and Playback number even if the Page, desk, or
 * topology moves underneath it. Every press is released exactly once, in order,
 * through the writer that sent it.
 */
export class KeyboardHeldActions {
	private readonly held = new Map<string, HeldPlaybackAction>();
	private tail: Promise<void> = Promise.resolve();
	private authority: PlaybackRuntimeActions | null | undefined;

	/** Releases anything still held when the runtime authority is replaced. */
	syncAuthority(actions: PlaybackRuntimeActions | null) {
		if (this.authority !== undefined && actions !== this.authority)
			this.releaseAll();
		this.authority = actions;
	}

	press(
		code: string,
		playbackNumber: number,
		action: KeyboardHeldAction,
		actions: PlaybackRuntimeActions | null,
	) {
		if (this.held.has(code)) return;
		const current = { playbackNumber, action, actions };
		this.held.set(code, current);
		this.enqueue(current, true);
	}

	release(code: string) {
		const current = this.held.get(code);
		if (!current) return false;
		this.held.delete(code);
		this.enqueue(current, false);
		return true;
	}

	releaseAll() {
		for (const code of [...this.held.keys()]) this.release(code);
	}

	private enqueue(current: HeldPlaybackAction, pressed: boolean) {
		const send = () => this.send(current, pressed);
		const result = this.tail.then(send, send);
		this.tail = result.then(
			() => undefined,
			() => undefined,
		);
	}

	private async send(current: HeldPlaybackAction, pressed: boolean) {
		await current.actions?.poolPlaybackAction(
			current.playbackNumber,
			current.action,
			{ pressed, surface: "physical" },
		);
	}
}
