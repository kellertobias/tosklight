import { type ReactNode, useEffect, useRef, useState } from "react";
import { requiredMainPlaybackHeight } from "./requiredMainPlaybackHeight";

/** A screen fits its authored rows; only personal panes offer scrolling. */
export function ScreenPlaybackFit({
	children,
	className = "",
}: {
	children: ReactNode;
	className?: string;
}) {
	const element = useRef<HTMLDivElement>(null);
	const [insufficient, setInsufficient] = useState(false);
	useEffect(() => {
		const host = element.current;
		if (!host) return;
		const shell = host.closest<HTMLElement>(".app-shell");
		let frame = 0;
		let ownsMainHeight = false;
		let observedBank: HTMLElement | null = null;
		const measure = () => {
			cancelAnimationFrame(frame);
			frame = requestAnimationFrame(() => {
				const bank = host.querySelector<HTMLElement>(".playback-fader-bank");
				if (bank !== observedBank) {
					if (observedBank) resize?.unobserve(observedBank);
					if (bank) resize?.observe(bank);
					observedBank = bank;
				}
				if (!bank || !bank.classList.contains("touch-layout")) {
					if (bank) bank.inert = false;
					if (ownsMainHeight)
						shell?.style.removeProperty("--screen-playback-required-height");
					setInsufficient(false);
					return;
				}
				const initialBounds = bank.getBoundingClientRect();
				if (!initialBounds.width || !initialBounds.height) return;
				if (shell && bank.dataset.playbackSurface === "main") {
					ownsMainHeight = true;
					const base = getComputedStyle(
						document.documentElement,
					).getPropertyValue("--control-height");
					shell.style.setProperty("--screen-playback-base-height", base);
					shell.style.setProperty(
						"--screen-playback-required-height",
						`${requiredMainPlaybackHeight(bank)}px`,
					);
				}
				// Allocating Main's required height can reflow the bank in this frame.
				// Validate controls and position feedback against that resulting geometry.
				const bounds = bank.getBoundingClientRect();
				if (!bounds.width || !bounds.height) return;
				const targets = bank.querySelectorAll<HTMLElement>(
					"article, .single-button-playback-action, .vertical-touch-fader, .vertical-touch-fader-actions button, .faderless-playback-actions button, .touch-encoder-push, .touch-encoder-number-entry",
				);
				// Pane tiles preserve the authored count even in a dense layout.
				const minimumSize = bank.dataset.playbackPresentation === "tiles" ? 1 : 44;
				const invalid = [...targets].some((target) => {
					const box = target.getBoundingClientRect();
					const card =
						target.closest("article")?.getBoundingClientRect() ?? bounds;
					return (
						box.width < minimumSize ||
						box.height < minimumSize ||
						box.left < bounds.left - 0.5 ||
						box.right > bounds.right + 0.5 ||
						box.top < bounds.top - 0.5 ||
						box.bottom > bounds.bottom + 0.5 ||
						box.left < card.left - 0.5 ||
						box.right > card.right + 0.5 ||
						box.top < card.top - 0.5 ||
						box.bottom > card.bottom + 0.5
					);
				});
				bank.inert = invalid;
				const origin = host.getBoundingClientRect();
				for (const [name, value] of Object.entries({
					top: bounds.top - origin.top,
					left: bounds.left - origin.left,
					width: bounds.width,
					height: bounds.height,
				})) {
					host.style.setProperty(`--screen-fit-${name}`, `${value}px`);
				}
				setInsufficient(invalid);
			});
		};
		const resize =
			typeof ResizeObserver === "undefined"
				? null
				: new ResizeObserver(measure);
		resize?.observe(host);
		window.addEventListener("resize", measure);
		const changes = new MutationObserver(measure);
		changes.observe(host.firstElementChild ?? host, {
			subtree: true,
			childList: true,
			attributes: true,
			attributeFilter: ["style", "class"],
		});
		measure();
		return () => {
			resize?.disconnect();
			window.removeEventListener("resize", measure);
			changes.disconnect();
			cancelAnimationFrame(frame);
			const bank = host.querySelector<HTMLElement>(".playback-fader-bank");
			if (bank) bank.inert = false;
			if (ownsMainHeight)
				shell?.style.removeProperty("--screen-playback-required-height");
			if (ownsMainHeight)
				shell?.style.removeProperty("--screen-playback-base-height");
		};
	}, []);
	return (
		<div ref={element} className={`screen-playback-fit ${className}`.trim()}>
			<div className="screen-playback-fit-content">{children}</div>
			{insufficient && (
				<div className="screen-playback-fit-status" role="status">
					This Playback layout needs more space for 44px touch controls.
					Configure fewer controls or enlarge this screen’s control area.
				</div>
			)}
		</div>
	);
}
