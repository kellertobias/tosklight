import { act, cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProgrammingInteractionStore } from "../programmingInteraction/store";
import { ShowObjectsStore } from "../showObjects/store";
import { CueTransferProvider, useCueTransfer } from "./CueTransferProvider";
import type { CueTransferTransport } from "./contracts";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const DESK_ID = "22222222-2222-4222-8222-222222222222";
const USER_ID = "33333333-3333-4333-8333-333333333333";
let renders = 0;

afterEach(cleanup);

function Consumer() {
	renders += 1;
	useCueTransfer();
	return null;
}

describe("CueTransferProvider", () => {
	it("is dormant on mount and isolates consumers from unrelated store changes", async () => {
		renders = 0;
		const showStore = new ShowObjectsStore();
		showStore.reset(SHOW_ID, "session-a");
		const programmingStore = new ProgrammingInteractionStore();
		programmingStore.reset(SHOW_ID, DESK_ID, "session-a");
		const apply = vi.fn();
		const transport: CueTransferTransport = { apply };
		const repair = {
			loadCueLists: vi.fn(),
			loadCommandLine: vi.fn(),
		};

		render(
			<CueTransferProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				userId={USER_ID}
				authorityKey="server-a|session-a"
				showStore={showStore}
				programmingStore={programmingStore}
				transport={transport}
				repair={repair}
			>
				<Consumer />
			</CueTransferProvider>,
		);
		await Promise.resolve();

		expect(apply).not.toHaveBeenCalled();
		expect(repair.loadCueLists).not.toHaveBeenCalled();
		expect(repair.loadCommandLine).not.toHaveBeenCalled();
		expect(renders).toBe(1);
		act(() => showStore.setCollection(SHOW_ID, "preset", [], 10, 3));
		act(() => programmingStore.setReady());
		expect(renders).toBe(1);
	});
});
