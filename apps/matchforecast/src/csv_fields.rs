//! Minimal RFC-4180 CSV field parsing shared by the match-forecast binaries.
//!
//! Registered squad names are written "Surname, Given Middle" and player
//! CSVs quote such fields; naive `split(',')` silently shifts every later
//! column. Callers must fail loudly on malformed rows rather than poison
//! downstream columns.

/// Parse one CSV line: a quoted field may contain commas and doubled quotes.
/// Returns `None` only for an unterminated quote.
pub fn parse_csv_fields(
    line: &str,
    path: &str,
    line_number: usize,
) -> Option<Vec<String>> {
    let line = line.strip_suffix('\r').unwrap_or(line);
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
        } else {
            match c {
                ',' => fields.push(std::mem::take(&mut field)),
                '"' if field.is_empty() => in_quotes = true,
                _ => field.push(c),
            }
        }
    }
    if in_quotes {
        eprintln!("malformed CSV {path} line {line_number}: unterminated quoted field");
        return None;
    }
    fields.push(field);
    Some(fields)
}

#[cfg(test)]
mod tests {
    use super::parse_csv_fields as parse;

    #[test]
    fn quoted_commas_and_doubled_quotes_round_trip() {
        let fields = parse(r#"14035968,"Franca De Oliveira, Matheus",Crystal Palace,"say ""hi""",901"#, "t", 2).unwrap();
        assert_eq!(fields.len(), 5);
        assert_eq!(fields[0], "14035968");
        assert_eq!(fields[1], "Franca De Oliveira, Matheus");
        assert_eq!(fields[2], "Crystal Palace");
        assert_eq!(fields[3], r#"say "hi""#);
        assert_eq!(fields[4], "901");
    }

    #[test]
    fn unterminated_quote_is_none() {
        assert!(parse(r#""open,Crystal Palace,90"#, "t", 3).is_none());
    }

    #[test]
    fn plain_rows_unchanged() {
        let fields = parse("a,b,c", "t", 4).unwrap();
        assert_eq!(fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn crlf_line_endings_are_stripped() {
        // Real callers pass `text.lines()` output, which strips \n and leaves \r.
        let fields = parse("a,b,c\r", "t", 5).unwrap();
        assert_eq!(fields.last().map(String::as_str), Some("c"));
    }
}