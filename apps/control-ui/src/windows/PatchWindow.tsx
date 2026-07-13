import { useState } from "react";
import type { WindowProps } from "./windowTypes";
import { FixturePatchSetup } from "../components/setup/FixturePatchSetup";
import { MediaServerSetup } from "../components/setup/MediaServerSetup";
import { Button } from "../components/common";
import { WindowHeader, WindowScrollArea } from "../components/window-kit";

export function PatchWindow(_: WindowProps) {
  const [tab, setTab] = useState<"fixtures" | "media">("fixtures");
  return <div className="patch-window">{tab === "fixtures" ? <FixturePatchSetup onMedia={() => setTab("media")} /> : <><WindowHeader title="Show Patch" info={{ primary: "Media Servers" }} actions={[[{ id: "fixtures", label: "Fixtures", onClick: () => setTab("fixtures") },{ id: "media", label: "Media Servers", active: true, onClick: () => undefined }]]} /><WindowScrollArea><main><MediaServerSetup /></main></WindowScrollArea></>}</div>;
}
