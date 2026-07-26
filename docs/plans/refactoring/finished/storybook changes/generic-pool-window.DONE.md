# Generic Pool Window

## Status

Completed on 2026-07-26. Package, live Group/Preset/Cuelist adoption, application-owned Storybook
adoption, and the final shared integration gates pass.

## Operator contract

1. Every pool window always displays a grid of at least 200 pool boxes.
2. Empty slots remain visible as real pool boxes; a pool with no stored objects therefore still
   renders 200 empty boxes.
3. Stored objects replace their corresponding empty slots. They must not be appended in addition
   to the 200 base slots.
4. A pool may request more than 200 slots, but no configuration may reduce the rendered slot count
   below 200.
5. Preserve stable slot identity and the numbering supplied by the owning pool. Do not assume that
   every pool uses one simple sequential display number; Preset families may use qualified
   numbers.
6. Preserve the existing `ButtonGrid` responsive scaling, stable row sizing, touch targets, and
   scroll behavior.
7. Empty, filled, selected, active, disabled, store-target, update-target, and set-target states
   remain available on individual pool boxes.
8. Click and press-and-hold callbacks receive the stable slot identity, including for empty slots.

## Generic package API

Add a package-owned generic pool-window composition rather than another feature-specific mock. The
exact names may follow the package conventions, but the public contract should be equivalent to:

```ts
interface PoolSlotViewModel<SlotId extends string | number> {
  id: SlotId;
  position: number; // zero-based ordered grid position
  card: PoolCardViewModel;
}

interface PoolWindowProps<SlotId extends string | number> {
  title: ReactNode;
  slots: readonly PoolSlotViewModel<SlotId>[];
  slotCount?: number; // normalized to at least 200
  emptySlot(index: number): PoolSlotViewModel<SlotId>;
  info?: WindowInfo;
  actions?: WindowAction[][];
  settingsTabs?: WindowSettingsTab[];
  minimumCardWidth?: number;
  onSlotClick?(id: SlotId, index: number): void;
  onSlotPressHold?(id: SlotId, index: number): void;
}
```

The implementation must:

- compose the package `WindowFrame`, `WindowScrollArea`, `ButtonGrid`, and `PoolCard`;
- normalize `slotCount` with `Math.max(200, slotCount ?? 200)`;
- resolve each ordered position to the stored model with that `position`, or to
  `emptySlot(index)` when no stored model occupies it;
- render exactly the normalized number of boxes;
- remain free of server APIs, Tauri, application contexts, `WindowRegistry`, and feature
  controllers; and
- expose package-owned typed view models and callbacks only.

Do not encode Group, Preset, or Cuelist retrieval, recording, selection, numbering policy, or
command-line behavior in the generic component. Those remain in application adapters.

## Specific pool adoption

- Replace the Storybook-only `PoolWindow` helper in
  the relocated mock complete-window story with the exported generic component until the
  application-owned stories replace that mock file.
- Configure the Group Pool, Preset Pool, and Cuelist Pool stories from deterministic typed slot
  models.
- Each of those stories must render at least 200 boxes even when only a few are populated.
- Add a dedicated `Pools/Generic pool window` Storybook family showing:
  - a completely empty 200-slot pool;
  - a sparse pool with stored objects at non-contiguous slots;
  - every supported card state;
  - responsive narrow and wide layouts; and
  - a configuration with more than 200 slots.
- During application adoption, make Group, Preset, and Cuelist window adapters consume the same
  generic package component while retaining their current controllers and operator behavior.

## Verification

- Package test: an empty pool renders exactly 200 pool-box buttons.
- Package test: `slotCount` below 200 still renders 200 boxes.
- Package test: a request above 200 renders the requested count.
- Package test: sparse stored slots replace the matching empty slots without changing the total.
- Package test: click and press-and-hold callbacks return the correct stable identity for filled
  and empty slots.
- Package test: header information, actions, settings, and card-width configuration pass through
  to the underlying package primitives.
- Storybook test: the generic empty, sparse, and extended stories render deterministically without
  REST or WebSocket traffic.
- Storybook test: Group, Preset, and Cuelist pool stories each contain at least 200 `.pool-card`
  elements.
