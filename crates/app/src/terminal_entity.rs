//! The app's copy of a terminal on the server: the last frame it sent, kept whole by applying
//! the changes that follow. Its actions are requests, as the thread copy's are.

use agentz_protocol::terminal::{
    TerminalFrame, TerminalInput, TerminalKey, TerminalModes, TerminalScroll,
    TerminalSelectionUpdate,
};
use agentz_protocol::{Request, Response};
use gpui::{App, AppContext as _, ClipboardItem, Context, Entity, SharedString, Task};

use crate::server_client::ServerClient;
use crate::terminal_element::terminal_palette;

/// The grid the view lays out, as the server needs it to size the PTY.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalSize {
    pub columns: u16,
    pub screen_lines: u16,
    pub cell_width: u16,
    pub cell_height: u16,
}

pub struct Terminal {
    key: TerminalKey,
    /// `None` until the server has answered the subscription.
    frame: Option<TerminalFrame>,
    /// Frames that arrived while the first one was on its way.
    queued_frames: Option<Vec<TerminalFrame>>,
    /// Why the terminal can't be shown: it failed to start, or the server closed it.
    error: Option<SharedString>,
    /// The size last sent, so a view that lays out the same grid again sends nothing.
    size: Option<TerminalSize>,
    /// Kept to unsubscribe on drop, when there's no context to look it up.
    server: Option<agentz_client::Connection>,
    _subscribe: Task<()>,
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if let Some(server) = &self.server {
            drop(server.request(Request::UnsubscribeTerminal(self.key.clone())));
        }
    }
}

impl Terminal {
    /// The app's one copy of the terminal, shared by every view showing it. The server starts
    /// a thread's terminal or drawer if it isn't running.
    pub fn shared(key: TerminalKey, cx: &mut App) -> Entity<Self> {
        if let Some(terminal) = ServerClient::global(cx).read(cx).terminal(&key) {
            return terminal;
        }
        cx.new(|cx| {
            let mut this = Self {
                key: key.clone(),
                frame: None,
                queued_frames: None,
                error: None,
                size: None,
                server: None,
                _subscribe: Task::ready(()),
            };
            let weak = cx.weak_entity();
            ServerClient::global(cx).update(cx, |client, _| client.register_terminal(key, weak));
            this.subscribe(cx);
            this
        })
    }

    pub fn frame(&self) -> Option<&TerminalFrame> {
        self.frame.as_ref()
    }

    pub fn error(&self) -> Option<&SharedString> {
        self.error.as_ref()
    }

    pub fn modes(&self) -> TerminalModes {
        self.frame
            .as_ref()
            .map_or(TerminalModes::NONE, |frame| frame.modes)
    }

    pub fn has_exited(&self) -> bool {
        self.frame
            .as_ref()
            .is_some_and(|frame| frame.exited.is_some())
    }

    fn subscribe(&mut self, cx: &mut Context<Self>) {
        let client = ServerClient::global(cx);
        self.server = client.read(cx).connection().cloned();
        self.queued_frames = Some(Vec::new());
        // The new connection's terminal hasn't been sized yet.
        self.size = None;
        let response = client
            .read(cx)
            .request(Request::SubscribeTerminal(self.key.clone()));
        self._subscribe = cx.spawn(async move |this, cx| {
            let response = response.await;
            this.update(cx, |this, cx| match response {
                Ok(Response::TerminalFrame(frame)) => {
                    let mut frame = frame;
                    for queued in this.queued_frames.take().unwrap_or_default() {
                        frame.apply(queued);
                    }
                    this.frame = Some(frame);
                    this.error = None;
                    let palette = terminal_palette(cx);
                    this.input(TerminalInput::Palette(palette), cx);
                    cx.notify();
                }
                Ok(response) => this.fail(format!("unexpected response: {response:?}"), cx),
                Err(error) => this.fail(format!("{error:#}"), cx),
            })
            .ok();
        });
    }

    pub(crate) fn apply_frame(&mut self, frame: TerminalFrame, cx: &mut Context<Self>) {
        match (&mut self.queued_frames, &mut self.frame) {
            (Some(queued), _) => queued.push(frame),
            (None, Some(current)) => {
                current.apply(frame);
                cx.notify();
            }
            (None, None) => {}
        }
    }

    pub(crate) fn reconnected(&mut self, cx: &mut Context<Self>) {
        self.subscribe(cx);
    }

    /// The server closed the terminal, as when its thread was deleted. The last screen stays.
    pub(crate) fn closed(&mut self, cx: &mut Context<Self>) {
        self.fail("The terminal was closed.".to_string(), cx);
    }

    fn fail(&mut self, error: String, cx: &mut Context<Self>) {
        self.queued_frames = None;
        self.error = Some(error.into());
        cx.notify();
    }

    pub fn input(&mut self, input: TerminalInput, cx: &mut Context<Self>) {
        ServerClient::global(cx).read(cx).send(
            Request::TerminalInput {
                terminal: self.key.clone(),
                input,
            },
            cx,
        );
    }

    pub fn write(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        if !bytes.is_empty() && !self.has_exited() {
            self.input(TerminalInput::Bytes(bytes), cx);
        }
    }

    pub fn paste(&mut self, text: String, cx: &mut Context<Self>) {
        if !self.has_exited() {
            self.input(TerminalInput::Paste(text), cx);
        }
    }

    pub fn resize(&mut self, size: TerminalSize, cx: &mut Context<Self>) {
        if self.size == Some(size) || self.frame.is_none() {
            return;
        }
        self.size = Some(size);
        self.input(
            TerminalInput::Resize {
                columns: size.columns,
                screen_lines: size.screen_lines,
                cell_width: size.cell_width,
                cell_height: size.cell_height,
            },
            cx,
        );
    }

    pub fn scroll(&mut self, scroll: TerminalScroll, cx: &mut Context<Self>) {
        self.input(TerminalInput::Scroll(scroll), cx);
    }

    pub fn select(&mut self, update: Option<TerminalSelectionUpdate>, cx: &mut Context<Self>) {
        self.input(TerminalInput::Select(update), cx);
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.input(TerminalInput::SelectAll, cx);
    }

    pub fn focus(&mut self, focused: bool, cx: &mut Context<Self>) {
        if !self.has_exited() {
            self.input(TerminalInput::Focus(focused), cx);
        }
    }

    pub fn has_selection(&self) -> bool {
        self.frame
            .as_ref()
            .is_some_and(|frame| frame.selection.is_some())
    }

    /// Copies the selection, which only the server has the text of.
    pub fn copy(&mut self, cx: &mut Context<Self>) {
        if !self.has_selection() {
            return;
        }
        let response = ServerClient::global(cx)
            .read(cx)
            .request(Request::TerminalSelectionText(self.key.clone()));
        cx.spawn(async move |_, cx| match response.await {
            Ok(Response::Message(text)) if !text.is_empty() => {
                cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string(text)));
            }
            Ok(_) => {}
            Err(error) => log::error!("failed to copy from the terminal: {error:#}"),
        })
        .detach();
    }

    /// Runs the terminal's program again, in place of the one that ran.
    pub fn restart(&mut self, cx: &mut Context<Self>) {
        self.size = None;
        ServerClient::global(cx)
            .read(cx)
            .send(Request::RestartTerminal(self.key.clone()), cx);
    }
}
