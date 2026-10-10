import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { defaultPoolPresentation } from "../../features/poolPresentation/poolPresentation";
import { GroupCard } from "./GroupCard";
import type { Group } from "./model";

afterEach(cleanup);

function Card({ count, selected = false, fixtures = ["a", "b"] }: {
	count: number; selected?: boolean; fixtures?: string[];
}) {
	const group: Group = { kind: "group", id: "1", revision: 1, updated_at: "", body: { name: "Front", fixtures } };
	return <GroupCard group={group} index={0} poolSlotId="1"
		knownFixtureIds={new Set(fixtures)} capabilities={new Map()}
		selected={selected} selectedFixtureCount={count}
		fullySelected={count > 0 && count === fixtures.length}
		partiallySelected={count > 0 && count < fixtures.length}
		storeArmed={false} updateArmed={false} setTarget={false} mutationOperation={null}
		poolPresentation={defaultPoolPresentation()} showId="show" surfaceKey="group"
		beginHold={() => undefined} cancelHold={() => undefined} consumeHold={() => false}
		openSettings={() => undefined} dereference={() => undefined} select={() => undefined} />;
}

describe("Group card fixture count", () => {
	it.each([[0, "2"], [1, "1/2"], [2, "2/2"]])("shows the actual membership count %s", (count, label) => {
		const { container } = render(<Card count={count} selected />);
		expect(container.querySelector(".pool-card-information > small")).toHaveTextContent(label);
	});
	it("shows zero for a stored empty group", () => {
		const { container } = render(<Card count={0} fixtures={[]} selected />);
		expect(container.querySelector(".pool-card-information > small")).toHaveTextContent("0");
		expect(container.querySelector(".pool-card-name")).toHaveTextContent("Front");
	});
});
