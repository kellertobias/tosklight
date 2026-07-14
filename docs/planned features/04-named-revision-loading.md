# Named Revision Loading

Revisit what happens when an operator loads a named revision, especially how subsequent autosaves relate to that revision and to the show file's newer history.

Loading an older named revision effectively branches the show from that saved point. Planning must define whether future autosaves continue on a distinct branch, replace the current latest-autosave line, or require another explicit model. The operator must be able to understand which revision the current show branched from, retain the original named revision unchanged, and avoid unintentionally losing or obscuring autosaves that were created after that revision.

Before implementation, define how these branches are represented, named, selected, restored, and eventually merged or discarded, as well as what the Show menu communicates immediately after loading a named revision.
