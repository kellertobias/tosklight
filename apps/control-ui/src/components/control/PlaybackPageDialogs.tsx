import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { PlaybackPage } from "../../api/types";
import {
	usePlaybackDeskView,
	usePlaybackRuntimeActions,
	usePlaybackRuntimeStatus,
} from "../../features/playbackRuntime/PlaybackRuntimeView";
import { usePlaybackTopologyActions } from "../../features/playbackTopology/PlaybackTopologyProvider";
import { usePlaybackPagesView } from "../../features/playbackTopology/PlaybackTopologyView";
import { normalizePlaybackPageName } from "../../features/playbackTopology/pageNames";
import type { ShowObject } from "../../features/showObjects/contracts";
import { Button, ModalTitleBar, TextInput } from "../common";
import {
	useOpenedPageMenuAuthority,
	usePlaybackPageMenuEscape,
} from "./playbackPageMenuLifecycle";
import { useScopedPageOperation } from "./useScopedPageOperation";

type PlaybackPageObject = ShowObject<"playback_page">;

export const MAX_PLAYBACK_PAGES = 127;

export function nextPlaybackPageNumber(
	pages: readonly PlaybackPage[],
): number | null {
	const lastNumber = pages.reduce(
		(maximum, page) => Math.max(maximum, page.number),
		0,
	);
	return lastNumber < MAX_PLAYBACK_PAGES ? lastNumber + 1 : null;
}

export function canAdvancePlaybackPage(
	pages: readonly PlaybackPage[],
	currentPage: number,
): boolean {
	if (pages.some((page) => page.number === currentPage + 1)) return true;
	const lastPage = pages.reduce<PlaybackPage | undefined>(
		(last, page) => (!last || page.number > last.number ? page : last),
		undefined,
	);
	return Boolean(
		lastPage &&
			currentPage === lastPage.number &&
			Object.keys(lastPage.slots ?? {}).length > 0 &&
			lastPage.number < MAX_PLAYBACK_PAGES,
	);
}

export function PlaybackPageMenu({
	open,
	onClose,
	initialFailure = null,
}: {
	open: boolean;
	onClose: () => void;
	initialFailure?: string | null;
}) {
	const topology = usePlaybackPagesView(open);
	const topologyActions = usePlaybackTopologyActions();
	const runtimeActions = usePlaybackRuntimeActions();
	const runtimeStatus = usePlaybackRuntimeStatus(open);
	const playbackDesk = usePlaybackDeskView(open);
	const [renamePage, setRenamePage] = useState<PlaybackPageObject | null>(null);
	const operation = useScopedPageOperation([
		open,
		topology.ready,
		runtimeStatus.status,
		playbackDesk !== null,
		topologyActions?.createPage,
		runtimeActions?.setActivePage,
	]);
	useOpenedPageMenuAuthority(
		open,
		topologyActions?.createPage,
		runtimeActions?.setActivePage,
		onClose,
	);
	useEffect(() => {
		operation.report(open ? initialFailure : null);
		if (!open) setRenamePage(null);
	}, [initialFailure, open, operation.report]);
	const requestClose = () => {
		if (!operation.busy) onClose();
	};
	usePlaybackPageMenuEscape(
		open && renamePage === null,
		operation.busy,
		requestClose,
	);
	if (!open) return null;
	const pageObjects = [...topology.pages].sort(
		(left, right) => left.body.number - right.body.number,
	);
	const pages = pageObjects.map((page) => page.body);
	const nextNumber = nextPlaybackPageNumber(pages);
	const ready =
		topology.ready &&
		runtimeStatus.status === "ready" &&
		playbackDesk !== null &&
		topologyActions !== null &&
		runtimeActions !== null;
	const authorityError =
		topology.error?.message ??
		(runtimeStatus.status === "error" ? runtimeStatus.error?.message : null);
	const select = async (number: number) => {
		const setActivePage = runtimeActions?.setActivePage;
		const token = ready && setActivePage ? operation.begin("select") : null;
		if (token == null || !setActivePage) return;
		const selected = await setActivePage(number);
		const current = operation.complete(
			token,
			selected ? null : `Playback Page ${number} could not be selected.`,
		);
		if (current && selected) onClose();
	};
	const add = async () => {
		const createPage = topologyActions?.createPage;
		const setActivePage = runtimeActions?.setActivePage;
		const token = ready && nextNumber != null ? operation.begin("add") : null;
		if (token == null || nextNumber == null || !createPage || !setActivePage)
			return;
		const outcome = await createPage(nextNumber);
		if (!operation.isCurrent(token)) return;
		const selected = outcome
			? await setActivePage(nextNumber)
			: false;
		const failure = outcome
			? `Playback Page ${nextNumber} could not be selected.`
			: `Playback Page ${nextNumber} could not be created.`;
		const current = operation.complete(token, selected ? null : failure);
		if (current && selected) onClose();
	};
	return createPortal(
		<div
			className="stacked-modal-layer"
			onPointerDown={(event) =>
				event.target === event.currentTarget && requestClose()
			}
		>
			<section
				className="nested-modal playback-page-modal"
				role="dialog"
				aria-modal="true"
				aria-label="Playback pages"
			>
				<ModalTitleBar
					title="Playback pages"
					actions={
						<Button
							variant="primary"
							disabled={
								operation.busy ||
								!ready ||
								nextNumber == null
							}
							onClick={() => void add()}
						>
							{operation.pending === "add" ? "Adding…" : "Add new page"}
						</Button>
					}
					closeLabel="Close Playback pages"
					closeDisabled={operation.busy}
					onClose={requestClose}
				/>
				{operation.failure && (
					<p className="modal-error" role="alert">
						{operation.failure}
					</p>
				)}
				{!ready && (
					<p role={authorityError ? "alert" : "status"}>
						{authorityError ?? "Loading Playback pages…"}
					</p>
				)}
				{operation.pending === "select" && (
					<p role="status">Selecting Playback page…</p>
				)}
				<PlaybackPageRows
					activePage={playbackDesk?.active_page ?? null}
					busy={operation.busy}
					pageObjects={pageObjects}
					ready={ready}
					onRename={setRenamePage}
					onSelect={select}
				/>
			</section>
			<PlaybackPageRenameDialog
				page={renamePage}
				openKeyboardInitially
				onClose={() => setRenamePage(null)}
			/>
		</div>,
		document.body,
	);
}

