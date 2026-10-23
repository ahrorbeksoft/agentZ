# Backlog

Features and bugs to implement, in the order they were given. Keep the user's request and any
findings with each entry. Remove an entry once it's built.

## Bugs

### Linux: Ctrl-, doesn't open Settings while a terminal has focus

On Linux, Ctrl-, does nothing while a terminal has focus; it should open Settings as it does
elsewhere.

Not reproduced yet: headless tests (`shell::tests::cmd_comma_opens_settings_while_…`) show the
binding (`secondary-,` in the `Shell` context) reaching `OpenSettings` with a terminal focused,
so the cause is likely in what a real Linux keyboard sends (xkb's key and `key_char` for the
comma while Ctrl is down). Needs a run under a real X or Wayland session.

Skipped for now (October 18, 2026). Not reproduced under X11 either: on Xvfb with no input
method, `xdotool key ctrl+comma` opened Settings with focus in a Workspaces pane, a thread's
terminal drawer and a terminal thread. Left to try: Wayland, an input method (IBus or Fcitx,
which get every key on X11 through XIM), a desktop shortcut on Ctrl-, and non-US layouts.

### Factory Droid's turn ends while its background work runs, again

Reported October 23, 2026 as "this bug again", with the screenshot below. A Factory Droid
thread's turn shows as ended (1h 32m 38s) right after Droid said "The 0.4.1 release workflow is
running … the watcher will wake me when the workflow finishes", with its plan still at
"Current: Bump to 0.4.1, push the tag and watch the release workflow" (2 left).

![A Factory Droid turn shown as ended while its release watcher runs](backlog/droid-turn-ends-during-background-work.png)

Which earlier bug wasn't said. The closest are "Turns split without user messages" and "A
message sent while Factory Droid wakes by itself is never answered", removed as built by
a7a4262 ("Keep Droid's turns whole while its background work runs", in v0.4.0 and v0.4.1).
The account's email in the composer is covered in the screenshot.

## Features

### Artifacts move with their thread to another machine

From the Artifacts 2 round (`design/artifacts-2/`, decided in its intro and topic 7): an
artifact moves with its thread when the thread goes on on another machine. Nothing moves a
thread to another machine yet (Continue with Another Agent, workspace handoff and the limit
notice stay on its machine), so this waits for that. Until then, a thread can't publish a new
version of another machine's artifact ("there's no artifact … to update").
