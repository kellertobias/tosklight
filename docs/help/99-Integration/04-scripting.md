# Scripting

Use [Macros](../20-ToskLight-Control/20-Programmer-and-Cues/19-macros.md) to automate desk command sequences. Macro operation belongs to show programming: the same command semantics apply when commands are run by a Macro.

Fixture-package scripts serve a different purpose. A laser scan script, particle effect, or physics script supplies the bounded behavior of that fixture in the Visualizer. Its runtime and available data depend on the effect type; a package script is not a general desktop automation interface.

The [Architect scripting reference](https://github.com/kellertobias/tosklight/blob/main/docs/engineering/operator-manual-architect-scripting.md) documents scan engines, particles, physics, script editing, and the Architect MCP interface. See the [Color contract](https://github.com/kellertobias/tosklight/blob/main/docs/engineering/operator-manual-color-contract.md) for typed Color data used by integrations.

When controlling the desk from an external script, use the [REST API](01-rest-api.md) or [OSC](02-osc.md). Device access and native feedback belong to the [hardware bridge](03-hardware-bridge.md).
