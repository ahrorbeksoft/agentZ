# Logging in: design decisions

Picked on the design board (`design/login/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Where the login shows: A. A card where the reply would be + also C. The Add Account dialog, opened from the thread

*In a thread*

**Today:** A panel takes the middle of an empty thread. After a message, it follows the conversation: your message at the top, the panel under it, and “The message wasn’t sent” with Retry above the composer. The composer says “Log in to Factory Droid to send a message” and can’t send. Topic 5 decides the unsent message; these mocks leave it out, and use topic 2’s rows for the methods where today’s buttons don’t apply.

**A. A card where the reply would be** (from new, in the look of the Add Account dialog): The login is a card in the conversation, 440 px wide: the agent’s icon and “Log in to Factory Droid” on one line, then the methods. In an empty thread it sits in the middle, as today. After a message, it follows your message on the agent’s side, where its reply would be. Once logged in, the card goes.

**Also take from C. The Add Account dialog, opened from the thread** (from t3code (its banner sends you to setup), with the Add Account dialog): The thread shows only a line over the composer: “Factory Droid needs a login” with Log In…. That opens the Add Account dialog, titled “Log in to Factory Droid”: the methods, the picked one’s step, then “Logged in as alex@hey.com” with Done. It doesn’t open by itself.

**Note:** for a new thread in the center, and for after a first message inside a window

## 2. The login methods: A. Rows, as in the Add Account dialog

*In a thread*

**Today:** A full-width button per method, 300 px wide and 32 tall, with its icon and name: the agent’s first method filled, the rest outlined. Its description is a tooltip. The Add Account dialog lists the same methods as rows instead: an icon in a tile, the name, the description under it and a chevron. These mocks put the methods in topic 1 A’s card.

**A. Rows, as in the Add Account dialog** (from the Add Account dialog (Usage round, topic 6 A)): Each method is a row: its icon in a small tile, its name, the agent’s description under it, and a chevron. A row lights up under the mouse; a click starts it. None is filled.

## 3. What it says: B. The agent’s words in place of agentZ’s line

*In a thread*

**Today:** Under the icon: “Log in to Factory Droid”, then “Every thread with Factory Droid shares the login.” in muted text. Under that, what the agent said when it asked for the login, as markdown, as Zed shows it; Droid says “Click the “Login” button to authenticate, or set a FACTORY_API_KEY environment variable.” Its lines are left-aligned under a centered head. With several accounts, it doesn’t say which account logs in. The mocks use topic 1 A’s card and topic 2 A’s rows.

**B. The agent’s words in place of agentZ’s line** (from Zed): Zed’s: under the title, the agent’s message as the description. Only when the agent says nothing, agentZ’s line (“Every thread with Factory Droid shares the login.”, or the account with several).

## 4. Each method’s step: A. Each step in the card, with Back

*In a thread*

**Today:** Picking a method shows its step in the panel. A pairing code replaces the head and buttons: “Enter this code at app.factory.ai”, the code in boxes, Copy Code and Open app.factory.ai, then “Waiting for you to finish · Cancel”. A terminal login opens a 240 px terminal under the buttons, which stay, with “Finish in the terminal. Claude Agent starts again logged in once it’s done.” An API key opens a card under the head with the field, “Factory Droid keeps the key, agentZ doesn’t store it.”, Cancel and Log In. The Add Account dialog shows the same steps in its body, with Back and Cancel at its foot. Pairing code: now-login-browser.png; terminal: now-login-terminal.png; API key: now-login-key.png; the dialog: now-add-account-browser.png.

**A. Each step in the card, with Back** (from the Add Account dialog): The step replaces the methods in the same card, under the same head, as the dialog does: the code with Copy Code and Open; the method’s name over its terminal at the card’s width; the key field. Each ends in Back (to the methods) and Cancel, or Log In for a key.

## 5. The message that wasn’t sent: B. Under your message

*In a thread*

**Today:** Built from your report: a message the agent asks a login for fails. Your message stays at the top, the login shows under it, and over the composer a red callout says “The message wasn’t sent — Factory Droid asked for a login before taking it. Once you’ve logged in, retry.” with Retry, which works once the agent is ready. After the login, the callout stays until Retry or a new message. A message that fails for another reason (a lost connection) has Retry in its error callout. The mocks use topic 1 A’s card.

**B. Under your message** (from chat apps (Messages’ “Not Delivered”)): Your message gets a red “Not sent · Retry” under its bubble, as a chat app marks a message that didn’t go. The login card follows. After the login only the mark stays. A message that fails for another reason gets the same mark, in place of its callout’s Retry.

## 6. The same login in Add Account and Settings: A. One look in all three

*Everywhere*

**Today:** The login shows in three places, from one piece of code laid out three ways. In a thread, today’s panel. In the Add Account dialog, the methods as rows with chevrons, then the picked one’s step with Back and Cancel. On the Account tab, a logged-out account’s card has a row per method with its own Log In (the first filled), and a step opens in place of its row. The dialog: now-add-account.png.

**A. One look in all three** (from the Add Account dialog): Topic 2’s methods and topic 4’s steps are the same in the thread, the dialog and the account’s card; only the frame around them differs. In the card, a step replaces the rows, with Back.
