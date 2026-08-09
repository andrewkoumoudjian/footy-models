/// Normalize a football name deterministically without a heavyweight Unicode dependency.
///
/// Common Latin diacritics are folded to ASCII. Other alphabetic Unicode characters are
/// lowercased and retained. Punctuation and repeated whitespace become a single space.
pub fn normalize_name(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pending_space = false;

    for ch in input.chars() {
        if let Some(folded) = fold_latin(ch) {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.push_str(folded);
            continue;
        }

        if ch.is_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
        } else if !out.is_empty() {
            pending_space = true;
        }
    }

    out
}

pub fn normalized_surname(input: &str) -> String {
    normalize_name(input)
        .split_whitespace()
        .last()
        .unwrap_or_default()
        .to_owned()
}

/// Normalized Levenshtein similarity in `[0, 1]`.
pub fn name_similarity(left: &str, right: &str) -> f32 {
    let left = normalize_name(left);
    let right = normalize_name(right);

    if left == right {
        return 1.0;
    }
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }

    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();
    let distance = levenshtein(&left_chars, &right_chars);
    let denominator = left_chars.len().max(right_chars.len()) as f32;
    1.0 - (distance as f32 / denominator)
}

fn levenshtein(left: &[char], right: &[char]) -> usize {
    if left.len() > right.len() {
        return levenshtein(right, left);
    }

    let mut previous: Vec<usize> = (0..=left.len()).collect();
    let mut current = vec![0; left.len() + 1];

    for (right_index, right_char) in right.iter().enumerate() {
        current[0] = right_index + 1;
        for (left_index, left_char) in left.iter().enumerate() {
            let substitution = previous[left_index] + if left_char == right_char { 0 } else { 1 };
            let insertion = current[left_index] + 1;
            let deletion = previous[left_index + 1] + 1;
            current[left_index + 1] = substitution.min(insertion).min(deletion);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[left.len()]
}

fn fold_latin(ch: char) -> Option<&'static str> {
    match ch {
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => Some("a"),
        'Æ' | 'æ' => Some("ae"),
        'Ç' | 'ç' => Some("c"),
        'È' | 'É' | 'Ê' | 'Ë' | 'è' | 'é' | 'ê' | 'ë' => Some("e"),
        'Ì' | 'Í' | 'Î' | 'Ï' | 'ì' | 'í' | 'î' | 'ï' => Some("i"),
        'Ñ' | 'ñ' => Some("n"),
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => Some("o"),
        'Œ' | 'œ' => Some("oe"),
        'Ù' | 'Ú' | 'Û' | 'Ü' | 'ù' | 'ú' | 'û' | 'ü' => Some("u"),
        'Ý' | 'Ÿ' | 'ý' | 'ÿ' => Some("y"),
        'Š' | 'š' => Some("s"),
        'Ž' | 'ž' => Some("z"),
        'Ð' | 'ð' => Some("d"),
        'Þ' | 'þ' => Some("th"),
        'Ł' | 'ł' => Some("l"),
        'ß' => Some("ss"),
        _ if ch.is_ascii_alphanumeric() => None,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_common_diacritics_and_punctuation() {
        assert_eq!(normalize_name("  João  Félix-Séqueira "), "joao felix sequeira");
        assert_eq!(normalize_name("MARTIN ØDEGAARD"), "martin odegaard");
    }

    #[test]
    fn extracts_normalized_surname() {
        assert_eq!(normalized_surname("Kylian Mbappé Lottin"), "lottin");
    }

    #[test]
    fn similarity_is_bounded() {
        let similarity = name_similarity("Alexander Isak", "Alex Isak");
        assert!(similarity > 0.5 && similarity < 1.0);
        assert_eq!(name_similarity("João Félix", "Joao Felix"), 1.0);
    }
}
