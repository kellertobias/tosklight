import { render, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
	ShowObjectsViewProvider,
	useShowObjectView,
} from "./ShowObjectsView";
import { ShowObjectsStore } from "./store";
import type {
	ShowObjectsEventObserver,
	ShowObjectsEventTransport,
} from "./transport";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";

class FakeTransport implements ShowObjectsEventTransport {
	readonly subscriptions: Array<{
		observer: ShowObjectsEventObserver;
		close: ReturnType<typeof vi.fn>;
	}> = [];

	subscribe(
		_showId: string,
		_afterSequence: number | null,
		observer: ShowObjectsEventObserver,
	) {
		const close = vi.fn();
		this.subscriptions.push({ observer, close });
		return { close };
	}
}

function GroupConsumer({ active }: { active: boolean }) {
	useShowObjectView("group", active);
	return null;
}

describe("ShowObjectsViewProvider", () => {
	it("opens a stream only while its view is visibly active", async () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const loadCollection = vi.fn().mockResolvedValue([]);
		const view = (active: boolean) => (
			<ShowObjectsViewProvider
				showId={SHOW_ID}
				store={store}
				transport={transport}
				loadCollection={loadCollection}
			>
				<GroupConsumer active={active} />
			</ShowObjectsViewProvider>
		);
		const rendered = render(view(false));
		expect(transport.subscriptions).toHaveLength(0);

		rendered.rerender(view(true));
		await waitFor(() => expect(transport.subscriptions).toHaveLength(1));
		rendered.rerender(view(false));
		await waitFor(() =>
			expect(transport.subscriptions[0].close).toHaveBeenCalledOnce(),
		);
	});
});
