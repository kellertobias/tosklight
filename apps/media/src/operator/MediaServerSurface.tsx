import type { ReactNode } from "react";

export const MEDIA_SERVER_SECTIONS = [
	{ id: "dashboard", label: "Dashboard", icon: "⌂" },
	{ id: "media", label: "Media", icon: "▣" },
	{ id: "library", label: "Library", icon: "▦" },
	{ id: "visualizers", label: "Visualizers", icon: "✦" },
	{ id: "text", label: "Text", icon: "T" },
	{ id: "settings", label: "Settings", icon: "⚙" },
] as const;

export type MediaServerSection = (typeof MEDIA_SERVER_SECTIONS)[number]["id"];

export function MediaServerShell({
	active,
	connected,
	instance,
	now,
	children,
	onNavigate,
}: {
	active: MediaServerSection;
	connected: boolean;
	instance?: string;
	now: Date;
	children: ReactNode;
	onNavigate?: (section: MediaServerSection) => void;
}) {
	return (
		<div className="media-operator-shell">
			<aside className="media-operator-dock" aria-label="Media Server sections">
				<div className="media-operator-identity">
					<div
						className="media-operator-mark"
						role="img"
						aria-label="ToskLight Media Server"
					>
						<span aria-hidden="true">M</span>
					</div>
					<time dateTime={now.toISOString()}>
						{now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
					</time>
				</div>
				<nav>
					{MEDIA_SERVER_SECTIONS.map((section) => (
						<button
							key={section.id}
							type="button"
							aria-current={active === section.id ? "page" : undefined}
							onClick={() => onNavigate?.(section.id)}
						>
							<span className="media-operator-dock-icon" aria-hidden="true">
								{section.icon}
							</span>
							<span>{section.label}</span>
						</button>
					))}
				</nav>
				<div
					className={`media-operator-connection ${connected ? "is-connected" : "is-disconnected"}`}
				>
					<span aria-hidden="true" />
					<strong>{connected ? "Connected" : "Offline"}</strong>
					<small>{instance ?? "Media Server"}</small>
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
		<header className="media-screen-header">
			<div>
				{eyebrow && <p className="media-screen-eyebrow">{eyebrow}</p>}
				<h1>{title}</h1>
				<p>{detail}</p>
			</div>
			{actions && <div className="media-screen-actions">{actions}</div>}
		</header>
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
			{(title || detail) && (
				<header>
					{title && <h2>{title}</h2>}
					{detail && <p>{detail}</p>}
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
			<section className="media-list-detail-list" aria-label={label}>
				{items.map((item) => (
					<button
						key={item.id}
						type="button"
						aria-current={item.id === selectedId ? "true" : undefined}
						onClick={() => onSelect?.(item.id)}
					>
						<span>
							<strong>{item.title}</strong>
							<small>{item.detail}</small>
						</span>
						{item.meta && <em>{item.meta}</em>}
					</button>
				))}
			</section>
			<section className="media-list-detail-content">{detail}</section>
		</div>
	);
}

export function MediaPreview({
	title,
	variant = "aurora",
	children,
}: {
	title: string;
	variant?: "aurora" | "particles" | "text" | "media";
	children?: ReactNode;
}) {
	return (
		<figure
			className={`media-preview is-${variant}`}
			aria-label={`${title} preview`}
		>
			<div className="media-preview-picture">
				<span className="media-preview-grid" aria-hidden="true" />
				{children ?? <span className="media-preview-orb" aria-hidden="true" />}
			</div>
			<figcaption>
				<strong>{title}</strong>
				<span>Preview</span>
			</figcaption>
		</figure>
	);
}

export const MEDIA_SETTINGS_SECTIONS = [
	{ id: "libraries", label: "Libraries" },
	{ id: "outputs", label: "Outputs" },
	{ id: "network-inputs", label: "Network & Inputs" },
	{ id: "logs", label: "Logs" },
] as const;

export type MediaSettingsSection =
	(typeof MEDIA_SETTINGS_SECTIONS)[number]["id"];

export function MediaSettingsLayout({
	active,
	onSelect,
	children,
}: {
	active: MediaSettingsSection;
	onSelect?: (section: MediaSettingsSection) => void;
	children: ReactNode;
}) {
	return (
		<div className="media-settings-layout">
			<nav aria-label="Media Server settings">
				{MEDIA_SETTINGS_SECTIONS.map((section) => (
					<button
						key={section.id}
						type="button"
						aria-current={active === section.id ? "page" : undefined}
						onClick={() => onSelect?.(section.id)}
					>
						{section.label}
					</button>
				))}
			</nav>
			<div className="media-settings-content">{children}</div>
		</div>
	);
}
