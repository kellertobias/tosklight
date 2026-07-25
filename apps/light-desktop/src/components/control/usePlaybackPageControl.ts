import { useEffect, useState } from "react";
import {
	usePlaybackDeskView,
	usePlaybackRuntimeActions,
	usePlaybackRuntimeStatus,
} from "../../features/playbackRuntime/PlaybackRuntimeView";
import { usePlaybackTopologyActions } from "../../features/playbackTopology/PlaybackTopologyProvider";
import { usePlaybackPagesView } from "../../features/playbackTopology/PlaybackTopologyView";
import type { ShowObject } from "../../features/showObjects/contracts";
import { useApp } from "../../state/AppContext";
import { canAdvancePlaybackPage } from "./PlaybackPageDialogs";
import { useScopedPageOperation } from "./useScopedPageOperation";

type PlaybackPageObject = ShowObject<"playback_page">;

export function usePlaybackPageControl() {
	const { state, dispatch } = useApp();
	const desk = usePlaybackDeskView();
	const runtimeActions = usePlaybackRuntimeActions();
	const runtimeStatus = usePlaybackRuntimeStatus();
	const topology = usePlaybackPagesView();
	const topologyActions = usePlaybackTopologyActions();
	const [pagePickerOpen, setPagePickerOpen] = useState(false);
	const [renamePage, setRenamePage] = useState<PlaybackPageObject | null>(null);
	const runtimeReady = runtimeStatus.status === "ready";
	const activePageNumber = runtimeReady ? (desk?.active_page ?? null) : null;
	const activePage = topology.pages.find(
		(page) => page.body.number === activePageNumber,
	);
	const ready = topology.ready && runtimeReady && activePage != null;
	const operation = useScopedPageOperation([
		topology.ready,
		runtimeReady,
		desk !== null,
		activePage != null,
		topologyActions?.createPage,
		runtimeActions?.setActivePage,
	]);
	useEffect(() => {
		setPagePickerOpen(false);
		setRenamePage(null);
	}, [operation.generation]);
	const pages = topology.pages.map((page) => page.body);
	const previousPageNumber = ready && activePageNumber != null
		? existingPageNumber(pages, activePageNumber - 1)
		: null;
	const openPageMenu = () => {
		if (!ready) return;
		if (!state.playbackSetArmed) return setPagePickerOpen(true);
		dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
		setRenamePage(activePage);
	};
	const selectPage = async (target: number) => {
		const setActivePage = runtimeActions?.setActivePage;
		const token = ready && setActivePage ? operation.begin("select") : null;
		if (token == null || !setActivePage) return;
		const selected = await setActivePage(target);
		if (!operation.complete(token, selected ? null : selectionFailure(target)))
			return;
		if (!selected) setPagePickerOpen(true);
	};
	const nextPage = async () => {
		const createPage = topologyActions?.createPage;
		const setActivePage = runtimeActions?.setActivePage;
		const token =
			ready && activePageNumber != null && setActivePage
				? operation.begin("next")
				: null;
		if (token == null || !setActivePage || activePageNumber == null) return;
		const target = activePageNumber + 1;
		if (!existingPageNumber(pages, target)) {
			if (!createPage)
				return finishFailedCreate(operation, token, target, setPagePickerOpen);
			const created = await createPage(target);
			if (!operation.isCurrent(token)) return;
			if (!created) return finishFailedCreate(operation, token, target, setPagePickerOpen);
		}
		const selected = await setActivePage(target);
		if (!operation.complete(token, selected ? null : selectionFailure(target)))
			return;
		if (!selected) setPagePickerOpen(true);
	};
	return {
		activePageNumber,
		busy: operation.busy,
		canAdvance:
			ready &&
			activePageNumber != null &&
			canAdvancePlaybackPage(pages, activePageNumber),
		currentPageName: activePage?.body.name ?? "Loading…",
		pageFailure: operation.failure,
		pagePickerOpen,
		previousPageNumber,
		ready,
		renamePage,
		closeMenu: () => {
			setPagePickerOpen(false);
			operation.report(null);
		},
		closeRename: () => setRenamePage(null),
		nextPage,
		openPageMenu,
		selectPage,
	};
}

function existingPageNumber(
	pages: readonly { number: number }[],
	number: number,
) {
	return pages.some((page) => page.number === number) ? number : null;
}

function selectionFailure(page: number) {
	return `Playback Page ${page} could not be selected.`;
}

function finishFailedCreate(
	operation: ReturnType<typeof useScopedPageOperation>,
	token: number,
	page: number,
	open: (value: boolean) => void,
) {
	if (operation.complete(token, `Playback Page ${page} could not be created.`))
		open(true);
}