- Storybook touch-viewport check proves the pool remains scrollable and its boxes retain usable
  touch geometry.
- Focused live-app tests preserve Group ordering, Preset family numbering, Cuelist selection,
  store/update targets, and press-and-hold behavior after adoption.

## Result

- The package-owned `PoolWindow` still composes `WindowFrame`, `WindowScrollArea`, `ButtonGrid`,
  and `PoolCard`. Its slot normalization is now also exported as `PoolGrid`, allowing live
  application adapters to keep their feature-owned card content and controllers while sharing the
  same minimum-count, sparse-position, sizing, and stable-identity behavior.
- `PoolGrid` renders at least 200 positions, supports larger explicit counts, replaces sparse
  positions rather than appending them, applies the stable slot identity and ordered position to
  application-rendered cards, and retains the package `PoolCard` click/press-hold path.
- The live Group Pool now has 200 ordered slots instead of 40 and renders them through `PoolGrid`.
  Existing selection, recording, Update, SET/properties, context menu, double-click dereference,
  and 600 ms press-and-hold behavior remain application-owned and covered.
- The live Preset Pool renders its 200 family-addressed positions through `PoolGrid`. Qualified
  storage identities such as `2.5`, visible family numbering, recording/recall, store targets,
  Update targets, and Set targets remain unchanged.
- The live Cuelist Pool renders its normal 1,000-slot surface through `PoolGrid`, preserving stable
  playback-number identities, selection, recording, Set assignment, Update, and 650 ms settings
  hold behavior. Its active search-result view intentionally retains the existing filtered
  `ButtonGrid` path so query filtering and the no-results state are not padded with unrelated empty
  slots.
- The existing deterministic generic empty, sparse, all-state, narrow, wide, and 240-slot stories
  remain. Application-owned stories now render the real `GroupPoolGrid`, `PresetCardGrid`, and
  `CuelistWindow` with deterministic local fixtures/providers. The corresponding Group, Preset,
  and Cuelist substitutes and their `CompletePoolWindow` helper have been removed from
  `CompleteWindows.stories.tsx`.
- The live story interaction check covers the selected Group's stable identity and context-menu
  callback, the qualified Preset identity/store target and activation callback, and the Cuelist
  pool's 1,000-slot identity plus search filtering/restoration. Each story also passes the shared
  deterministic no-REST/no-WebSocket render check.
- The former `CompleteWindows.stories.tsx` substitute module was removed after production-owned
  stories replaced its remaining compositions.
- Storybook coverage now tracks the public `PoolGrid` export and verifies all generic/complete pool
  counts plus 88-pixel-or-larger card geometry and scrolling at the narrow touch viewport.

Verification completed:

- `npm test --workspace @tosklight/ui -- --run src/pools/PoolWindow.test.tsx src/pools/PoolCard.test.tsx`
  — 2 files and 10 tests passed.
- `npm test --workspace @tosklight/light-desktop -- --run src/windows/GroupsWindow.test.tsx src/features/groupSelection/GroupSelection.test.tsx src/windows/PresetsWindow.recording.test.tsx src/windows/CuelistWindow.test.tsx`
  — 4 files and 53 tests passed.
- `npm run typecheck --workspace @tosklight/ui` — passed.
- `npm run typecheck --workspace @tosklight/light-desktop` — passed.
- `npm run test:architecture` — passed.
- Focused Storybook Playwright checks for public export coverage, 200/240-slot story counts, and
  narrow touch geometry/scrolling — 3 passed.
- `npm run storybook:build --workspace @tosklight/ui` — passed.
- Focused Storybook Playwright check for the application-owned Group, Preset, and Cuelist slot and
  interaction contracts — 1 passed.
- Shared deterministic Storybook render checks for those three application-owned stories — 3
  passed with no REST or WebSocket requests.

The shared integration blockers were resolved. The complete UI and desktop unit suites, both
TypeScript gates, architecture check, production Storybook build, and all 209 integrated
Storybook Playwright checks now pass. Browser review covered the narrow and wide production pool
stories, including square geometry, scrolling, full outlines, and explicit status badges.
