use crate::ttml_processor::types::ParsedSourceData;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum CharType {
    Cjk,
    Latin,
    Numeric,
    Whitespace,
    Other,
}

fn get_char_type(c: char) -> CharType {
    if c.is_whitespace() {
        CharType::Whitespace
    } else if c.is_ascii_alphabetic() {
        CharType::Latin
    } else if c.is_ascii_digit() {
        CharType::Numeric
    } else if (0x4E00..=0x9FFF).contains(&(c as u32))
        || (0x3040..=0x309F).contains(&(c as u32))
        || (0x30A0..=0x30FF).contains(&(c as u32))
        || (0xAC00..=0xD7AF).contains(&(c as u32))
    {
        CharType::Cjk
    } else {
        CharType::Other
    }
}

fn auto_tokenize(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut tokens = Vec::new();
    let mut current_token = String::new();
    let mut last_char_type: Option<CharType> = None;
    for grapheme in unicode_segmentation::UnicodeSegmentation::graphemes(text, true) {
        let first_char = grapheme.chars().next().unwrap_or(' ');
        let current_char_type = get_char_type(first_char);

        if let Some(last_type) = last_char_type {
            let should_break = !matches!(
                (last_type, current_char_type),
                (CharType::Latin, CharType::Latin)
                    | (CharType::Numeric, CharType::Numeric)
                    | (CharType::Whitespace, CharType::Whitespace)
            );
            if should_break && !current_token.is_empty() {
                tokens.push(current_token);
                current_token = String::new();
            }
        }
        current_token.push_str(grapheme);
        last_char_type = Some(current_char_type);
    }
    if !current_token.is_empty() {
        tokens.push(current_token);
    }
    tokens
}

fn split_line_into_syllables(
    line_text: &str,
    start_ms: u64,
    end_ms: u64,
    punctuation_weight: f64,
) -> Vec<crate::ttml_processor::types::LyricSyllable> {
    let tokens = auto_tokenize(line_text);
    if tokens.is_empty() {
        return vec![];
    }

    let last_visible_token_index = tokens.iter().rposition(|token| {
        get_char_type(token.chars().next().unwrap_or(' ')) != CharType::Whitespace
    });

    let total_weight: f64 = tokens
        .iter()
        .map(|token| {
            let first_char = token.chars().next().unwrap_or(' ');
            match get_char_type(first_char) {
                CharType::Latin | CharType::Numeric | CharType::Cjk => token.chars().count() as f64,
                CharType::Other => punctuation_weight,
                CharType::Whitespace => 0.0,
            }
        })
        .sum();

    if total_weight <= 0.0 {
        return vec![];
    }

    let mut new_syllables: Vec<super::types::LyricSyllable> = Vec::new();
    let total_duration = end_ms.saturating_sub(start_ms);
    let duration_per_weight = total_duration as f64 / total_weight;
    let mut current_token_start_ms = start_ms;
    let mut accumulated_weight = 0.0;

    for (token_idx, token) in tokens.iter().enumerate() {
        let char_type = get_char_type(token.chars().next().unwrap_or(' '));

        if char_type == CharType::Whitespace {
            if let Some(last_syl) = new_syllables.last_mut() {
                last_syl.ends_with_space = true;
            }
            continue;
        }

        let token_weight = match char_type {
            CharType::Latin | CharType::Numeric | CharType::Cjk => token.chars().count() as f64,
            CharType::Other => punctuation_weight,
            _ => 0.0,
        };
        accumulated_weight += token_weight;

        let mut token_end_ms = start_ms + (accumulated_weight * duration_per_weight).round() as u64;
        if Some(token_idx) == last_visible_token_index {
            token_end_ms = end_ms;
        }

        new_syllables.push(crate::ttml_processor::types::LyricSyllable {
            text: token.clone(),
            start_ms: current_token_start_ms,
            end_ms: token_end_ms,
            duration_ms: Some(token_end_ms.saturating_sub(current_token_start_ms)),
            ends_with_space: false,
        });

        current_token_start_ms = token_end_ms;
    }

    new_syllables
}

pub fn apply_auto_word_splitting(parsed_data: &mut ParsedSourceData) {
    for line in &mut parsed_data.lines {
        if line.main_syllables.len() <= 1
            && line.end_ms > line.start_ms
            && let Some(line_text) = &line.line_text
            && !line_text.trim().is_empty()
        {
            let new_syllables =
                split_line_into_syllables(line_text, line.start_ms, line.end_ms, 0.5);
            if !new_syllables.is_empty() {
                line.main_syllables = new_syllables;
            }
        }
    }
}
