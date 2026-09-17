# Quiet No-Op Feedback

## Purpose

Prove that a desk action which changes nothing while the desk stays healthy is explained by a
brief, non-blocking notice, and that real refusals and failures keep the red
**Desk needs attention** treatment.

## NOTICE-001 — Align with nothing selected

Given an open show with no fixtures selected, pressing **Align Off** beside the encoders leaves
Align **Off**, changes no Programmer value, and opens no red **Desk needs attention** message and
no **Dismiss** button. A brief notice at the top of the desk says that no fixtures are selected and
nothing changed. It is announced politely to assistive technology, never takes focus, leaves the
encoders and command line usable, and disappears on its own after a few seconds or earlier with
**OK**. The attached-hardware and keyboard Align gestures reach the same server-owned action and
show the same notice. With fixtures selected, the same press activates **Align Left** without a
notice. A genuine failure of the Align action, such as a lost desk connection, still opens the red
**Desk needs attention** message.
