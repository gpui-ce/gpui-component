//! Compatibility for the component library's row-based editor model.
//! Wrapping algorithm adapted from GPUI CE `afc7a95c8971c3d829a2fe8a2646ed3c436ca2b4`,
//! `crates/gpui/src/text_system/line_wrapper.rs` (Apache-2.0).
//! Wrapping retains the pre-0.3 GPUI algorithm; shaping and hit testing use GPUI 0.3.
use gpui::{CaretPosition, Font, Pixels, ShapedLine, TextSystem, WindowTextSystem, point, px};
use std::{collections::HashMap, iter, ops::Deref, sync::Arc};

pub struct LineWrapper {
    text_system: WindowTextSystem,
    font: Font,
    font_size: Pixels,
    letter_spacing: Option<Pixels>,
    cached_ascii_char_widths: [Option<Pixels>; 128],
    cached_other_char_widths: HashMap<char, Pixels>,
}
impl LineWrapper {
    pub const MAX_INDENT: u32 = 256;
    fn new(font: Font, font_size: Pixels, text_system: Arc<TextSystem>) -> Self {
        Self {
            font,
            text_system: WindowTextSystem::new(text_system),
            font_size,
            letter_spacing: None,
            cached_ascii_char_widths: [None; 128],
            cached_other_char_widths: HashMap::new(),
        }
    }
    /// Wrap a line of text to the given width with this wrapper's font and font size.
    pub fn wrap_line<'a>(
        &'a mut self,
        fragments: &'a [LineFragment],
        wrap_width: Pixels,
    ) -> impl Iterator<Item = Boundary> + 'a {
        let mut width = px(0.);
        let mut first_non_whitespace_ix = None;
        let mut indent = None;
        let mut last_candidate_ix = 0;
        let mut last_candidate_width = px(0.);
        let mut last_wrap_ix = 0;
        let mut prev_c = '\0';
        let mut index = 0;
        let mut previous_text_character = false;
        let mut candidates = fragments
            .iter()
            .flat_map(move |fragment| fragment.wrap_boundary_candidates())
            .peekable();
        iter::from_fn(move || {
            for candidate in candidates.by_ref() {
                let ix = index;
                index += candidate.len_utf8();
                let mut new_prev_c = prev_c;
                let mut item_had_spacing = false;
                let item_width = match candidate {
                    WrapBoundaryCandidate::Char { character: c } => {
                        if c == '\n' {
                            previous_text_character = false;
                            continue;
                        }

                        if Self::is_word_char(c) {
                            if prev_c == ' ' && c != ' ' && first_non_whitespace_ix.is_some() {
                                last_candidate_ix = ix;
                                last_candidate_width = width;
                            }
                        } else {
                            // CJK may not be space separated, e.g.: `Hello world你好世界`
                            if c != ' ' && first_non_whitespace_ix.is_some() {
                                last_candidate_ix = ix;
                                last_candidate_width = width;
                            }
                        }

                        if c != ' ' && first_non_whitespace_ix.is_none() {
                            first_non_whitespace_ix = Some(ix);
                        }

                        new_prev_c = c;

                        let width = self.width_for_char(c);
                        let spacing = if previous_text_character {
                            self.letter_spacing.unwrap_or_default()
                        } else {
                            px(0.)
                        };
                        item_had_spacing = previous_text_character;
                        previous_text_character = true;
                        width + spacing
                    }
                    WrapBoundaryCandidate::Element {
                        width: element_width,
                        ..
                    } => {
                        if prev_c == ' ' && first_non_whitespace_ix.is_some() {
                            last_candidate_ix = ix;
                            last_candidate_width = width;
                        }

                        if first_non_whitespace_ix.is_none() {
                            first_non_whitespace_ix = Some(ix);
                        }

                        element_width
                    }
                };

                width += item_width;
                if width > wrap_width && ix > last_wrap_ix {
                    if let (None, Some(first_non_whitespace_ix)) = (indent, first_non_whitespace_ix)
                    {
                        indent = Some(
                            Self::MAX_INDENT.min((first_non_whitespace_ix - last_wrap_ix) as u32),
                        );
                    }

                    if last_candidate_ix > 0 {
                        last_wrap_ix = last_candidate_ix;
                        width -= last_candidate_width;
                        width -= self.letter_spacing.unwrap_or_default();
                        last_candidate_ix = 0;
                    } else {
                        last_wrap_ix = ix;
                        width = if item_had_spacing {
                            item_width - self.letter_spacing.unwrap_or_default()
                        } else {
                            item_width
                        };
                    }

                    if let Some(indent) = indent {
                        width += self.width_for_char(' ') * indent as f32;
                    }

                    return Some(Boundary::new(last_wrap_ix, indent.unwrap_or(0)));
                }

                prev_c = new_prev_c;
            }

            None
        })
    }

    /// Any character in this list should be treated as a word character,
    /// meaning it can be part of a word that should not be wrapped.
    pub(crate) fn is_word_char(c: char) -> bool {
        // ASCII alphanumeric characters, for English, numbers: `Hello123`, etc.
        c.is_ascii_alphanumeric() ||
        // Latin script in Unicode for French, German, Spanish, etc.
        // Latin-1 Supplement
        // https://en.wikipedia.org/wiki/Latin-1_Supplement
        matches!(c, '\u{00C0}'..='\u{00FF}') ||
        // Latin Extended-A
        // https://en.wikipedia.org/wiki/Latin_Extended-A
        matches!(c, '\u{0100}'..='\u{017F}') ||
        // Latin Extended-B
        // https://en.wikipedia.org/wiki/Latin_Extended-B
        matches!(c, '\u{0180}'..='\u{024F}') ||
        // Cyrillic for Russian, Ukrainian, etc.
        // https://en.wikipedia.org/wiki/Cyrillic_script_in_Unicode
        matches!(c, '\u{0400}'..='\u{04FF}') ||

        // Vietnamese (https://vietunicode.sourceforge.net/charset/)
        matches!(c, '\u{1E00}'..='\u{1EFF}') || // Latin Extended Additional
        matches!(c, '\u{0300}'..='\u{036F}') || // Combining Diacritical Marks

        // Bengali (https://en.wikipedia.org/wiki/Bengali_(Unicode_block))
        matches!(c, '\u{0980}'..='\u{09FF}') ||

        // Some other known special characters that should be treated as word characters,
        // e.g. `a-b`, `var_name`, `I'm`/`won’t`, '@mention`, `#hashtag`, `100%`, `3.1415`,
        // `2^3`, `a~b`, `a=1`, `Self::new`, etc. Trailing punctuation like `,`, `.`, `:`, `;`
        // is included so it stays attached to the preceding word when wrapping.
        matches!(c, '-' | '_' | '.' | '\'' | '’' | '‘' | '$' | '%' | '@' | '#' | '^' | '~' | ',' | '=' | ':' | ';') ||
        // Closing punctuation never starts a line (UAX #14 LB13: no break
        // before `!`, `)`, `]`, `}`, closing quotes or an ellipsis) — `plz!`,
        // `see)`, `quoted”` wrap as one word instead of orphaning the mark on
        // the next line. `/` and `?` stay break opportunities so long paths
        // and URLs (`a/b`, `foo?b=2`) can wrap.
        matches!(c, '!' | ')' | ']' | '}' | '"' | '”' | '»' | '…') ||
        // `⋯` character is special used in Zed, to keep this at the end of the line.
        matches!(c, '⋯') ||

        // Non-breaking glue characters
        matches!(c, '\u{202F}' | '\u{00A0}' | '\u{2011}')
    }

    #[inline(always)]
    fn width_for_char(&mut self, c: char) -> Pixels {
        if (c as u32) < 128 {
            if let Some(cached_width) = self.cached_ascii_char_widths[c as usize] {
                cached_width
            } else {
                let width = self.measure_char(c);
                self.cached_ascii_char_widths[c as usize] = Some(width);
                width
            }
        } else if let Some(cached_width) = self.cached_other_char_widths.get(&c) {
            *cached_width
        } else {
            let width = self.measure_char(c);
            self.cached_other_char_widths.insert(c, width);
            width
        }
    }

    fn measure_char(&self, c: char) -> Pixels {
        // Shape through the backend so fallback glyphs have their painted width,
        // including characters absent from the requested font (emoji and CJK).
        let text = c.to_string();
        self.text_system
            .shape_line(
                text.clone().into(),
                self.font_size,
                &[gpui::TextRun {
                    len: text.len(),
                    font: self.font.clone(),
                    ..Default::default()
                }],
            )
            .width
    }
}
/// A fragment of a line that can be wrapped.
pub enum LineFragment<'a> {
    /// A text fragment consisting of characters.
    Text {
        /// The text content of the fragment.
        text: &'a str,
    },
    /// A non-text element with a fixed width.
    Element {
        /// The width of the element in pixels.
        width: Pixels,
        /// The UTF-8 encoded length of the element.
        len_utf8: usize,
    },
}

