# Planning a Rig, and Moving It

The ToskLight Viz Editor is a rig-planning window: the same patch sheet the desk uses, over a
show file rather than over a running desk. A rig planned there and a show running on a desk are
the same rig, and neither side should have to go looking for a file to get from one to the other.

## Open Demo Show

The editor's file bar has an **Open Demo Show** button, and it needs nothing else: no file to find,
no desk on the network, and no rig to patch first. It opens a full demonstration rig — front-of-house
profiles and PAR cans, moving washes and profiles, beams, strobes, scanners, Sunstrips, blinders, a
laser and a hazer — as an ordinary show of your own.

What opens is always a **copy**. The demo that ships with ToskLight is a template and is never
opened, never written to, and never changed by anything you do. The copy is written into this
installation's own shows folder and named after the demo it came from: **Demo Show** the first
time, **Demo Show 2** the next, and so on. The file bar's status line says which copy it is and
where it was written.

So a demo copy is yours. Patch it, repatch it, save it, rename it, delete it. Pressing **Open Demo
Show** again gives you a fresh copy of the shipped rig rather than reopening whatever you did to
the last one.

The demo is built from the fixture packages this version of ToskLight ships, so its fixtures carry
the same profile revisions, models and modes the fixture library does. It is the quickest way to
see what the Visualizer draws, and the rig the product demonstration video is shot from.

## Load from Desk

When a ToskLight desk with a show open is on the same network, the editor's file bar gains a
**Load from Desk** button naming that desk and the show it is running. Pressing it takes a copy of
that show, keeps it beside the editor's own documents, and opens it. Two desks are two buttons,
each naming its own machine; hovering one shows the address it was found at.

What arrives is a copy. Patching it here does not reach the desk, and the desk does not know the
copy exists. To send work back, use **Load from Visualizer** in the desk's **Load Show** menu.

## Load from Visualizer

The desk's **Load Show** menu offers the document this editor has open, in the same way and with
the same result: the desk imports it as an ordinary show and opens it. Only an editor that
actually has a document open is offered — an editor with nothing open is on the network, and says
so, but there is nothing to load from it.

## What the two sides publish

Each application announces itself on the local network, over the same standard service discovery
printers and audio interfaces use, saying which of the two it is and what it currently holds. It
publishes nothing else: no show content, no programming, no desk state. The name follows the
machine, so a rig with two editors is two entries an operator can tell apart.

The document served to the network is read-only, and it is the same read-only document the
Visualizer itself reads. There is no route into the editor that changes anything from outside it.

## When there is nothing to offer

Discovery is a convenience and never a requirement. A network with no discovery, a firewall that
blocks it, or a machine where the responder will not start costs the button and nothing else:
both applications start, run, and open files exactly as they did before. A show file opened
through **Open** or **Show from USB** is the same show file either button would have fetched.
