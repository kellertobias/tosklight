# HTP, LTP, and ownership

Each fixture attribute has one resolved output, even when the Programmer and several playbacks supply values.

## Programmer values use LTP

Within the Programmer, the latest applicable edit wins, including Intensity. If a Group is at 80% and you then set one of its fixtures to 30%, that fixture uses 30%. An edit from another surface of the same desk follows the same rule.

Clear the Programmer before checking a Cue on its own. Its values can otherwise mask the playback result.

## Playback HTP and LTP

**Highest Takes Precedence (HTP)** compares intensity contributions. The highest eligible value wins within the configured priorities. Bringing up another HTP intensity playback cannot reduce a higher contribution. Releasing the winner reveals the next contribution.

**Latest Takes Precedence (LTP)** uses the most recent eligible owner within the priority rules. Color, Position, and other non-intensity attributes normally use LTP. Cuelist Settings also offers an intensity HTP/LTP choice.

Temporary Flash and Temp controls restore the previous LTP owner when released, according to their configured release behavior. Masters and Blackout further affect output.

## Find the owner

Inspect the [Fixture Sheet](../30-Windows/05-fixture-sheet.md) or [Channels](../30-Windows/07-channel-faders.md) to see the ordinary base and source. Use [Running](../30-Windows/12-running.md) for active runtime objects and [DMX](../30-Windows/04-utility-and-diagnostics.md#dmx-output) for the final channel values.

For an unexpected look, check the current Cue and its tracked source, then the Programmer, temporary actions, masters, and DMX. [Cuelists and tracking](16-cuelists-and-tracking.md) explains values inherited from earlier Cues.