impl<'a> LineFragment<'a> {
    /// Creates a new text fragment from the given text.
    pub fn text(text: &'a str) -> Self {
        LineFragment::Text { text }
    }

    /// Creates a new non-text element with the given width and UTF-8 encoded length.
    pub fn element(width: Pixels, len_utf8: usize) -> Self {
        LineFragment::Element { width, len_utf8 }
    }

    fn wrap_boundary_candidates(&self) -> impl Iterator<Item = WrapBoundaryCandidate> {
        let text = match self {
            LineFragment::Text { text } => text,
            LineFragment::Element { .. } => "\0",
        };
        text.chars().map(move |character| {
            if let LineFragment::Element { width, len_utf8 } = self {
                WrapBoundaryCandidate::Element {
                    width: *width,
                    len_utf8: *len_utf8,
                }
            } else {
                WrapBoundaryCandidate::Char { character }
            }
        })
    }
}

enum WrapBoundaryCandidate {
    Char { character: char },
    Element { width: Pixels, len_utf8: usize },
}

impl WrapBoundaryCandidate {
    pub fn len_utf8(&self) -> usize {
        match self {
            WrapBoundaryCandidate::Char { character } => character.len_utf8(),
            WrapBoundaryCandidate::Element { len_utf8: len, .. } => *len,
        }
    }
}

