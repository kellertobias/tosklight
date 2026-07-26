import { Button } from "@tosklight/ui";
import { type ReactNode, useRef } from "react";
import appMark from "../../../src-tauri/icons/mark-shadow.svg";
import { useActiveShow } from "../../features/deskSnapshot/DeskSnapshotState";
import { useApp } from "../../state/AppContext";
import type { BuiltInWindow } from "../../types";
import { DeskSettingsModal } from "../modals/DeskSettingsModal";
import { Clock } from "./Clock";
import { type ShowIndicator, useShowIndicator } from "./showIndicator";

export const builtIns: Array<[BuiltInWindow, string, string]> = [
	["stage", "⌖", "Stage"],
	["fixtures", "♙", "Fixtures"],
	["presets", "▣", "Presets"],
	["cuelists", "▶", "Cuelists"],
	["dynamics", "∿", "Dynamics"],
	["channels", "▥", "Channels"],
];

function DockEntryContent({ icon, label }: { icon: string; label: string }) {
	return (
		<>
			<span className="dock-entry-icon" aria-hidden="true">
				{icon}
			</span>
			<span className="dock-entry-label">{label}</span>
		</>
	);
}

export function LeftDock({
	presentation,
}: {
	presentation?: {
		showIdentity?: string;
		showIndicator?: ShowIndicator;
		clock?: ReactNode;
	};
} = {}) {
	const { state, dispatch } = useApp();
	const activeShow = useActiveShow();
	const longPress = useRef<number | null>(null);
	const held = useRef(false);
	const suppressUntil = useRef(0);
	const runtimeShowIndicator = useShowIndicator();
	const showIndicator = presentation?.showIndicator ?? runtimeShowIndicator;
	const showIdentity =
		presentation?.showIdentity ??
		(activeShow?.revision_copy
			? `Revision Copy · ${activeShow.name}`
			: (activeShow?.name ?? "Show"));
	const identityDetail = activeShow?.revision_copy
		? `${showIdentity}. Source: ${activeShow.revision_copy.show_name}, Revision ${activeShow.revision_copy.revision} · ${activeShow.revision_copy.revision_name}. Created ${new Date(activeShow.revision_copy.copied_at).toLocaleString()}. ${showIndicator.detail}`
		: `${showIndicator.label}. ${showIndicator.detail}`;
	const showingDesktops = state.dockMode === "desks";
	const nextMode = showingDesktops ? "builtins" : "desks";

	return (
		<aside className="left-dock">
			<Button
				className={`dock-identity ${activeShow?.revision_copy ? "revision-copy-active" : ""}`}
				aria-label={`Open show menu. ${identityDetail}`}
				title={identityDetail}
				onClick={() =>
					dispatch({ type: "SET_MODAL", modal: "setupOpen", value: true })
				}
			>
				<span className="app-mark">
					<img src={appMark} alt="" />
				</span>
				{presentation?.clock ?? <Clock />}
				<b>
					<span
						className={`show-status-dot ${showIndicator.className}`}
						aria-hidden="true"
					>
						●
					</span>{" "}
					{showIdentity}
				</b>
			</Button>

			<Button
				className="dock-mode-toggle"
				aria-label="Desktops / Built-ins"
				data-dock-mode={state.dockMode}
				title={`Show ${showingDesktops ? "Built-ins" : "Desktops"}`}
				onClick={() => dispatch({ type: "SET_DOCK_MODE", mode: nextMode })}
			>
				<span className={`dock-mode-option ${showingDesktops ? "active" : ""}`}>
					Desktops
				</span>
				<span className="dock-mode-divider" aria-hidden="true">
					/
				</span>
				<span className={`dock-mode-option ${showingDesktops ? "" : "active"}`}>
					Built-ins
				</span>
			</Button>

			{showingDesktops ? (
				<nav
					key="desktops"
					className="dock-list dock-list-swap dock-list-swap-desktops"
					aria-label="Desktops"
				>
					{state.desks.map((desk) => (
						<Button
							key={desk.id}
							data-desktop-id={desk.id}
							aria-label={desk.name}
							aria-current={state.activeDeskId === desk.id ? "page" : undefined}
							className={`dock-entry ${state.activeDeskId === desk.id ? "active" : ""}`}
							onPointerDown={() => {
								held.current = false;
								longPress.current = window.setTimeout(() => {
									held.current = true;
									suppressUntil.current = performance.now() + 1000;
									dispatch({ type: "OPEN_DESK_SETTINGS", id: desk.id });
								}, 650);
							}}
							onPointerUp={() => {
								if (longPress.current) window.clearTimeout(longPress.current);
								longPress.current = null;
							}}
							onPointerCancel={() => {
								if (longPress.current) window.clearTimeout(longPress.current);
								longPress.current = null;
							}}
							onClick={() => {
								if (
									!held.current &&
									performance.now() >= suppressUntil.current
								) {
									dispatch({ type: "OPEN_DESK", id: desk.id });
								}
								held.current = false;
							}}
						>
							<DockEntryContent icon={desk.icon ?? "⊞"} label={desk.name} />
						</Button>
					))}
					<Button
						className="dock-entry"
						aria-label="New desktop"
						onClick={() => dispatch({ type: "NEW_DESK" })}
					>
						<DockEntryContent icon="＋" label="New desktop" />
					</Button>
				</nav>
			) : (
				<nav
					key="builtins"
					className="dock-list builtins-list dock-list-swap dock-list-swap-builtins"
					aria-label="Built-ins"
				>
					{builtIns.map(([kind, icon, label]) => (
						<Button
							key={kind}
							aria-label={label}
							aria-current={state.builtIn === kind ? "page" : undefined}
							className={`dock-entry ${state.builtIn === kind ? "active" : ""}`}
							onClick={() => dispatch({ type: "OPEN_BUILTIN", kind })}
						>
							<DockEntryContent icon={icon} label={label} />
						</Button>
					))}
				</nav>
			)}
			<DeskSettingsModal />
		</aside>
	);
}
