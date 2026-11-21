//! Page selections for `--pages`.

/// Parses a 1-based page selection such as `1-3,7`, `5-` (page 5 to the end),
/// `-2` (the first two pages) or `all`, against a document of `count` pages.
///
/// The pages come back in the order they were written, so `--pages 3,1` reorders.
pub fn parse(spec: &str, count: usize) -> Result<Vec<usize>, String> {
    let spec = spec.trim();
    if spec.eq_ignore_ascii_case("all") {
        return Ok((1..=count).collect());
    }

    let mut pages = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (from, to) = match part.split_once('-') {
            None => {
                let n = number(part)?;
                (n, n)
            }
            Some((a, b)) => {
                let from = if a.trim().is_empty() { 1 } else { number(a)? };
                let to = if b.trim().is_empty() {
                    count
                } else {
                    number(b)?
                };
                (from, to)
            }
        };
        if from == 0 || to == 0 {
            return Err(format!("page numbers start at 1: '{part}'"));
        }
        if from > to {
            return Err(format!("empty range '{part}'"));
        }
        if to > count {
            return Err(format!(
                "page {to} does not exist, the document has {count} page{}",
                if count == 1 { "" } else { "s" }
            ));
        }
        pages.extend(from..=to);
    }

    if pages.is_empty() {
        return Err(format!("no pages in selection '{spec}'"));
    }
    Ok(pages)
}

fn number(s: &str) -> Result<usize, String> {
    s.trim()
        .parse::<usize>()
        .map_err(|_| format!("'{}' is not a page number", s.trim()))
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn single_ranges_and_open_ends() {
        assert_eq!(parse("2", 5).unwrap(), vec![2]);
        assert_eq!(parse("1-3,5", 5).unwrap(), vec![1, 2, 3, 5]);
        assert_eq!(parse("4-", 5).unwrap(), vec![4, 5]);
        assert_eq!(parse("-2", 5).unwrap(), vec![1, 2]);
        assert_eq!(parse("all", 3).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn keeps_the_written_order() {
        assert_eq!(parse("3,1", 5).unwrap(), vec![3, 1]);
    }

    #[test]
    fn rejects_bad_selections() {
        assert!(parse("0", 5).is_err());
        assert!(parse("6", 5).is_err());
        assert!(parse("3-2", 5).is_err());
        assert!(parse("x", 5).is_err());
        assert!(parse("", 5).is_err());
    }
}
