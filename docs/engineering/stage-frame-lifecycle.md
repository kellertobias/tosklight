# Embedded Stage frame lifecycle

The embedded renderer and Control compositor use protocol major 12. The greeting must reject an incompatible helper before either side imports a picture. A helper beside an older desktop bundle must not be substituted independently.

## Scheduling and ownership

The producer admits GPU preparation at a monotonic 60 Hz deadline. Missing a deadline skips elapsed scheduling slots; it never renders a burst to catch up. Input, provider events, simulation and GPU completion polling continue independently of render admission. An unchanged successful picture requires no new GPU submission.

Every image has a `FrameId { generation, slot, sequence }`. Shared transport has three slots, allowing a retained front, a consumer composition and the next producer render to overlap; copy transport has one outstanding producer credit. A reserved slot cannot be rendered into again until its exact credit is returned. Neither a different sequence nor a release from an old raster generation can free it.

Shared transport announces the slot's IOSurface once per generation, then announces each `FrameReady` only after the producer GPU completion callback has run. The callback retains the source backing and signals a local channel; it does not write to the pipe or call Control. Control imports the slot and retains its completed front picture. It returns that front's credit only after a replacement composition finishes sampling it. Resize generations may retire while old front/flight resources remain retained by Control.

Ready sequence admission is monotonic across all slots within a raster generation. A delayed older picture returns only its own unused credit and cannot replace a newer front or pending picture. Duplicate or reused owned slots remain protocol errors. The queue stays bounded; there is no early release of the retained front.

Copy transport sends pixels before readiness. Control defers GPU allocation/upload until a visible composition is admitted, and returns the copied picture's credit after its composition completes. Control then owns the retained texture. A hidden pending picture can be discarded and credited without uploading it.

The compositor admits at most one composition in flight. Clean, hidden, busy or paced attempts do not acquire a drawable, encode, submit or present. A submission or completion is not evidence of display scanout.

## Visibility and simulation

Effective demand combines the pane's positive finite geometry, web visibility, native window visibility and compositor occlusion. Keyboard focus does not determine demand. Collapsed panes retain their last positive allocation rather than allocating an active one-pixel image. Occluded drawables are probed without submission at bounded intervals.

Hidden or backpressured rendering does not freeze authoritative state. Fixture motion advances on a CPU clock. Physics and laser scripts receive each delivered value edge and periodic time updates even while Stage GPU work is suspended. This preserves release/reset commands and script state; hidden Stage can still consume simulation CPU. GPU scene preparation and labels remain behind render admission. Motion/simulation dirtiness is retained until a picture succeeds.

On macOS, only the generated, non-view-delegated `RawWindowMetalLayer` sublayer may be hidden or detached. The hosting view's backing layer and sibling Stage layers must remain untouched. Other backends clear a suspended pane once when its native window is drawable, then retain its front without recurring work. A hidden native window does not acquire a drawable.

## Control thread and teardown

Pipe writes use a bounded worker queue. The native UI pump has one outstanding UI task, a coalesced wake queue, and exact compositor deadlines. Timing out while waiting for the UI task does not cancel it or permit posting another. The pump does not wait for GPU completion.

Closing stops the owned helper off the UI thread. A compositor may be dropped after its owned native layer is detached and a terminal queue completion is observed, or after the transparent retirement composition actually completes on the GPU. Final queue destruction can otherwise wait implicitly; pending measurement work must also be cancelled or covered by that terminal completion. A skip, timeout, failed acquisition or completion of an older picture does not establish retirement. Unsupported hidden retirement remains resident until the window becomes drawable; a replacement attachment cannot overwrite that retained slot. Destroying the owning window permits final disposal.

## Validation boundaries

Use the packaged benchmark on the explicitly selected internal display while other displays are in use. Keep static-idle and animated-hidden conditions distinct. Retain exact bundle identity, physical pixel budget, quality/view, lane ownership and raw measurements.

Scheduling skips, helper submissions, compositor submissions, CPU elapsed time and sparse GPU render-pass samples describe different boundaries. They do not establish actual displayed 60 FPS or total GPU busy time per second. Those acceptance claims require correlated platform measurements and the complete parent validation matrix.
