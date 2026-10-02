import type { ProgrammingComponentEdit } from "../../api/generated/light-wire";
import {
	encodeProgrammingComponentEdits,
	type ProgrammingComponentEditsOperation,
} from "../../api/programmingComponentEditWire";
import type { ColorAdoptionInput, ProgrammerValuesHold } from "../../api/colorAdoptionWire";
import type { ProgrammerValueTiming } from "./contracts";
import type { DisplayedSource } from "./displayedSource";

/**
 * Family-neutral gesture session core (TL-556).
 *
 * One session belongs to one control surface (a dialog, an aim pad, an encoder bank, an OSC
 * adapter) and one programming family. The family is fixed at construction: its `attribute`
 * (the owner key the backend finishes, for example `position`, `focus`, `zoom`, `color`) and
 * its edit builder, which turns one surface-level change sample into ordered generated
 * `ProgrammingComponentEdit`s. The session owns at most one open gesture. Each gesture:
 *
 * - pins the writer of the lane it started on (Normal or Preload) until it ends, so a lane
 *   switch mid-gesture never moves the gesture's edits or its Finish to the other writer;
 * - mints a fresh Undo group, which is also the backend `caller_id`, so two gestures never
 *   coalesce into one Undo step;
 * - submits only `component_edits` for the family attribute, tagged with that Undo group;
 * - ends exactly once. Ending stops the producer first (rate motion is client-integrated, so
 *   this is what stops the motion), then drops the gesture's unsent rows
 *   (`writer.cancelGesture`), then queues exactly one `writer.finishGesture` with a fresh
 *   request ID, the family attribute and the original Undo group. Admitted and in-flight edits
 *   still complete.
 * - A discrete step (a key, a button, a typed value, an encoder detent) is not motion: its
 *   edits ARE the operator's request. `commit()` and the encoder `idle` end keep every admitted
 *   edit (sent in order, behind whatever is in flight) and queue the one Finish behind them, so
 *   a step pressed while the previous one is still settling is never lost (TL-637 follow-up).
 *
 * The session never infers family state (for example whether Position Target is active). Any
 * state an edit builder needs is supplied by the caller in the change sample or the start input.
 */

export type FamilyGestureLane = "normal" | "preload";

/** Non-release ends. The list is a superset of the special dialogs' cancel reasons. */
export type FamilyGestureCancelReason =
	| "pointer-cancel"
	| "lost-capture"
	| "blur"
	| "hidden"
	| "close"
	| "superseded"
	| "teardown";

/**
 * `release` is `end()`; `commit` is `commit()` (a completed discrete step); `idle` is the
 * encoder idle timeout. `commit` and `idle` keep the gesture's admitted edits.
 */
export type FamilyGestureEndReason =
	| "release"
	| "commit"
	| "idle"
	| FamilyGestureCancelReason;

export interface FamilyGestureIntentInput {
	requestId: string;
	fixtureIds: readonly string[];
	groupId: string | null;
	attribute: string;
	operation: ProgrammingComponentEditsOperation;
	undoGroup: string;
	timing: ProgrammerValueTiming;
	/** TL-594: the leased source the surface displayed when this gesture started. */
	displayedSource?: DisplayedSource | null;
	/** TL-554: the Direct reference head / explicit semantic start of this gesture. */
	colorAdoption?: ColorAdoptionInput | null;
}

export interface FamilyGestureFinishInput {
	requestId: string;
	attribute: string;
	undoGroup: string;
	/**
	 * A completed discrete step: keep the gesture's admitted (unsent) edits and queue the Finish
	 * behind them instead of dropping them. Absent for motion ends and cancels.
	 */
	keepAdmittedEdits?: true;
}

/**
 * The lane writer surface the session needs. `ProgrammerValuesWriter` and
 * `ProgrammerPreloadValuesWriter` both satisfy it unchanged.
 */
export interface FamilyGestureWriter {
	applyIntent(input: FamilyGestureIntentInput): Promise<unknown>;
	cancelGesture(undoGroup: string): number;
	finishGesture(input: FamilyGestureFinishInput): Promise<unknown>;
}

/** Injectable timer for the idle-ending encoder mode. */
export interface FamilyGestureTimers {
	setTimeout(callback: () => void, millis: number): unknown;
	clearTimeout(handle: unknown): void;
}

