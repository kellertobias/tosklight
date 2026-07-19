import { useTextEditorController } from "./textEditorWindow/controller";
import { TextEditorContent } from "./textEditorWindow/EditorContent";
import { TextEditorMessages } from "./textEditorWindow/Messages";
import {
	TextEditorPaneChrome,
	TextEditorToolbar,
} from "./textEditorWindow/Toolbar";
import type { WindowProps } from "./windowTypes";

export { listTextEditorFiles } from "./textEditorWindow/files";

export function TextEditorWindow({ paneId }: WindowProps) {
	const controller = useTextEditorController(paneId);
	return (
		<section
			className="text-editor"
			aria-label="Text Editor"
			data-dirty={controller.dirty || undefined}
		>
			<TextEditorPaneChrome controller={controller} />
			<TextEditorToolbar controller={controller} />
			<TextEditorMessages controller={controller} />
			<TextEditorContent controller={controller} />
		</section>
	);
}
