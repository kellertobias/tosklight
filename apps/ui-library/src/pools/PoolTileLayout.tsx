import {
	createContext,
	type ReactNode,
	useContext,
	useLayoutEffect,
	useState,
} from "react";

/** Automatic tiles remain square; fixed grids fill the visible pool viewport. */
export type PoolTileLayout =
	| { mode: "automatic" }
	| { mode: "fixed"; rows: number; columns: number };

const PoolTileLayoutContext = createContext<PoolTileLayout | undefined>(
	undefined,
);

export function PoolTileLayoutProvider({
	layout,
	children,
}: {
	layout?: PoolTileLayout;
	children: ReactNode;
}) {
	return (
		<PoolTileLayoutContext.Provider value={layout}>
			{children}
		</PoolTileLayoutContext.Provider>
	);
}

export function usePoolTileLayout() {
	return useContext(PoolTileLayoutContext);
}

/** Measure the scroll viewport, never the full (potentially thousands of slots) grid. */
export function useFixedPoolRowHeight(node: HTMLElement | null, rows?: number) {
	const [height, setHeight] = useState<number>();
	useLayoutEffect(() => {
		if (!node || rows === undefined) return;
		const viewport =
			node.closest<HTMLElement>(".ui-window-scroller") ?? node.parentElement;
		if (!viewport) return;
		const measure = () => {
			const styles = getComputedStyle(node);
			const padding =
				(Number.parseFloat(styles.paddingTop) || 0) +
				(Number.parseFloat(styles.paddingBottom) || 0);
			const gap = Number.parseFloat(styles.rowGap) || 0;
			const next = Math.max(
				0,
				(viewport.clientHeight - padding - gap * (rows - 1)) / rows,
			);
			setHeight((current) => (current === next ? current : next));
		};
		measure();
		if (typeof ResizeObserver === "undefined") return;
		const observer = new ResizeObserver(measure);
		observer.observe(viewport);
		observer.observe(node);
		return () => observer.disconnect();
	}, [node, rows]);
	return rows === undefined ? undefined : height;
}
