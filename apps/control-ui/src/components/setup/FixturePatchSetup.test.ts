import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../../api/types";
import { compareFixtureIds } from "./FixturePatchSetup";

const fixture = (fixture_number: number | null, fixture_id: string) => ({ fixture_number, fixture_id }) as PatchedFixture;

describe("Show Patch fixture ordering", () => {
  it("sorts numbered fixtures by fixture ID and leaves unnumbered fixtures last", () => {
    const fixtures = [fixture(999, "rgb"), fixture(null, "z"), fixture(2, "two"), fixture(101, "one-oh-one"), fixture(null, "a")];
    expect(fixtures.sort(compareFixtureIds).map((item) => item.fixture_id)).toEqual(["two", "one-oh-one", "rgb", "a", "z"]);
  });
});
