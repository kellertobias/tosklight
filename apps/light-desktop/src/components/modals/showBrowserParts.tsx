import { Button } from "@tosklight/ui";
import {
	type FileKind,
	FileKindIcon,
	TreeChevron,
} from "../../windows/fileManagerWindow/FileKindIcon";
import "./showBrowser.css";

/** One table cell label shared by New Show, Load Show and Save Show: kind icon, name, detail. */
export function ShowEntryName({
	kind,
	name,
	detail,
}: {
	kind: FileKind;
	name: string;
	detail?: string;
}) {
	return (
		<span className="show-entry-name">
			<FileKindIcon kind={kind} />
			<span className="show-entry-label">
				<strong>{name}</strong>
				{detail && <small>{detail}</small>}
			</span>
		</span>
	);
}

/**
 * The folder a show browser is looking at, read left to right. Every step is a button back to
 * that folder, the current one is marked, and Up one folder sits first so it never moves.
 */
export function ShowLocationBar({
	rootLabel,
	rootKind = "location-shows",
	path,
	busy,
	onNavigate,
}: {
	rootLabel: string;
	rootKind?: FileKind;
	path: string;
	busy: boolean;
	onNavigate: (path: string) => void;
}) {
	const parts = path ? path.split("/") : [];
	return (
		<nav className="show-location-bar" aria-label="Location">
			<Button
				className="show-location-up"
				disabled={busy || !parts.length}
				onClick={() => onNavigate(parts.slice(0, -1).join("/"))}
			>
				Up one folder
			</Button>
			<div className="show-location-steps">
				<Button
					variant="ghost"
					disabled={busy}
					aria-current={parts.length ? undefined : "location"}
					onClick={() => onNavigate("")}
				>
					<FileKindIcon kind={rootKind} />
					{rootLabel}
				</Button>
				{parts.map((part, index) => (
					<span className="show-location-step" key={`${part}-${index}`}>
						<TreeChevron expanded={false} />
						<Button
							variant="ghost"
							disabled={busy}
							aria-current={index === parts.length - 1 ? "location" : undefined}
							onClick={() => onNavigate(parts.slice(0, index + 1).join("/"))}
						>
							{part}
						</Button>
					</span>
				))}
			</div>
		</nav>
	);
}