function PlaybackPageRows({
	activePage,
	busy,
	pageObjects,
	ready,
	onRename,
	onSelect,
}: {
	activePage: number | null;
	busy: boolean;
	pageObjects: readonly PlaybackPageObject[];
	ready: boolean;
	onRename: (page: PlaybackPageObject) => void;
	onSelect: (page: number) => void;
}) {
	return (
		<div aria-busy={!ready || busy}>
			{pageObjects.map((item) => (
				<div
					className={`playback-page-row ${item.body.number === activePage ? "active" : ""}`}
					key={item.id}
				>
					<Button
						className="playback-page-select"
						disabled={busy || !ready}
						onClick={() => onSelect(item.body.number)}
					>
						<strong>{item.body.number}</strong>
						<span>{item.body.name}</span>
					</Button>
					<Button
						className="playback-page-rename"
						iconOnly
						aria-label={`Rename playback page ${item.body.number}`}
						title={`Rename ${item.body.name}`}
						disabled={busy || !ready}
						onClick={() => onRename(item)}
					>
						<span className="ui-keyboard-icon" aria-hidden="true">
							⌨
						</span>
					</Button>
				</div>
			))}
		</div>
	);
}

export function PlaybackPageRenameDialog({
	page,
	onClose,
	openKeyboardInitially = false,
}: {
	page: PlaybackPageObject | null;
	onClose: () => void;
	openKeyboardInitially?: boolean;
}) {
	if (!page) return null;
	return (
		<OpenPlaybackPageRenameDialog
			page={page}
			onClose={onClose}
			openKeyboardInitially={openKeyboardInitially}
		/>
	);
}

function OpenPlaybackPageRenameDialog({
	page,
	onClose,
	openKeyboardInitially,
}: {
	page: PlaybackPageObject;
	onClose: () => void;
	openKeyboardInitially: boolean;
}) {
	const topology = usePlaybackPagesView(true);
	const actions = usePlaybackTopologyActions();
	const openedRenamePage = useRef(actions?.renamePage ?? null).current;
	const authoritativePage = topology.pages.find(
		(candidate) => candidate.id === page.id,
	);
	const pageExists = authoritativePage !== undefined;
	const authorityCurrent =
		topology.ready &&
		pageExists &&
		actions?.renamePage === openedRenamePage;
	const [name, setName] = useState(page.body.name);
	const operation = useScopedPageOperation([
		topology.ready,
		pageExists,
		actions?.renamePage,
	]);
	useEffect(() => {
		if (!authorityCurrent) onClose();
	}, [authorityCurrent, onClose]);
	const save = async (value = name) => {
		const normalized = normalizePlaybackPageName(value);
		const token =
			normalized && authorityCurrent ? operation.begin("rename") : null;
		if (token == null || !normalized || !openedRenamePage) return;
		const currentPage = authoritativePage;
		if (!currentPage) return;
		const outcome = await openedRenamePage(currentPage.body.number, normalized, {
			expectedPageRevision: currentPage.revision,
			expectedPageObjectId: page.id,
		});
		const current = operation.complete(
			token,
			outcome
				? null
				: `Playback Page ${page.body.number} could not be renamed.`,
		);
		if (current && outcome) onClose();
	};
	return createPortal(
		<div
			className="stacked-modal-layer"
			onPointerDown={(event) =>
				event.target === event.currentTarget && !operation.busy && onClose()
			}
		>
			<section
				className="nested-modal playback-page-name-modal"
				role="dialog"
				aria-modal="true"
				aria-label={`Rename playback page ${page.body.number}`}
			>
				<Button
					className="modal-close"
					disabled={operation.busy}
					onClick={onClose}
				>
					×
				</Button>
				<h3>Rename Playback Page {page.body.number}</h3>
				<TextInput
					autoFocus
					clearable
					aria-label="Playback page name"
					value={name}
					disabled={operation.busy || !authorityCurrent}
					openKeyboardInitially={openKeyboardInitially}
					onChange={(event) => setName(event.target.value)}
					onKeyboardCommit={(value) => void save(value)}
				/>
				{operation.failure && (
					<p className="modal-error" role="alert">
						{operation.failure}
					</p>
				)}
				<footer>
					<Button disabled={operation.busy} onClick={onClose}>
						Cancel
					</Button>
					<Button
						variant="primary"
						disabled={
							operation.busy ||
							!authorityCurrent ||
							!normalizePlaybackPageName(name)
						}
						onClick={() => void save()}
					>
						{operation.pending === "rename" ? "Renaming…" : "Rename Page"}
					</Button>
				</footer>
			</section>
		</div>,
		document.body,
	);
}