/// A boundary between two lines of text.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Boundary {
    /// The index of the last character in a line
    pub ix: usize,
    /// The indent of the next line.
    pub next_indent: u32,
}

impl Boundary {
    fn new(ix: usize, next_indent: u32) -> Self {
        Self { ix, next_indent }
    }
}

pub trait LineWrapperExt {
    fn line_wrapper(&self, font: Font, size: Pixels) -> LineWrapper;
}
impl LineWrapperExt for Arc<TextSystem> {
    fn line_wrapper(&self, font: Font, size: Pixels) -> LineWrapper {
        LineWrapper::new(font, size, self.clone())
    }
}
impl LineWrapperExt for WindowTextSystem {
    fn line_wrapper(&self, font: Font, size: Pixels) -> LineWrapper {
        LineWrapper::new(font, size, self.deref().clone())
    }
}

pub trait ShapedLineExt {
    fn x_for_index(&self, index: usize) -> Pixels;
    fn closest_index_for_x(&self, x: Pixels) -> usize;
    fn index_for_x(&self, x: Pixels) -> Option<usize>;
}
impl ShapedLineExt for ShapedLine {
    fn x_for_index(&self, index: usize) -> Pixels {
        self.platform_layout
            .caret_bounds(
                CaretPosition::attached_to_next_cluster(index.min(self.len())),
                self.font_size,
            )
            .map_or(self.width, |bounds| bounds.origin.x)
    }
    fn closest_index_for_x(&self, x: Pixels) -> usize {
        if self.width == px(0.) {
            return if x <= px(0.) { 0 } else { self.len() };
        }
        let height = self.font_size.max(px(1.));
        self.platform_layout
            .caret_from_pixel_point(point(x, height / 2.), height)
            .unwrap_or_else(|caret| caret)
            .index
    }
    fn index_for_x(&self, x: Pixels) -> Option<usize> {
        let height = self.font_size.max(px(1.));
        self.platform_layout
            .byte_index_from_pixel_point(point(x, height / 2.), height)
            .ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TextRun;
    use std::borrow::Cow;

    fn text_system() -> Arc<TextSystem> {
        let system = Arc::new(TextSystem::new(Arc::new(
            gpui_ce_parley::ParleyTextSystem::without_system_fonts(),
        )));
        system
            .add_fonts(vec![Cow::Borrowed(include_bytes!(
                "../../story-web/fonts/IBMPlexSans-Regular.ttf"
            ))])
            .unwrap();
        system
    }

    #[test]
    fn wrapping_preserves_unicode_and_atomic_elements() {
        let system = text_system();
        let font = Font::default();
        let id = system.resolve_font(&font);
        let cell = system.advance(id, px(16.), 'a').unwrap().width;
        let mut wrapper = system.line_wrapper(font, px(16.));
        let fragments = [
            LineFragment::text("é "),
            LineFragment::element(cell * 2., 4),
            LineFragment::text("🙂 end"),
        ];
        let boundaries = wrapper.wrap_line(&fragments, cell * 3.).collect::<Vec<_>>();
        let source = "é @tag🙂 end";
        assert!(!boundaries.is_empty());
        assert!(
            boundaries
                .iter()
                .all(|boundary| source.is_char_boundary(boundary.ix))
        );
        assert!(boundaries.windows(2).all(|pair| pair[0].ix < pair[1].ix));
        assert!(
            boundaries
                .iter()
                .all(|boundary| !("é ".len() + 1.."é @tag".len()).contains(&boundary.ix))
        );
        assert!(boundaries.iter().any(|boundary| boundary.ix == "é ".len()));
    }

    #[test]
    fn shaped_line_carets_use_backend_utf8_indices() {
        let system = WindowTextSystem::new(text_system());
        let text = "abé🙂 end";
        let line = system.shape_line(
            text.into(),
            px(16.),
            &[TextRun {
                len: text.len(),
                ..Default::default()
            }],
        );
        for index in text
            .char_indices()
            .map(|(index, _)| index)
            .chain([text.len()])
        {
            assert_eq!(line.closest_index_for_x(line.x_for_index(index)), index);
        }
        assert_eq!(line.index_for_x(px(-10.)), None);
        assert_eq!(line.closest_index_for_x(line.width + px(10.)), text.len());
    }
}
