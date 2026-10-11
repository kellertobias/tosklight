import { describe, expect, it } from "vitest";
import { windowChoices } from "../components/modals/WindowPicker";
import { decodeShowObjectBody } from "../api/showObjectBodyWire";
import type { BuiltInWindow } from "../types";
import { VirtualPlaybacksWindow } from "./VirtualPlaybacksWindow";
import { isRegisteredWindow, windowRegistry } from "./WindowRegistry";

describe("canonical Playback pane startup", () => {
    it("registers the canonical demo/default kind using the real Playback component", () => {
        expect((windowRegistry as Record<string, unknown>).playbacks).toBe(VirtualPlaybacksWindow);
        expect(windowChoices.some(([kind]) => String(kind) === "playbacks")).toBe(true);
    });
    it("rejects unknown and retired pane kinds instead of returning an undefined component", () => {
        expect(isRegisteredWindow("unknown" as BuiltInWindow)).toBe(false);
        expect(isRegisteredWindow("virtual_playbacks" as BuiltInWindow)).toBe(false);
        expect(isRegisteredWindow("layout")).toBe(false);
    });
    it("decodes the same canonical pane kind and surface used by portable demo layouts", () => {
        const layout = decodeShowObjectBody("user_layout", {
            desks: [{ id: "busking", name: "Busking", panes: [{
                id: "busking-playbacks", kind: "playbacks", title: "Virtual Playbacks",
                surface_id: "pane:busking-playbacks", x: 0, y: 0, width: 12, height: 10,
            }] }], activeDeskId: "busking", windowSettings: {},
        }, "canonical demo");
        expect(layout.desks[0].panes[0].kind).toBe("playbacks");
        expect(layout.desks[0].panes[0].surface_id).toBe("pane:busking-playbacks");
    });
});
