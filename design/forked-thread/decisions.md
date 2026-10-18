# Forked thread look: design decisions

Picked on the design board (`design/forked-thread/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. The fork when it opens, before its first message: C. Context chip over the composer

*Fork*

**Today:** Fork from This Response makes the thread and opens it as a long conversation: the source’s turns up to that reply, in full, ending at a “Forked from conversation” pill divider, with the composer as a bar along the bottom like any thread. The header has a pill with the source’s title. Nothing says what a fork is or that the first message will carry all of it to the agent.

**C. Context chip over the composer** (from Merge-back chip (new here)): The new thread screen with the carried conversation as a chip inside the composer card, like the merge-back chip: “Add the checkout page · 6 messages” with an ×. Clicking the chip previews exactly what goes to the agent; × sends the first message without it (then the thread is a plain new thread, with its source pill still in the header).

## 2. Once the fork has its own messages: None of these

*Fork*

**Today:** The carried conversation stays in the thread in full, like any other messages, and its “Forked from conversation” pill divider sits where the fork’s own messages start. The thread scrolls from the source’s first message on, so the fork’s own work starts far down.

**Note:** current is fine

## 3. Open Source and Merge Back in this look: None of these

*Fork*

**Today:** Both live in the menu of the header pill that holds the source’s title: Open Source Thread, and Merge Back, which is off while the fork is working. The pill is there before the first message and after.

**Note:** current is fine
