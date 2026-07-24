import { useEffect } from "react";
import { retainEquivalent } from "./pollingEquivalence";
import type { ServerState } from "./useServerState";

function useMediaPreviewCleanup(state: ServerState) {
	const { bootstrap, mediaPreviewUrlsRef, setMediaPreviewUrls } = state;
	useEffect(
		() => () => {
			for (const url of Object.values(mediaPreviewUrlsRef.current))
				URL.revokeObjectURL(url);
		},
		[mediaPreviewUrlsRef],
	);
	useEffect(() => {
		for (const url of Object.values(mediaPreviewUrlsRef.current))
			URL.revokeObjectURL(url);
		mediaPreviewUrlsRef.current = {};
		setMediaPreviewUrls({});
	}, [bootstrap?.active_show?.id, mediaPreviewUrlsRef, setMediaPreviewUrls]);
}

function useDeskLockPolling(state: ServerState) {
	const { api, session, deskLockStore } = state;
	useEffect(() => {
		if (!session) return;
		let cancelled = false;
		const refresh = () =>
			void api.desk
				.deskLock()
				.then((value) => !cancelled && deskLockStore.install(value))
				.catch(() => undefined);
		refresh();
		const timer = window.setInterval(refresh, 500);
		return () => {
			cancelled = true;
			window.clearInterval(timer);
		};
	}, [api, session, deskLockStore]);
}

function useHighlightPolling(state: ServerState) {
	const {
		api,
		session,
		highlightEpoch,
		highlightErrorSticky,
		highlightWrite,
		setHighlight,
		setHighlightError,
	} = state;
	useEffect(() => {
		if (!session) {
			highlightEpoch.current += 1;
			highlightErrorSticky.current = false;
			setHighlight(null);
			setHighlightError(null);
			return;
		}
		let cancelled = false;
		const load = () => {
			const request = ++highlightEpoch.current;
			void highlightWrite.current
				.catch(() => undefined)
				.then(() => api.mediaOutput.highlight())
				.then((next) => {
					if (cancelled || request !== highlightEpoch.current) return;
					setHighlight((current) => retainEquivalent(current, next));
					if (!highlightErrorSticky.current) setHighlightError(null);
				})
				.catch((reason) => {
					if (!cancelled && request === highlightEpoch.current)
						setHighlightError(
							reason instanceof Error ? reason.message : String(reason),
						);
				});
		};
		load();
		const timer = window.setInterval(load, 2_000);
		return () => {
			cancelled = true;
			window.clearInterval(timer);
		};
	}, [
		api,
		session,
		highlightEpoch,
		highlightErrorSticky,
		highlightWrite,
		setHighlight,
		setHighlightError,
	]);
}

function useMatterPolling(state: ServerState) {
	const { api, configuration, session, setMatter } = state;
	useEffect(() => {
		if (!session || !configuration?.matter_enabled) return;
		let cancelled = false;
		const poll = () =>
			void api.desk
				.matterStatus()
				.then(
					(next) =>
						!cancelled &&
						setMatter((current) => retainEquivalent(current, next)),
				)
				.catch(() => undefined);
		poll();
		const timer = window.setInterval(poll, 1_000);
		return () => {
			cancelled = true;
			window.clearInterval(timer);
		};
	}, [api, configuration?.matter_enabled, session, setMatter]);
}

export function useServerPolling(state: ServerState) {
	useMediaPreviewCleanup(state);
	useDeskLockPolling(state);
	useHighlightPolling(state);
	useMatterPolling(state);
}
