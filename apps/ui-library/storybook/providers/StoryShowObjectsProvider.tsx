import type { ReactNode } from "react";
import { ShowObjectsViewProvider } from "../../../light-desktop/src/features/showObjects/ShowObjectsView";
import { ShowObjectsStore } from "../../../light-desktop/src/features/showObjects/store";

const store = new ShowObjectsStore();

export function StoryShowObjectsProvider({
	children,
}: {
	children: ReactNode;
}) {
	const unavailable = async () => {
		throw new Error("The isolated Storybook fixture has no live show");
	};
	return (
		<ShowObjectsViewProvider
			showId={null}
			store={store}
			transport={null}
			loadCollection={unavailable}
			loadObject={unavailable}
		>
			{children}
		</ShowObjectsViewProvider>
	);
}
