import { useState } from "react";
import type { SendControl } from "../../controller/types";
import { oscPaths } from "../../oscPaths";

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
  const path = nav ? oscPaths.navigation : oscPaths.encoder(number);
  const name = nav ? "Navigation" : `Encoder ${number}`;

  return (
    <section className={`encoder-emulation ${held ? "held" : ""}`}>
      <button
        aria-label={`${name} ${held ? "left" : "up"}`}
        onClick={() => send(path, [held ? "left" : "up"])}
      >
        {held ? "‹" : "⌃"}
      </button>
      <div>
        <button onClick={() => send(path, ["press"])}>CLK</button>
        <button
          className={held ? "active" : ""}
          onClick={() => setHeld((value) => !value)}
        >
          HLD
        </button>
      </div>
      <button
        aria-label={`${name} ${held ? "right" : "down"}`}
        onClick={() => send(path, [held ? "right" : "down"])}
      >
        {held ? "›" : "⌄"}
      </button>
      <small>{nav ? "NAV" : number}</small>
    </section>
  );
}
