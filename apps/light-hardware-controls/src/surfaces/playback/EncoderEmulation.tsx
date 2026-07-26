import {
  controlSurfaceOscPaths,
  type EncoderControlAction,
} from "@tosklight/ui/control-surface-contracts";
import { useState } from "react";
import type { SendControl } from "../../controller/types";

interface EncoderEmulationProps {
  number: number;
  nav?: boolean;
  send: SendControl;
}

export function EncoderEmulation({
  number,
  nav = false,
  send,
}: EncoderEmulationProps) {
  const [held, setHeld] = useState(false);
  const path = nav
    ? controlSurfaceOscPaths.navigation
    : controlSurfaceOscPaths.encoder(number);
  const name = nav ? "Navigation" : `Encoder ${number}`;

  return (
    <section className={`encoder-emulation ${held ? "held" : ""}`}>
      <button
        type="button"
        aria-label={`${name} ${held ? "left" : "up"}`}
        onClick={() =>
          send(path, [(held ? "left" : "up") satisfies EncoderControlAction])
        }
      >
        {held ? "‹" : "⌃"}
      </button>
      <div>
        <button
          type="button"
          aria-label={`${name} click`}
          onClick={() => send(path, ["press" satisfies EncoderControlAction])}
        >
          CLK
        </button>
        <button
          type="button"
          aria-label={`${name} hold`}
          className={held ? "active" : ""}
          onClick={() => setHeld((value) => !value)}
        >
          HLD
        </button>
      </div>
      <button
        type="button"
        aria-label={`${name} ${held ? "right" : "down"}`}
        onClick={() =>
          send(path, [(held ? "right" : "down") satisfies EncoderControlAction])
        }
      >
        {held ? "›" : "⌄"}
      </button>
      <small>{nav ? "NAV" : number}</small>
    </section>
  );
}