export interface FamilyGestureSessionOptions {
	/** The lane's current writer, read once at `start`. `null` refuses the start quietly. */
	writerFor(lane: FamilyGestureLane): FamilyGestureWriter | null | undefined;
	/** Fresh identity for Undo groups and request IDs. Defaults to `crypto.randomUUID()`. */
	createId?(): string;
	timers?: FamilyGestureTimers;
	/** Programming errors, edit refusals and genuine writer/transport failures. */
	onError?(error: Error): void;
	/**
	 * TL-594, optional: the displayed-source lease, read once at `start`. Every edit of that
	 * gesture names it, so the server adopts exactly what the operator saw. `fixtureIds` are the
	 * fixtures the gesture's surface displayed (`displayedFixtureIds`, else `fixtureIds`); pass
	 * them to `DisplayedSourceReadouts.displayedSource(lane, fixtureIds)` so a Preload capture
	 * names a lease that delivered them. Without it edits keep the server's latest adoption.
	 */
	displayedSource?(
		lane: FamilyGestureLane,
		fixtureIds: readonly string[],
	): DisplayedSource | null | undefined;
	/** TL-594: the server held an edit because the named lease is gone. Re-read readouts. */
	onDisplayedSourceHold?(lane: FamilyGestureLane): void;
	/**
	 * TL-554: any quiet hold of an edit (including the displayed-source one), for example
	 * `explicit_color_start_required` or `native_color_unavailable`. Nothing changed.
	 */
	onHold?(lane: FamilyGestureLane, reason: ProgrammerValuesHold): void;
	/** TL-554: the outcome of every edit, for surfaces that report an adoption once. */
	onOutcome?(lane: FamilyGestureLane, outcome: unknown): void;
}

export interface FamilyGestureStartInput {
	lane: FamilyGestureLane;
	fixtureIds: readonly string[];
	groupId?: string | null;
	timing: ProgrammerValueTiming;
	/** Stops the surface's producer (rate frames, encoder accumulation) before cleanup. */
	stopProducer?: () => void;
	/** Encoder mode: end with `idle` after this many milliseconds without a change. */
	idleEndMillis?: number;
	/** TL-554: the Direct reference head / explicit semantic start every edit names. */
	colorAdoption?: ColorAdoptionInput | null;
	/**
	 * The fixtures whose readouts the surface displayed (a Group gesture's members); selects the
	 * covering displayed-source lease. Defaults to `fixtureIds`.
	 */
	displayedFixtureIds?: readonly string[];
}

/**
 * One programming family: the attribute every gesture authors and finishes, and the builder
 * of one change sample. The builder returns `[]` for an empty change (quiet refusal) and
 * throws (`FamilyGestureEditRefusedError` or any error) for a change the family contract
 * forbids; a thrown refusal is reported once through `onError` and nothing is sent.
 */
export interface FamilyGestureFamily<
	TChange,
	TStart extends FamilyGestureStartInput = FamilyGestureStartInput,
> {
	readonly attribute: string;
	buildEdits(change: TChange, start: TStart): ProgrammingComponentEdit[];
}

/** A change the family contract refuses locally (for example a Target offset without Target). */
export class FamilyGestureEditRefusedError extends Error {
	constructor(message: string) {
		super(message);
		this.name = "FamilyGestureEditRefusedError";
	}
}

export interface FamilyGestureHandle<TChange> {
	readonly lane: FamilyGestureLane;
	readonly attribute: string;
	readonly undoGroup: string;
	readonly isOpen: boolean;
	readonly endReason: FamilyGestureEndReason | null;
	/** Resolves with the Finish outcome (or `null` when quietly abandoned) after the end. */
	readonly finished: Promise<unknown>;
	/** Submits one family edit. Returns `null` when refused (ended gesture, empty or invalid). */
	change(change: TChange): Promise<unknown> | null;
	/** Operator release. Returns `false` when the gesture had already ended. */
	end(): boolean;
	/**
	 * A completed discrete step (key, button, typed value): ends the gesture keeping every
	 * admitted edit, sent in order, then one Finish. Returns `false` when already ended.
	 */
	commit(): boolean;
	cancel(reason: FamilyGestureCancelReason): boolean;
	/** Registers another producer stop; runs at once if the gesture has already ended. */
	addProducerStop(stop: () => void): () => void;
}

