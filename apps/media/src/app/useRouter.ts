// A history-API router, kept to what this application actually needs: read the path, change it,
// and restore focus to the new page so a keyboard operator is never dropped at the document top.

import { useCallback, useEffect, useRef, useState } from "react";
import { type RoutePath, normalizePath } from "./routes";

export interface Router {
	path: RoutePath;
	navigate: (to: RoutePath) => void;
	/** Attach to the page heading; it receives focus after every navigation. */
	headingRef: React.RefObject<HTMLHeadingElement | null>;
}

export function useRouter(): Router {
	const [path, setPath] = useState<RoutePath>(() =>
		normalizePath(window.location.pathname),
	);
	const headingRef = useRef<HTMLHeadingElement | null>(null);
	const navigated = useRef(false);

	useEffect(() => {
		const onPopState = () => setPath(normalizePath(window.location.pathname));
		window.addEventListener("popstate", onPopState);
		return () => window.removeEventListener("popstate", onPopState);
	}, []);

	useEffect(() => {
		if (!navigated.current) return;
		headingRef.current?.focus();
	}, [path]);

	const navigate = useCallback((to: RoutePath) => {
		navigated.current = true;
		window.history.pushState(null, "", to);
		setPath(normalizePath(to));
	}, []);

	return { path, navigate, headingRef };
}
