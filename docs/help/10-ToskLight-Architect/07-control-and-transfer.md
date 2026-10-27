# Work with Control

Publish a planned rig to Control when it is ready for programming. On the **Show** screen, choose **Publish to ToskLight Control**, then choose a discovered desk. Control adds it as a new show, and Architect follows that desk copy. Open the show in Control to begin synchronized editing.

Use **Save As** for an independent show with a new identity and no desk binding. Opening a portable `.show` file on another computer also carries no binding.

![Architect document synchronization and live lighting inputs](../assets/diagrams/architect-transfer.svg)

## Load from ToskLight Control

The third action under **Open** opens a modal listing the currently active shows announced by
control desks on the network. Desks without an active show are omitted. Select **Load** beside
a show to copy the desk's currently active show into Architect. There is no IP address input or
manual desk configuration. The list follows network discovery as desks arrive and disappear.

The loaded document is bound to that desk and that show on this computer, and from then on the two
stay in step on their own. A portable copy transferred to another computer, or written with
**Save As**, carries no desk binding.

## Stay in step with Control

A document loaded from ToskLight Control follows the desk's show without any Save:

* Every edit made here, moving a lamp, drawing an annotation, renaming a layer, editing the show
  information, reaches the desk as soon as it is made. One drag, one paste or one undo is one
  change on the desk, however many lamps it moves.
* Every change made on the desk to the rig, a fixture's address, a layer, a note, appears here
  without reopening the document. Cues, playbacks, programmer values, selection and Highlight are
  the desk's own and never travel.
* Edits are saved on this computer before anything is sent. Quit, lose the network or restart the
  computer, and they are still there; they reach the desk, each exactly once and in order, the next
  time it can be reached.
* Live control, the preview controls, received DMX, never becomes an edit and is never sent.

The chip under the file name in the dock says where the document stands. It shows nothing for a
document that follows no desk.

| Chip | Meaning |
|---|---|
| **Synced** (green) | The desk has every edit. Only this state says *Saved to Control*. |
| **N pending** (blue) | Saved on this computer and on its way to the desk. |
| **Offline** (grey) | The desk cannot be reached, the operator chose **Work offline**, or the desk has a different show open, then it reads **Show not active on Control**. Edits keep being saved here and are sent once the show is open on the desk again; nothing is ever written into another show. |
| **N conflicts** (amber) | Someone on the desk changed the same thing. |
| **Sync error** (red) | Something needs a look: the desk refused a change, a different desk answers at the desk's address, or a file this computer keeps for synchronization was damaged and has been rebuilt. |

When two people change *different* things, a layer's order here and its name on the desk, both
changes are kept. When both change the *same* thing, the desk's version stays in the document and
the chip turns amber. Press it: the panel lists each conflict with **Control's** version and
**yours**. **Keep Control's** drops your version; **Use mine** sends yours again, this time over
Control's. Until you choose, your version is kept and nothing is lost.

A change the desk refuses outright appears in the same panel with **Discard my change** and **Send
again**. **Dismiss** clears a sync error once you have read it. **Work offline** stops talking to
the desk while you keep working; **Reconnect** catches up.

On the desk, the show name in the dock shows **⇄ Architect** while an Architect is following the
show.

If a file this computer keeps for synchronization is damaged, the document still opens. A damaged
list of followed documents opens the document on its own, without its desk, and says why; the
damaged file is left in place. A damaged record of unconfirmed edits is set aside, and the edits are
recovered from the document itself.

## Load from Visualizer

The desk's **Load Show** menu offers the document this editor has open, in the same way and with
the same result: the desk imports it as an ordinary show and opens it. Only an editor that
actually has a document open is offered, an editor with nothing open is on the network, and says
so, but there is nothing to load from it.

## Discovery unavailable

Discovery is a convenience and never a requirement. A network with no discovery, a firewall that
blocks it, or a machine where the responder will not start costs the button and nothing else:
both applications start, run, and open files exactly as they did before. A show file opened
through **Open** or **Show from USB** is the same show file either button would have fetched.
