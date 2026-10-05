//! The sounds an agent makes as it finishes its turn or starts waiting for the user: Zed's
//! `agent_done.wav`, and t3code's `notification-input.mp3` for input. Played with AppKit's
//! `NSSound`, where Zed brings in rodio.

use gpui::App;

use crate::app_settings::AppSettingsStore;
use crate::project_store::ThreadStatus;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sound {
    Finished,
    NeedsInput,
}

impl Sound {
    pub fn for_status(status: ThreadStatus) -> Self {
        match status {
            ThreadStatus::PendingApproval | ThreadStatus::AwaitingInput => Self::NeedsInput,
            ThreadStatus::Working | ThreadStatus::Completed => Self::Finished,
        }
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    fn asset_path(self) -> &'static str {
        match self {
            Self::Finished => "sounds/agent_done.wav",
            Self::NeedsInput => "sounds/agent_needs_input.mp3",
        }
    }
}

/// Plays the status's sound if the settings ask for it, as Zed's
/// `PlaySoundWhenAgentDone::should_play` decides: `is_visible` is whether the user can see the
/// agent.
pub fn play_for_status(status: ThreadStatus, is_visible: bool, cx: &mut App) {
    let sound = Sound::for_status(status);
    let settings = AppSettingsStore::global(cx).read(cx).settings();
    let when = match sound {
        Sound::Finished => settings.play_sound_when_finished,
        Sound::NeedsInput => settings.play_sound_when_input_needed,
    };
    if when.should_play(is_visible) {
        play(sound, cx);
    }
}

#[cfg_attr(not(any(test, target_os = "macos")), allow(unused_variables))]
pub fn play(sound: Sound, cx: &mut App) {
    #[cfg(test)]
    cx.default_global::<PlayedForTest>().0.push(sound);
    #[cfg(all(target_os = "macos", not(test)))]
    macos::play(sound, cx);
}

/// What tests played, in place of the speakers.
#[cfg(test)]
#[derive(Default)]
struct PlayedForTest(Vec<Sound>);

#[cfg(test)]
impl gpui::Global for PlayedForTest {}

/// The sounds played so far, which the next call no longer returns.
#[cfg(test)]
pub fn take_played_for_test(cx: &mut App) -> Vec<Sound> {
    std::mem::take(&mut cx.default_global::<PlayedForTest>().0)
}

// Tests decode the sounds but don't play them.
#[cfg(target_os = "macos")]
#[cfg_attr(test, allow(dead_code))]
mod macos {
    use std::collections::HashMap;

    use anyhow::{Context as _, Result};
    use gpui::{App, Global};
    use objc2::AnyThread as _;
    use objc2::rc::Retained;
    use objc2_app_kit::NSSound;
    use objc2_foundation::NSData;
    use util::ResultExt as _;

    use super::Sound;

    /// Each sound, decoded the first time it plays. An `NSSound` stops when it's released, so
    /// they're kept.
    #[derive(Default)]
    struct Sounds(HashMap<Sound, Retained<NSSound>>);

    impl Global for Sounds {}

    pub(super) fn play(sound: Sound, cx: &mut App) {
        let is_loaded = cx
            .try_global::<Sounds>()
            .is_some_and(|sounds| sounds.0.contains_key(&sound));
        if !is_loaded {
            let Some(loaded) = load(sound, cx).log_err() else {
                return;
            };
            cx.default_global::<Sounds>().0.insert(sound, loaded);
        }
        let Some(player) = cx.global::<Sounds>().0.get(&sound) else {
            return;
        };
        // Playing again while it plays does nothing, so it starts over instead.
        if player.isPlaying() {
            player.stop();
        }
        if !player.play() {
            log::error!("couldn't play {}", sound.asset_path());
        }
    }

    fn load(sound: Sound, cx: &App) -> Result<Retained<NSSound>> {
        let path = sound.asset_path();
        let bytes = cx
            .asset_source()
            .load(path)?
            .with_context(|| format!("no asset at {path}"))?;
        decode(&bytes).with_context(|| format!("decoding {path}"))
    }

    pub(super) fn decode(bytes: &[u8]) -> Option<Retained<NSSound>> {
        NSSound::initWithData(NSSound::alloc(), &NSData::with_bytes(bytes))
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use gpui::AssetSource as _;

    use super::{Sound, macos};

    #[test]
    fn the_bundled_sounds_decode() {
        for sound in [Sound::Finished, Sound::NeedsInput] {
            let bytes = assets::Assets
                .load(sound.asset_path())
                .ok()
                .flatten()
                .unwrap_or_else(|| panic!("{} is bundled", sound.asset_path()));
            let decoded =
                macos::decode(&bytes).unwrap_or_else(|| panic!("{} decodes", sound.asset_path()));
            assert!(decoded.duration() > 0.5, "{sound:?} is a whole sound");
        }
    }
}
