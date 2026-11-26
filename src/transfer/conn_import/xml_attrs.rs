//! Just enough XML for Navicat's `.ncx` files: the attributes of every element with a
//! given tag. Element text, nesting and namespaces don't matter there.

pub(super) type Attrs = Vec<(String, String)>;

/// The attributes of each `<tag …>` in `xml`, in order.
pub(super) fn elements(xml: &str, tag: &str) -> Vec<Attrs> {
    let open = format!("<{}", tag);
    let mut out = vec![];
    let mut rest = xml;
    while let Some(i) = rest.find(&open) {
        rest = &rest[i + open.len()..];
        // `<Connection` must not match `<Connections`.
        if !rest.starts_with(char::is_whitespace) {
            continue;
        }
        let (attrs, tail) = attributes(rest);
        out.push(attrs);
        rest = tail;
    }
    out
}

/// `name="value"` pairs up to the end of the tag, and the text after them.
fn attributes(s: &str) -> (Attrs, &str) {
    let mut attrs = vec![];
    let mut rest = s.trim_start();
    while !rest.is_empty() && !rest.starts_with('>') && !rest.starts_with("/>") {
        let Some((pair, tail)) = attribute(rest) else { return (attrs, "") };
        attrs.push(pair);
        rest = tail.trim_start();
    }
    (attrs, rest)
}

fn attribute(s: &str) -> Option<((String, String), &str)> {
    let eq = s.find('=')?;
    let name = s[..eq].trim().to_string();
    let after = s[eq + 1..].trim_start();
    let quote = after.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let body = &after[1..];
    let end = body.find(quote)?;
    Some(((name, unescape(&body[..end])), &body[end + 1..]))
}

pub(super) fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        match rest.find(';').and_then(|end| entity(&rest[1..end]).map(|c| (c, end))) {
            Some((c, end)) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let num = name.strip_prefix('#')?;
            let code = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => num.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}