const defaultTimers: FamilyGestureTimers = {
	setTimeout: (callback, millis) => globalThis.setTimeout(callback, millis),
	clearTimeout: (handle) =>
		globalThis.clearTimeout(handle as ReturnType<typeof setTimeout>),
};

function asError(error: unknown) {
	return error instanceof Error ? error : new Error(String(error));
}

interface FamilyGestureSessionContext {
	readonly timers: FamilyGestureTimers;
	createId(): string;
	report(error: unknown): void;
	released(gesture: object): void;
	held(lane: FamilyGestureLane, reason: ProgrammerValuesHold): void;
	outcome(lane: FamilyGestureLane, outcome: unknown): void;
}

function holdOf(outcome: unknown): ProgrammerValuesHold | null {
	if (typeof outcome !== "object" || outcome === null) return null;
	const hold = (outcome as { hold?: unknown }).hold;
	return typeof hold === "string" ? (hold as ProgrammerValuesHold) : null;
}

class FamilyGesture<TChange, TStart extends FamilyGestureStartInput>
	implements FamilyGestureHandle<TChange>
{
	readonly finished: Promise<unknown>;
	private resolveFinished!: (outcome: unknown) => void;
	private producerStops: (() => void)[] = [];
	private idleTimer: unknown = null;
	private reason: FamilyGestureEndReason | null = null;

	constructor(
		readonly undoGroup: string,
		private readonly family: FamilyGestureFamily<TChange, TStart>,
		private readonly writer: FamilyGestureWriter,
		private readonly input: TStart,
		private readonly session: FamilyGestureSessionContext,
		private readonly displayedSource: DisplayedSource | null = null,
	) {
		this.finished = new Promise((resolve) => {
			this.resolveFinished = resolve;
		});
		if (input.stopProducer) this.producerStops.push(input.stopProducer);
		this.armIdle();
	}

	get lane() {
		return this.input.lane;
	}

	get attribute() {
		return this.family.attribute;
	}

	get isOpen() {
		return this.reason === null;
	}

	get endReason() {
		return this.reason;
	}

	change(change: TChange) {
		if (!this.isOpen) return null;
		let edits: ProgrammingComponentEdit[];
		try {
			edits = this.family.buildEdits(change, this.input);
			if (edits.length === 0) return null;
			encodeProgrammingComponentEdits(edits, `$.${this.attribute}.edits`);
		} catch (error) {
			this.session.report(error);
			return null;
		}
		this.armIdle();
		return this.writer
			.applyIntent({
				requestId: this.session.createId(),
				fixtureIds: this.input.fixtureIds,
				groupId: this.input.groupId ?? null,
				attribute: this.attribute,
				operation: { type: "component_edits", edits },
				undoGroup: this.undoGroup,
				timing: this.input.timing,
				...(this.displayedSource
					? { displayedSource: this.displayedSource }
					: {}),
				...(this.input.colorAdoption
					? { colorAdoption: this.input.colorAdoption }
					: {}),
			})
			.then((outcome) => {
				const hold = holdOf(outcome);
				if (hold) this.session.held(this.lane, hold);
				this.session.outcome(this.lane, outcome);
				return outcome;
			})
			.catch((error: unknown) => {
				this.session.report(error);
				return null;
			});
	}

	end() {
		return this.stop("release");
	}

	commit() {
		return this.stop("commit");
	}

	cancel(reason: FamilyGestureCancelReason) {
		return this.stop(reason);
	}

	addProducerStop(stop: () => void) {
		if (!this.isOpen) {
			this.runProducerStop(stop);
			return () => undefined;
		}
		this.producerStops.push(stop);
		return () => {
			this.producerStops = this.producerStops.filter((entry) => entry !== stop);
		};
	}

	/**
	 * The single terminal path: producer stop, then (motion ends and cancels) drop unsent rows,
	 * then one Finish. A discrete step's end (`commit`, `idle`) keeps its admitted edits.
	 */
	stop(reason: FamilyGestureEndReason) {
		if (!this.isOpen) return false;
		this.reason = reason;
		this.clearIdle();
		this.session.released(this);
		const stops = this.producerStops;
		this.producerStops = [];
		for (const stop of stops) this.runProducerStop(stop);
		const keepAdmittedEdits = reason === "commit" || reason === "idle";
		if (!keepAdmittedEdits)
			try {
				this.writer.cancelGesture(this.undoGroup);
			} catch (error) {
				this.session.report(error);
			}
		let finish: Promise<unknown>;
		try {
			finish = this.writer.finishGesture({
				requestId: this.session.createId(),
				attribute: this.attribute,
				undoGroup: this.undoGroup,
				...(keepAdmittedEdits ? { keepAdmittedEdits: true as const } : {}),
			});
		} catch (error) {
			finish = Promise.reject(error);
		}
		finish.then(this.resolveFinished, (error: unknown) => {
			this.session.report(error);
			this.resolveFinished(null);
		});
		return true;
	}

	private runProducerStop(stop: () => void) {
		try {
			stop();
		} catch (error) {
			this.session.report(error);
		}
	}

	private armIdle() {
		const millis = this.input.idleEndMillis;
		if (millis === undefined || !(millis > 0) || !Number.isFinite(millis))
			return;
		this.clearIdle();
		this.idleTimer = this.session.timers.setTimeout(() => {
			this.idleTimer = null;
			this.stop("idle");
		}, millis);
	}

	private clearIdle() {
		if (this.idleTimer === null) return;
		this.session.timers.clearTimeout(this.idleTimer);
		this.idleTimer = null;
	}
}

