import { Button } from "@tosklight/ui";
import { useEffect, useState, type RefObject } from "react";

/** Visible touch controls supplement swipe and scrollbars on constrained software layouts. */
export function PlaybackBankScrollControls({
	viewport,
}: {
	viewport: RefObject<HTMLDivElement | null>;
}) {
	const [edges, setEdges] = useState({
		left: true,
		right: false,
		up: true,
		down: false,
	});
	useEffect(() => {
		const element = viewport.current;
		if (!element) return;
		const update = () =>
			setEdges({
				left: element.scrollLeft <= 1,
				right:
					element.scrollLeft + element.clientWidth >= element.scrollWidth - 1,
				up: element.scrollTop <= 1,
				down:
					element.scrollTop + element.clientHeight >= element.scrollHeight - 1,
			});
		update();
		element.addEventListener("scroll", update);
		const observer =
			typeof ResizeObserver === "undefined" ? null : new ResizeObserver(update);
		observer?.observe(element);
		if (element.firstElementChild) observer?.observe(element.firstElementChild);
		window.addEventListener("resize", update);
		return () => {
			element.removeEventListener("scroll", update);
			observer?.disconnect();
			window.removeEventListener("resize", update);
		};
	}, [viewport]);
	const scroll = (x: number, y: number) => {
		const element = viewport.current;
		if (element)
			element.scrollBy({
				left: x * element.clientWidth * 0.8,
				top: y * element.clientHeight * 0.8,
				behavior: "auto",
			});
	};
	return (
		<nav
			className="playback-bank-scroll-controls"
			aria-label="Scroll Playbacks"
		>
			<span>Swipe or scroll for more Playbacks</span>
			<Button
				disabled={edges.left}
				aria-label="Scroll Playbacks left"
				onClick={() => scroll(-1, 0)}
			>
				←
			</Button>
			<Button
				disabled={edges.right}
				aria-label="Scroll Playbacks right"
				onClick={() => scroll(1, 0)}
			>
				→
			</Button>
			<Button
				disabled={edges.up}
				aria-label="Scroll Playbacks up"
				onClick={() => scroll(0, -1)}
			>
				↑
			</Button>
			<Button
				disabled={edges.down}
				aria-label="Scroll Playbacks down"
				onClick={() => scroll(0, 1)}
			>
				↓
			</Button>
		</nav>
	);
}
