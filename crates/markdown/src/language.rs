//! The parts of Zed's `language` crate the renderer needs.
//!
//! Zed highlights fenced code blocks through its language registry, which this app does not
//! include. `Language` and `LanguageRegistry` therefore have no values: the renderer's
//! highlighting paths stay in place but never run, so code blocks render unhighlighted until a
//! real highlighter replaces this module.

use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use gpui::SharedString;
use rope::Rope;

pub use rope::OffsetUtf16;
pub use sum_tree::Bias;

#[derive(Debug)]
pub enum Language {}

impl Language {
    pub fn highlight_text_resolved(
        &self,
        _text: &Rope,
        _range: Range<usize>,
    ) -> ResolvedHighlights {
        match *self {}
    }

    pub fn default_scope(&self) -> LanguageScope {
        match *self {}
    }
}

pub enum LanguageScope {}

pub enum LanguageRegistry {}

impl LanguageRegistry {
    pub async fn language_for_name_or_extension(
        &self,
        _name: &str,
    ) -> anyhow::Result<Arc<Language>> {
        match *self {}
    }

    pub async fn language_for_name(&self, _name: &str) -> anyhow::Result<Arc<Language>> {
        match *self {}
    }

    pub async fn load_language_for_file_path(&self, _path: &Path) -> anyhow::Result<Arc<Language>> {
        match *self {}
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LanguageName(pub SharedString);

impl AsRef<str> for LanguageName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HighlightId(pub u32);

impl From<HighlightId> for usize {
    fn from(id: HighlightId) -> Self {
        id.0 as usize
    }
}

/// Stands in for the grammars a highlight was resolved against.
#[derive(Clone, Debug, Default)]
pub struct HighlightSources;

#[derive(Clone, Debug, Default)]
pub struct ResolvedHighlights {
    pub sources: HighlightSources,
    pub runs: Arc<[(Range<usize>, HighlightId)]>,
}

impl ResolvedHighlights {
    pub fn is_current(&self) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CharKind {
    Whitespace,
    Punctuation,
    Word,
}

/// Classifies characters for double-click word selection.
pub struct CharClassifier;

impl CharClassifier {
    pub fn new(_scope: Option<LanguageScope>) -> Self {
        Self
    }

    pub fn kind(&self, character: char) -> CharKind {
        if character.is_alphanumeric() || character == '_' {
            CharKind::Word
        } else if character.is_whitespace() {
            CharKind::Whitespace
        } else {
            CharKind::Punctuation
        }
    }
}