export class FamilyGestureSession<
	TChange,
	TStart extends FamilyGestureStartInput = FamilyGestureStartInput,
> {
	private current: FamilyGesture<TChange, TStart> | null = null;
	private disposed = false;
	private readonly context: FamilyGestureSessionContext;

	constructor(
		private readonly family: FamilyGestureFamily<TChange, TStart>,
		private readonly options: FamilyGestureSessionOptions,
	) {
		this.context = {
			timers: options.timers ?? defaultTimers,
			createId: () => options.createId?.() ?? crypto.randomUUID(),
			report: (error) => options.onError?.(asError(error)),
			released: (gesture) => {
				if (this.current === gesture) this.current = null;
			},
			held: (lane, reason) => {
				if (reason === "displayed_source_unavailable")
					options.onDisplayedSourceHold?.(lane);
				options.onHold?.(lane, reason);
			},
			outcome: (lane, outcome) => options.onOutcome?.(lane, outcome),
		};
	}

	/** The family attribute every gesture of this session authors and finishes. */
	get attribute() {
		return this.family.attribute;
	}

	/** The open gesture, if any. */
	get active(): FamilyGestureHandle<TChange> | null {
		return this.current;
	}

	get isDisposed() {
		return this.disposed;
	}

	/**
	 * Opens a new gesture on `lane`, superseding any open one first. Returns `null` quietly
	 * when the session is disposed or the lane has no writer (scope gone).
	 */
	start(input: TStart): FamilyGestureHandle<TChange> | null {
		if (this.disposed) return null;
		this.current?.stop("superseded");
		const writer = this.options.writerFor(input.lane);
		if (!writer) return null;
		const displayed =
			this.options.displayedSource?.(
				input.lane,
				input.displayedFixtureIds ?? input.fixtureIds,
			) ?? null;
		const gesture = new FamilyGesture(
			this.context.createId(),
			this.family,
			writer,
			input,
			this.context,
			displayed?.lane === input.lane ? displayed : null,
		);
		this.current = gesture;
		return gesture;
	}

	/** Cancels the open gesture (surface-level blur, hidden, close). */
	cancel(reason: FamilyGestureCancelReason) {
		return this.current?.stop(reason) ?? false;
	}

	/** Ends an open gesture with `teardown` and refuses later starts. Idempotent. */
	dispose() {
		if (this.disposed) return;
		this.disposed = true;
		this.current?.stop("teardown");
	}
}

/**
 * Ends the session's open gesture on window blur and on the document becoming hidden, as the
 * special dialogs do. Works for any family session. Returns the disposer. Mounting surfaces
 * call it once per session.
 */
export function attachGestureWindowGuards(
	session: { cancel(reason: FamilyGestureCancelReason): boolean },
	target: { window: Window; document: Document } = { window, document },
) {
	const blur = () => session.cancel("blur");
	const visibility = () => {
		if (target.document.hidden) session.cancel("hidden");
	};
	target.window.addEventListener("blur", blur);
	target.document.addEventListener("visibilitychange", visibility);
	return () => {
		target.window.removeEventListener("blur", blur);
		target.document.removeEventListener("visibilitychange", visibility);
	};
}
