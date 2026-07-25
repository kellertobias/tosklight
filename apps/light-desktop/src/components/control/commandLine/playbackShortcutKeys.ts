import type { PlaybackRuntimeActions } from "../../../features/playbackRuntime/actionWriter";
import type { PlaybackTopologyCapability } from "../../../features/playbackTopology/contracts";
import type { PoolPlaybackAction } from "../../../features/server/playbackActionMapping";
import { canAdvancePlaybackPage } from "../PlaybackPageDialogs";
import type {
	KeyboardHeldAction,
	KeyboardHeldActions,
} from "./keyboardFlashActions";
import type { PlaybackShortcutAuthority } from "./playbackShortcutAuthority";

export interface PlaybackShortcutContext {
	authority: PlaybackShortcutAuthority;
	runtimeActions: PlaybackRuntimeActions | null;
	heldActions: KeyboardHeldActions;
	pageActions: KeyboardPageActions;
}

/** Resolves slot 1-8 on the authoritative current Page and sends its first button. */
export function pressPlaybackSlot(
	context: PlaybackShortcutContext,
	event: KeyboardEvent,
	slot: number,
) {
	const definition = context.authority.slotPlayback(slot);
	const action = definition?.buttons[0];
	if (!definition || !action || action === "none") return;
	if (!isHeldAction(action)) {
		if (event.repeat) return;
		void context.runtimeActions?.poolPlaybackAction(
			definition.number,
			action.replaceAll("_", "-") as PoolPlaybackAction,
			{ surface: "physical" },
		);
		return;
	}
	if (event.repeat) return;
	context.heldActions.press(
		event.code,
		definition.number,
		action,
		context.runtimeActions,
	);
}

export function releasePlaybackSlot(
	context: PlaybackShortcutContext,
	event: KeyboardEvent,
) {
	return context.heldActions.release(event.code);
}

/**
 * Steps the exact desk Page, creating the next one only when the current Page
 * is the last nonempty Page.
 */
export function stepPlaybackPage(
	context: PlaybackShortcutContext,
	direction: 1 | -1,
) {
	context.pageActions.step(context.authority, direction);
}

interface PageActionScope {
	generation: number;
	createPage: PlaybackTopologyCapability["createPage"];
	setActivePage: PlaybackRuntimeActions["setActivePage"];
}

/** Serializes keyboard Page writes and owns their writer-generation lifetime. */
export class KeyboardPageActions {
	private generation = 0;
	private pending = false;
	private createPage: PlaybackTopologyCapability["createPage"] | null = null;
	private setActivePage: PlaybackRuntimeActions["setActivePage"] | null = null;

	syncAuthority(
		createPage: PlaybackTopologyCapability["createPage"] | null,
		setActivePage: PlaybackRuntimeActions["setActivePage"] | null,
	) {
		if (this.createPage === createPage && this.setActivePage === setActivePage)
			return;
		this.invalidate();
		this.createPage = createPage;
		this.setActivePage = setActivePage;
	}

	invalidate() {
		this.generation += 1;
		this.pending = false;
		this.createPage = null;
		this.setActivePage = null;
	}

	step(authority: PlaybackShortcutAuthority, direction: 1 | -1) {
		if (this.pending) return;
		const target = pageStepTarget(authority, direction);
		const scope = target == null ? null : this.capture();
		if (!scope || target == null) return;
		this.pending = true;
		this.run(scope, target, authority.pages);
	}

	private capture(): PageActionScope | null {
		if (!this.createPage || !this.setActivePage) return null;
		return {
			generation: this.generation,
			createPage: this.createPage,
			setActivePage: this.setActivePage,
		};
	}

	private run(
		scope: PageActionScope,
		target: number,
		pages: PlaybackShortcutAuthority["pages"],
	) {
		void Promise.resolve()
			.then(() => this.apply(scope, target, pages))
			.catch(() => undefined)
			.finally(() => {
				if (this.isCurrent(scope)) this.pending = false;
			});
	}

	private async apply(
		scope: PageActionScope,
		target: number,
		pages: PlaybackShortcutAuthority["pages"],
	) {
		if (pages.some((page) => page.number === target)) {
			await scope.setActivePage(target);
			return;
		}
		if (!(await scope.createPage(target))) return;
		if (this.isCurrent(scope)) await scope.setActivePage(target);
	}

	private isCurrent(scope: PageActionScope) {
		return (
			scope.generation === this.generation &&
			scope.createPage === this.createPage &&
			scope.setActivePage === this.setActivePage
		);
	}
}

function pageStepTarget(
	authority: PlaybackShortcutAuthority,
	direction: 1 | -1,
) {
	const { activePage, pages } = authority;
	if (activePage == null) return;
	const target = activePage + direction;
	if (target < 1) return;
	if (pages.some((page) => page.number === target)) return target;
	return direction === 1 && canAdvancePlaybackPage(pages, activePage)
		? target
		: undefined;
}

function isHeldAction(action: string): action is KeyboardHeldAction {
	return action === "flash" || action === "swap";
}
