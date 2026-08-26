//! Pretty-printer for a codespan-style compiler diagnostic (naga, rustc): a
//! severity header, a `┌─ file:line:col` locator, a gutter carrying the source
//! line, carets under the offending span, `= note` footers. Colouring is per
//! line *role*, with the one line that is really source run through the code
//! editor's tokenizer. An unrecognised line falls through as plain body text.

use bevy::prelude::*;

use crate::font::{ui_font, EmberFonts};
use crate::theme::*;
use crate::widgets::Tone;

/// Type size for the block. Small: it sits inside a graph node, and the whole
/// point is that it doesn't dominate the node it belongs to.
const SIZE: f32 = 10.0;

/// One coloured run of text — what each `TextSpan` ends up carrying.
type Run = (String, (u8, u8, u8));

/// Build the block for `text`, tinted for `tone`. It sizes to its content, so
/// `max_width` is what stops one long line from stretching whatever it is
/// appended to; past that the text wraps.
pub fn diagnostic_block(commands: &mut Commands, fonts: &EmberFonts, text: &str, tone: Tone, max_width: f32) -> Entity {
    let panel = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                max_width: Val::Px(max_width),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                border: UiRect::top(Val::Px(1.0)),
                // Only the bottom rounds: the block is a footer by nature, and it
                // meets the square bottom of whatever it is appended to.
                border_radius: BorderRadius::bottom(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(faint_bg())),
            BorderColor::all(rgb(tone.color())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("diagnostic-block"),
        ))
        .id();
    let root = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.mono, SIZE),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::linebreak(bevy::text::LineBreak::AnyCharacter),
            bevy::ui::FocusPolicy::Pass,
            Pickable::IGNORE,
        ))
        .id();
    let mut runs: Vec<Run> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if i > 0 {
            runs.push(("\n".to_string(), text_primary()));
        }
        runs.extend(line_runs(line, tone));
    }
    let spans: Vec<Entity> = runs
        .iter()
        .filter(|(s, _)| !s.is_empty())
        .map(|(s, color)| {
            commands
                .spawn((TextSpan::new(s.clone()), ui_font(&fonts.mono, SIZE), TextColor(rgb(*color))))
                .id()
        })
        .collect();
    commands.entity(root).add_children(&spans);
    commands.entity(panel).add_child(root);
    panel
}

/// Split one line into coloured runs by the role it plays in the diagnostic.
fn line_runs(line: &str, tone: Tone) -> Vec<Run> {
    let trimmed = line.trim_start();
    if let Some(runs) = severity_runs(line) {
        return runs;
    }
    if trimmed.starts_with("┌─") || trimmed.starts_with("-->") {
        let split = line.len() - trimmed.len() + trimmed.chars().take_while(|c| !c.is_whitespace()).map(char::len_utf8).sum::<usize>();
        return vec![
            (line[..split].to_string(), placeholder()),
            (line[split..].to_string(), text_muted()),
        ];
    }
    if let Some(bar) = gutter_split(line) {
        let mut runs = vec![(line[..bar].to_string(), syntax_palette().line_number)];
        runs.extend(body_runs(&line[bar..], tone));
        return runs;
    }
    if trimmed.starts_with('=') {
        let split = line.len() - trimmed.len() + 1;
        let mut runs = vec![(line[..split].to_string(), placeholder())];
        runs.extend(ticked(&line[split..], text_muted()));
        return runs;
    }
    body_runs(line, tone)
}

/// The `error[E0425]:` / `warning:` header. `note`/`help` keep their own colour
/// rather than the block's — a note inside an error block is not an error.
fn severity_runs(line: &str) -> Option<Vec<Run>> {
    let colon = line.find(':')?;
    let head = &line[..colon];
    let word = head.split('[').next().unwrap_or(head);
    let color = match word {
        "error" => close_red(),
        "warning" => warn_amber(),
        "note" | "help" => accent(),
        _ => return None,
    };
    let mut runs = vec![(head.to_string(), color), (":".to_string(), placeholder())];
    runs.extend(ticked(&line[colon + 1..], text_primary()));
    Some(runs)
}

/// Byte offset just past the gutter's `│` (or its ASCII `|`), if this line has
/// one. Bounded to the start of the line so a bar *inside* the source — a WGSL
/// `a | b` — is never mistaken for the gutter.
fn gutter_split(line: &str) -> Option<usize> {
    let (i, c) = line.char_indices().take(8).find(|(_, c)| *c == '│' || *c == '|')?;
    line[..i].chars().all(|c| c.is_ascii_digit() || c.is_whitespace()).then(|| i + c.len_utf8())
}

/// Everything right of a gutter, or a line with no role of its own: a run of
/// carets (with whatever label follows it) belongs to the diagnostic and takes
/// the block's tone; anything else is source, and goes through the tokenizer.
fn body_runs(text: &str, tone: Tone) -> Vec<Run> {
    let trimmed = text.trim_start();
    if trimmed.starts_with('^') || trimmed.starts_with('~') {
        return vec![(text.to_string(), tone.color())];
    }
    if trimmed.is_empty() {
        return vec![(text.to_string(), text_muted())];
    }
    crate::widgets::code_editor::highlight::tokenize(text)
}

/// Lift `` `identifier` `` out of prose — a compiler names the thing it is
/// complaining about in backticks, and that name is what the reader is looking
/// for.
fn ticked(text: &str, base: (u8, u8, u8)) -> Vec<Run> {
    text.split('`')
        .enumerate()
        .map(|(i, part)| {
            let quoted = i % 2 == 1;
            let s = if quoted { format!("`{part}`") } else { part.to_string() };
            (s, if quoted { accent() } else { base })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colors(line: &str) -> Vec<(u8, u8, u8)> {
        line_runs(line, Tone::Error).into_iter().map(|(_, c)| c).collect()
    }

    #[test]
    fn the_severity_word_is_coloured_by_its_own_word_not_the_block() {
        assert_eq!(colors("error: boom")[0], close_red());
        assert_eq!(colors("warning: hmm")[0], warn_amber());
        assert_eq!(colors("note: by the way")[0], accent());
    }

    /// `error` with a code attached is still the severity, and a line that only
    /// happens to contain a colon is not.
    #[test]
    fn severity_tolerates_a_code_and_rejects_a_bare_colon() {
        assert_eq!(colors("error[E0425]: boom")[0], close_red());
        assert!(severity_runs("resfult: f32 = a;").is_none());
    }

    /// A `|` inside the source must not be read as the gutter, or the tokenizer
    /// would lose the front of the line to the line-number colour.
    #[test]
    fn a_pipe_in_the_source_is_not_a_gutter() {
        assert_eq!(gutter_split("47 │     resfult = a;"), Some("47 │".len()));
        assert_eq!(gutter_split("   │"), Some("   │".len()));
        assert_eq!(gutter_split("    let m = a | b;"), None);
    }

    #[test]
    fn carets_take_the_blocks_tone() {
        assert_eq!(colors("        ^^^^^^^ unknown identifier"), vec![close_red()]);
        assert_eq!(line_runs("   ^^^ nope", Tone::Warn)[0].1, warn_amber());
    }

    /// The backticked name is what the reader is hunting for, so it has to
    /// survive as its own run rather than melting into the prose.
    #[test]
    fn backticked_names_are_lifted_out_of_prose() {
        let runs = line_runs("error: no definition for `resfult`", Tone::Error);
        assert!(runs.iter().any(|(s, c)| s == "`resfult`" && *c == accent()));
    }
}
