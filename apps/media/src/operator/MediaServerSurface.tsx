import { OperatorDestinationList } from "@tosklight/ui/application";
import type { TitleActionGroup } from "@tosklight/ui/controls";
import { Button } from "@tosklight/ui/controls";
import {
	SelectionList,
	WindowFrame,
	WindowHeader,
} from "@tosklight/ui/window-kit";
import type { ReactNode } from "react";

export const MEDIA_SERVER_SECTIONS = [
	{ id: "media", label: "Playback", icon: "▣" },
	{ id: "library", label: "Library", icon: "▦" },
	{ id: "audio", label: "Audio", icon: "≋" },
	{ id: "dmx", label: "DMX", icon: "↯" },
	{ id: "settings", label: "Settings", icon: "⚙" },
] as const;

export type MediaServerSection = (typeof MEDIA_SERVER_SECTIONS)[number]["id"];

export function MediaServerShell({
	active,
	connected,
	instance,
	showName,
	now,
	children,
	playbackOwnership,
	onNavigate,
}: {
	active: MediaServerSection;
	connected: boolean;
	instance?: string;
	showName?: string;
	now: Date;
	children: ReactNode;
	playbackOwnership?: ReactNode;
	onNavigate?: (section: MediaServerSection) => void;
}) {
	return (
		<div className="media-operator-shell">
			<aside
				className="left-dock media-operator-dock"
				aria-label="Media Server sections"
			>
				<Button
					className="dock-identity media-operator-identity"
					aria-label="ToskLight Media Server"
				>
					<div
						className="app-mark media-operator-mark"
						role="img"
						aria-hidden="true"
					>
						<img
							src={
								new URL(
									"../../../../assets/branding/ToskLight Pixel.svg",
									import.meta.url,
								).href
							}
							alt=""
						/>
					</div>
					<time dateTime={now.toISOString()}>
						{now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
					</time>
					<b>Media Server</b>
				</Button>
				<OperatorDestinationList
					ariaLabel="Media Server destinations"
					entries={MEDIA_SERVER_SECTIONS}
					activeId={active}
					onSelect={(id) => onNavigate?.(id as MediaServerSection)}
				/>
				<section
					id="media-playback-dock-action"
					aria-label="Playback ownership"
				>
					{playbackOwnership}
				</section>
				<div
					className={`media-operator-connection ${connected ? "is-connected" : "is-disconnected"}`}
				>
					<span aria-hidden="true" />
					<strong>{connected ? "Connected" : "Offline"}</strong>
					<small>
						{showName
							? `Light Desk · ${showName}`
							: (instance ?? "Media Server")}
					</small>
				</div>
			</aside>
			<main className="media-operator-workspace">{children}</main>
		</div>
	);
}

export function MediaScreenHeader({
	eyebrow,
	title,
	detail,
	actions,
}: {
	eyebrow?: string;
	title: string;
	detail: string;
	actions?: ReactNode;
}) {
	return (
		<div className="media-screen-header">
			<h1 className="media-visually-hidden">{title}</h1>
			<WindowHeader
				title={title}
				info={{ primary: eyebrow ?? "Media Server", secondary: detail }}
				toolbar={actions}
			/>
		</div>
	);
}

export function MediaPanel({
	title,
	detail,
	children,
	className = "",
}: {
	title?: string;
	detail?: string;
	children: ReactNode;
	className?: string;
}) {
	return (
		<section className={`media-operator-panel ${className}`.trim()}>
			{title && (
				<header className="media-operator-panel-heading">
					<h2>{title}</h2>
					{detail && <small>{detail}</small>}
				</header>
			)}
			{children}
		</section>
	);
}

export function MediaMetric({
	label,
	value,
	detail,
	tone = "neutral",
}: {
	label: string;
	value: string;
	detail: string;
	tone?: "neutral" | "good" | "warn";
}) {
	return (
		<article className={`media-operator-metric is-${tone}`}>
			<span>{label}</span>
			<strong>{value}</strong>
			<small>{detail}</small>
		</article>
	);
}

export function MediaListDetail({
	label,
	items,
	selectedId,
	onSelect,
	detail,
}: {
	label: string;
	items: Array<{ id: string; title: string; detail: string; meta?: string }>;
	selectedId: string;
	onSelect?: (id: string) => void;
	detail: ReactNode;
}) {
	return (
		<div className="media-list-detail">
			<SelectionList
				className="media-list-detail-list"
				ariaLabel={label}
				value={selectedId}
				options={items.map((item) => ({
					value: item.id,
					label: (
						<>
							<strong>{item.title}</strong>
							{item.meta && <em>{item.meta}</em>}
						</>
					),
					description: item.detail,
				}))}
				onChange={(id) => onSelect?.(id)}
			/>
			<section className="media-list-detail-content">{detail}</section>
		</div>
	);
}

export function MediaPreview({
	title,
	meta = "Preview",
	variant = "aurora",
	children,
	aspectRatio,
}: {
	title: string;
	meta?: string;
	variant?: "aurora" | "particles" | "text" | "media";
	children?: ReactNode;
	aspectRatio?: number;
}) {
	return (
		<figure
			className={`media-preview is-${variant}`}
			aria-label={`${title} preview`}
		>
			<div
				className="media-preview-picture"
				style={aspectRatio ? { aspectRatio } : undefined}
			>
				<span className="media-preview-grid" aria-hidden="true" />
				{children ?? <span className="media-preview-orb" aria-hidden="true" />}
			</div>
			<figcaption>
				<strong>{title}</strong>
				<span>{meta}</span>
			</figcaption>
		</figure>
	);
}

export const MEDIA_SETTINGS_SECTIONS = [
	{ id: "libraries", label: "Libraries" },
	{ id: "picture-output", label: "Picture" },
	{ id: "sound-output", label: "Sound" },
	{ id: "network", label: "Network" },
	{ id: "dmx", label: "DMX" },
	{ id: "logs", label: "Logs" },
] as const;

export type MediaSettingsSection =
	(typeof MEDIA_SETTINGS_SECTIONS)[number]["id"];

export function MediaSettingsLayout({
	active,
	onSelect,
	groups = [],
	children,
}: {
	active: MediaSettingsSection;
	onSelect?: (section: MediaSettingsSection) => void;
	groups?: TitleActionGroup[];
	children: ReactNode;
}) {
	return (
		<WindowFrame
			title="Settings"
			info={{
				primary: "Media Server",
				secondary: "Output, network, and service configuration",
			}}
			className="media-settings-window"
			groups={[
				{
					id: "settings-sections",
					kind: "tabs",
					activeId: active,
					onActiveChange: (id) => onSelect?.(id as MediaSettingsSection),
					actions: MEDIA_SETTINGS_SECTIONS.map((section) => ({
						id: section.id,
						label: section.label,
					})),
				},
				...groups,
			]}
		>
			<div className="media-settings-content">{children}</div>
		</WindowFrame>
	);
}
