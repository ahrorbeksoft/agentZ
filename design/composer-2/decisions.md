# Composer, Image Markup, Pictures and Pickers: design decisions

Picked on the design board (`design/composer-2/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. The composer’s shape in a thread: B. A card, with a strip under it

*Composer*

**Today:** Once a thread has its first message, the composer is a bar across the bottom of the thread, as in Zed: a border along its top, the editor’s darker background, and its contents (up to 850 px wide) in the middle. The new thread screen has the same composer as a card: rounded, outlined, with a shadow. The activity bar (the plan’s current step, the Agents list) sits on the bar’s top edge. The footer is the next topic.

**B. A card, with a strip under it** (from t3code context strip, Codex): The card holds what you send: the text, + and Send, with the mode, model and effort. Who answers moves to a strip under the card, in small muted type: the agent, the account and its usage at the left, the context ring at the right, as the new thread’s strip shows the checkout and machine.

**Comment on B:** Put the machine and branch in the strip too. The branch moves there from the thread’s header (strip only). The machine always shows; in a sent thread it’s only a label (its icon and name), since a thread can’t move to another machine. The new thread screen gets the same strip: its agent picker moves out of the card into the strip, beside the account, the checkout, the machine and the branch.

## 2. The footer’s controls: A. Model and effort in one menu

*Composer*

**Today:** At the left: + (Add Context), the agent (“Claude Agent ⌄”), the account (an avatar and “Personal ⌄”, with two or more accounts) and the usage gauge (“69%”). At the right: the context ring, then the agent’s own settings, each its own menu (mode “Default ⌄”, model “Opus 5.5 ⌄”, effort “High ⌄”), and Send. Narrow, the right group wraps to a second line and then within it. (The screenshot is of the mock agent, whose settings are Default, Sonnet, Medium and a Fast switch.) The mocks use the card of A in the topic before.

**A. Model and effort in one menu** (from Codex /model (a model, then its effort)): The model and effort show as one chip, “Opus 5.5 · High ⌄”. Its menu lists the models, with the effort as a row of segments under them. An agent without effort keeps the model alone. Mode stays its own menu.

**Comment on A:** we need search for models

## 3. Send, Queue and Stop: A. A round button, filled when ready

*Composer*

**Today:** A small square button in the corner, the same gray whether or not there’s anything to send; only its icon turns blue when there is. While the agent works it’s a red Stop with nothing typed, and Queue (a list icon) with something typed. The mocks use the card of A in the first topic.

**A. A round button, filled when ready** (from t3code ComposerPrimaryActions): A round button with an up arrow: gray while there’s nothing to send, filled with the accent color once there is. Stop is a red round button with a white square; Queue keeps the round shape with the list icon.

## 4. Where markup starts: A. A Markup button in the viewer

*Image markup*

**Today:** Clicking an Image chip in the composer opens the image viewer, the same one every image in a thread opens: a dark backdrop over the window, the image fitted in the middle, Close above its corner and its name under it (“Image”). A click zooms to 200%, scrolling zooms up to 8×, dragging pans, Escape closes. Nothing can be drawn on it. In every option, markup is only for images in the composer, not yet sent; images in the conversation stay view-only.

**A. A Markup button in the viewer** (from macOS Quick Look Markup): The viewer opened from the composer has a Markup button beside Close. It brings up the toolbar over the image (the tools are the next topic); Done keeps the marks, Cancel drops them. Without it pressed, the viewer zooms and pans as today.

## 5. The markup tools: A. Pen + B. Rectangle + C. Arrow + D. Text + F. Numbered notes + G. Colors + H. Undo, redo and erase

*Image markup · pick any*

**Today:** There are none. Pick any: each is a button in the markup toolbar. A mark can be moved or deleted until Done, in every option.

**A. Pen** (from t3code Draw, macOS Markup Sketch): Draws freehand where you drag, in the picked color, 3 px wide.

**B. Rectangle** (from t3code Region, macOS Markup Shapes): Drag to draw a box around a part of the image.

**C. Arrow** (from macOS Markup Shapes): Drag from the tail to the point to draw an arrow.

**D. Text** (from macOS Markup Text): Click to type a short label on the image, in the picked color.

**F. Numbered notes** (from t3code annotation comment): Click to drop a numbered pin and type a note for it. The pin goes on the picture; the notes go along as text with the message (“1: Make this full width”), as t3code sends an annotation’s comment with its screenshot.

**G. Colors** (from macOS Markup colors): Five colors to draw in: red (the default), yellow, blue, green and white.

**H. Undo, redo and erase** (from t3code Erase, macOS Markup): Undo and Redo (Cmd-Z, Shift-Cmd-Z), and an eraser that removes the mark you click.

## 6. What Done does: A. It replaces the image, and can be undone

*Image markup*

**Today:** Nothing changes images: what you paste is what the agent gets. Hovering an Image chip shows the picture (up to 320×240); a sent message shows its images as small thumbnails.

**A. It replaces the image, and can be undone** (from macOS Quick Look Markup (Revert)): The chip now sends the marked-up picture, and shows a pencil. Opening it again shows the marks still editable, with Revert to Original. The marks are drawn into the picture only when the message is sent.

## 7. What the warning does: None of these

*Pictures and live pages*

**Today:** agentz_design_publish takes each variant as a picture, a live page, or both. Settings › General › Artifacts has design defaults, which each project can replace with its own (“Use General’s” off): Variants, Include today’s, Sizes and Variants as (Pictures, Live pages or Both; Pictures unless you change it). A design that differs from them (pictures only while Variants as says Live pages, say) is sent back, unless the agent gives a reason (deviationReason), which you then see on the design. Nothing else tells an agent that it gave pictures. In every option the warning is on or off by the setting in the next topic.

**Note:** idk, i don't want it anymore, let's drop this feature implementation

## 8. The setting: None of these

*Pictures and live pages*

**Today:** There’s no setting for a warning. The design defaults live in Settings › General › Artifacts; a project’s settings have “Design defaults” with a Use General’s switch, and turning it off gives the project its own copy of every default. Elsewhere a project overrides one General setting with a “Default (…)” menu, as Grouping does.

**Note:** idk, i don't want it anymore, let's drop this feature implementation

## 9. Whether an agent or account is in use: A. A working dot

*Agent and account pickers*

**Today:** The agent picker (“Claude Agent ⌄” in the composer, and on the new thread screen) lists the installed agents: icon, name and a check on the current one, then Manage Agents…. The account picker, shown with two or more accounts, lists “This thread”, then “Switch to”: each account’s avatar, name and plan, what’s left of its usage (“69% left”) and a check. Neither says whether an agent or account is working in another thread. In the mocks Claude Agent is working in two threads (this one, on Personal, and one on Work) and Codex in one.

**A. A working dot** (from agentZ sidebar status dot): The sidebar’s blue working dot after the name of an agent or account with a thread working right now. Hovering the row says where: “Working in 2 threads”.

## 10. Every agent’s usage in the agent picker: A. What’s left, at the row’s end

*Agent and account pickers*

**Today:** An agent’s usage shows in its account picker (“69% left” on each account) and in the composer’s gauge, for the thread’s own account. An agent with one account (Codex and Factory Droid here) has no account picker, so its usage shows nowhere until a thread of it is open. Google Antigravity’s usage isn’t read in the mocks. The mocks use the working dot of A in the topic before.

**A. What’s left, at the row’s end** (from agentZ account picker): Each agent ends with what’s left of the account a switch would use, in the account picker’s words (“41% left”): yellow at 15% or less, red when it’s used up. For Claude Agent that’s this thread’s account; for another agent, its default. Nothing when its usage isn’t read.
