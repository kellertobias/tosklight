import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { defaultUpdateSettings } from "../../components/control/updateWorkflow";
import { programmingUpdateSettingsView } from "../programmingUpdate/settingsView";
import { usePoolRecordLabel } from "./usePoolRecordLabel";

const mocks = vi.hoisted(() => ({text: "RECORD", update: null as null | {loadSettings: ReturnType<typeof vi.fn>}}));
vi.mock("../programmingUpdate/ProgrammingUpdateProvider", () => ({useProgrammingUpdate: () => mocks.update}));
vi.mock("../../components/control/commandLine/useCommandLineSurface", () => ({useCommandLineSurface: () => ({text: mocks.text})}));
afterEach(cleanup);
function Grid({active = true}: {active?: boolean}) {
 const label = usePoolRecordLabel({active});
 return <span>{label({kind: "preset", exists: true})}</span>;
}

describe("record labels follow the scoped desk settings", () => {
 it("shares reads, refreshes saved defaults, and honours a one-off option", async () => {
  mocks.text = "RECORD";
  mocks.update = {loadSettings: vi.fn(async () => ({...defaultUpdateSettings,record_default:"merge"}))};
  const {rerender} = render(<><Grid/><Grid/></>);
  await waitFor(() => expect(screen.getAllByText("Merge")).toHaveLength(2));
  expect(mocks.update.loadSettings).toHaveBeenCalledOnce();
  act(() => programmingUpdateSettingsView(mocks.update!).install({...defaultUpdateSettings,record_default:"smart"}));
  expect(screen.getAllByText("REC")).toHaveLength(2);
  mocks.text = "RECORD MERGE";
  rerender(<><Grid/><Grid/></>);
  expect(screen.getAllByText("Merge")).toHaveLength(2);
 });
 it("does not load settings for an inactive grid", async () => {
  mocks.update = {loadSettings: vi.fn(async () => defaultUpdateSettings)};
  mocks.text = "RECORD";
  render(<Grid active={false}/>);
  await act(async () => {});
  expect(mocks.update.loadSettings).not.toHaveBeenCalled();
 });
});
