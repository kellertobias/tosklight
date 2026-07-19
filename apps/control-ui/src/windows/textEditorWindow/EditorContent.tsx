import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { TextArea } from "../../components/common";
import type { TextEditorController } from "./controller";

export function TextEditorContent({
	controller,
}: {
	controller: TextEditorController;
}) {
	return (
		<div className={`text-editor-content mode-${controller.editorMode}`}>
			{controller.editorMode !== "markdown" && (
				<TextArea
					ref={controller.textarea}
					aria-label="File text"
					aria-describedby={
						controller.notice ? controller.messageId : undefined
					}
					value={controller.text}
					readOnly={
						!controller.document ||
						controller.paneReadOnly ||
						controller.document.read_only ||
						controller.availability === "missing"
					}
					onBlur={controller.persistViewState}
					onChange={(event) => controller.changeText(event.target.value)}
					onKeyDown={(event) => {
						if (
							(event.metaKey || event.ctrlKey) &&
							event.key.toLowerCase() === "s"
						) {
							event.preventDefault();
							if (event.shiftKey) controller.saveAs();
							else controller.save();
						}
					}}
					placeholder={
						controller.availability === "missing"
							? "The associated file is missing."
							: "Choose a UTF-8 text file to begin."
					}
				/>
			)}
			{controller.editorMode !== "plain" && (
				<article
					className="text-editor-markdown"
					aria-label="Rendered Markdown"
				>
					<ReactMarkdown remarkPlugins={[remarkGfm]}>
						{controller.text}
					</ReactMarkdown>
				</article>
			)}
		</div>
	);
}
