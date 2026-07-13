import { useState } from "react";
import type { WindowProps } from "./windowTypes";
import { FixturePatchSetup } from "../components/setup/FixturePatchSetup";
import { MediaServerSetup } from "../components/setup/MediaServerSetup";
import { Button } from "../components/common";

export function PatchWindow(_: WindowProps) {
  const [tab, setTab] = useState<"fixtures" | "media">("fixtures");
  return <div className="patch-window">{tab === "fixtures" ? <FixturePatchSetup onMedia={() => setTab("media")} /> : <><header className="patch-toolbar"><h1>Show Patch</h1><div className="button-group"><Button onClick={() => setTab("fixtures")}>Fixtures</Button><Button className="active">Media Servers</Button></div></header><main><MediaServerSetup /></main></>}</div>;
}
