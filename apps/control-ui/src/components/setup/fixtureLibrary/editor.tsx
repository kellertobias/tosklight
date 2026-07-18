import { useState } from "react";
import { useServer } from "../../../api/ServerContext";
import type { FixtureDefinition, FixtureProfile } from "../../../api/types";
import { FixtureProfileEditor } from "../FixtureProfileEditor";
import {
	blankFixtureProfile,
	fixtureProfileFromDefinition,
} from "../fixtureProfileModel";

export interface FixtureLibraryEditorState {
	draft: FixtureProfile;
	expectedRevision: number;
}

export function useFixtureLibraryEditor(fixtureProfiles: FixtureProfile[]) {
	const [editor, setEditor] = useState<FixtureLibraryEditorState | null>(null);

	const openCreate = () =>
		setEditor({ draft: blankFixtureProfile(), expectedRevision: 0 });

	const openSelected = (mode: FixtureDefinition) => {
		const draft = fixtureProfileFromDefinition(mode);
		setEditor({
			draft,
			expectedRevision: Math.max(
				draft.revision,
				...fixtureProfiles
					.filter((profile) => profile.id === draft.id)
					.map((profile) => profile.revision),
			),
		});
	};

	const openRevision = (profile: FixtureProfile, expectedRevision: number) => {
		setEditor({ draft: structuredClone(profile), expectedRevision });
	};

	return {
		close: () => setEditor(null),
		editor,
		openCreate,
		openRevision,
		openSelected,
	};
}

interface FixtureLibraryEditorProps {
	editor: FixtureLibraryEditorState;
	manufacturers: string[];
	onClose: () => void;
}

export function FixtureLibraryEditor({
	editor,
	manufacturers,
	onClose,
}: FixtureLibraryEditorProps) {
	const server = useServer();
	return (
		<FixtureProfileEditor
			initialProfile={editor.draft}
			expectedRevision={editor.expectedRevision}
			manufacturers={manufacturers}
			attributeRegistry={server.bootstrap?.attribute_registry ?? []}
			onSave={server.saveFixtureProfile}
			onClose={onClose}
		/>
	);
}
