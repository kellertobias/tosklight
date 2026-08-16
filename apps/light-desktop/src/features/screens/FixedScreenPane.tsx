import { useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { FixedScreenPane as FixedScreenPaneConfiguration } from "../../api/types";
import { CuelistWindow } from "../../windows/CuelistWindow";
import { FixtureSheetWindow } from "../../windows/FixtureSheetWindow";
import { StageWindow } from "../../windows/StageWindow";
import { EXTERNAL_CHECK_INTERVAL_MILLIS } from "../../windows/textEditorWindow/files";
import {
	TEXT_FILE_SAVED_EVENT,
	textDocumentFromSavedEvent,
} from "../../windows/textFileSync";
import { useFiles } from "../files/FilesContext";

function FixedTextPane({
	root,
	path,
	mode,
}: Extract<FixedScreenPaneConfiguration, { type: "text" }>) {
	const files = useFiles();
	const filesRef = useRef(files);
	const [text, setText] = useState<string | null>(null);
	const [unavailable, setUnavailable] = useState(!root || !path);
	filesRef.current = files;

	useEffect(() => {
		if (!root || !path) {
			setText(null);
			setUnavailable(true);
			return;
		}
		let cancelled = false;
		let loading = false;
		const load = async () => {
			if (loading) return;
			loading = true;
			try {
				const document = await filesRef.current.readTextFile(root, path);
				if (!cancelled) {
					setText(document.text);
					setUnavailable(false);
				}
			} catch {
				if (!cancelled) {
					setText(null);
					setUnavailable(true);
				}
			} finally {
				loading = false;
			}
		};
		const saved = (event: Event) => {
			const detail = textDocumentFromSavedEvent(event);
			if (detail?.document.root_id === root && detail.document.path === path) {
				setText(detail.document.text);
				setUnavailable(false);
			}
		};
		void load();
		const timer = window.setInterval(
			() => void load(),
			EXTERNAL_CHECK_INTERVAL_MILLIS,
		);
		window.addEventListener(TEXT_FILE_SAVED_EVENT, saved);
		return () => {
			cancelled = true;
			window.clearInterval(timer);
			window.removeEventListener(TEXT_FILE_SAVED_EVENT, saved);
		};
	}, [root, path]);

	if (unavailable)
		return (
			<div className="fixed-screen-unavailable" role="status">
				<b>Text unavailable</b>
				<span>
					The configured text file <code>{path}</code> is missing or
					unavailable.
				</span>
			</div>
		);
	if (text == null)
		return (
			<div className="fixed-screen-unavailable" role="status">
				<b>Loading text…</b>
				<span>{path}</span>
			</div>
		);
	return mode === "markdown" ? (
		<article
			className="fixed-screen-text text-editor-markdown"
			aria-label="Rendered Markdown"
		>
			<ReactMarkdown remarkPlugins={[remarkGfm]}>{text}</ReactMarkdown>
		</article>
	) : (
		<div className="fixed-screen-text" role="document" aria-label="Plain Text">
			<pre>{text}</pre>
		</div>
	);
}

function stageRenderQuality(
	value: Extract<
		FixedScreenPaneConfiguration,
		{ type: "stage_3d" }
	>["render_quality"],
) {
	if (value === "full") return "improved_beams" as const;
	return value;
}

export function FixedScreenPane({
	pane,
}: {
	pane: FixedScreenPaneConfiguration;
}) {
	let content: React.ReactNode;
	switch (pane.type) {
		case "fixture_sheet":
			content = (
				<FixtureSheetWindow
					active
					compact
					viewOnly
					fixtureSheetIncludedHeads={
						pane.included_heads === "no_sub_heads"
							? "no-sub-heads"
							: pane.included_heads === "no_master_heads"
								? "no-master-heads"
								: "all"
					}
					fixtureSheetOrder={
						pane.order === "fixture_id" ? "fixture-id" : "active"
					}
					fixtureSheetActiveOnly={pane.active_only}
					fixtureSheetCompactMode={
						pane.compact_mode === "icon_only"
							? "icon-only"
							: pane.compact_mode === "text_only"
								? "text-only"
								: "off"
					}
					fixtureSheetCueListId={pane.cue_list_id}
					fixtureSheetColumns={pane.columns}
					fixtureSheetShowType={pane.show_type}
					showGroupShortcuts={pane.show_group_shortcuts}
				/>
			);
			break;
		case "stage_2d":
			content = (
				<StageWindow
					active
					compact
					viewOnly
					stageView="2d"
					followPreload={pane.follow_preload}
					showFloorGrid={pane.show_floor_grid}
					showSelection={false}
				/>
			);
			break;
		case "stage_3d":
			content = (
				<StageWindow
					active
					compact
					viewOnly
					stageView="3d"
					followPreload={pane.follow_preload}
					showFloorGrid={pane.show_floor_grid}
					showBeamGuides={pane.show_beam_guides}
					environmentBrightness={pane.environment_brightness}
					showSelection={false}
				/>
			);
			break;
		case "cues":
			content = (
				<CuelistWindow
					active
					compact
					viewOnly
					cueListTab="cues"
					cueListSource="fixed"
					fixedCueListId={pane.cue_list_id}
					showCueSidebar={false}
				/>
			);
			break;
		case "text":
			content = <FixedTextPane {...pane} />;
			break;
	}
	return (
		<main
			className="fixed-screen-pane"
			data-fixed-pane-type={pane.type}
			data-light-surface="fixed-screen-pane"
			aria-label={`Fixed ${pane.type.replaceAll("_", " ")}`}
		>
			{content}
		</main>
	);
}
