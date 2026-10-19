//! A small PDF writer for text reports (A4, Helvetica, several pages).
//! Enough for diagnostic reports without a PDF library.

const W: f64 = 595.0;
const H: f64 = 842.0;
const MARGIN: f64 = 50.0;
const SIZE: f64 = 10.0;
const LEAD: f64 = 14.0;
const MAX_CHARS: usize = 95;

/// Maps text to WinAnsi bytes; characters outside it become '?' (λ → "lambda").
fn win_ansi(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for c in s.chars() {
        match c {
            '\u{20}'..='\u{7E}' => out.push(c as u8),
            '\u{A0}'..='\u{FF}' => out.push(c as u32 as u8),
            '–' => out.push(0x96),
            '—' => out.push(0x97),
            '…' => out.push(0x85),
            '→' => out.extend_from_slice(b"->"),
            'λ' => out.extend_from_slice(b"lambda"),
            'Ω' => out.extend_from_slice(b"ohm"),
            '“' | '”' => out.push(b'"'),
            '‘' | '’' => out.push(b'\''),
            _ => out.push(b'?'),
        }
    }
    out
}

fn escape(b: &[u8]) -> Vec<u8> {
    let mut o = Vec::new();
    for &c in b {
        if matches!(c, b'(' | b')' | b'\\') {
            o.push(b'\\');
        }
        o.push(c);
    }
    o
}

fn wrap(text: &str) -> Vec<String> {
    let indent: String = text.chars().take_while(|c| *c == ' ').collect();
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > MAX_CHARS {
            lines.push(std::mem::take(&mut cur));
            cur.push_str(&indent);
        } else if !cur.is_empty() {
            cur.push(' ');
        } else {
            cur.push_str(&indent);
        }
        cur.push_str(word);
    }
    lines.push(cur);
    lines
}

/// `lines`: (bold, text). Returns the PDF file bytes.
pub fn render(lines: &[(bool, String)]) -> Vec<u8> {
    let mut pages: Vec<Vec<u8>> = Vec::new();
    let mut content = Vec::new();
    let mut y = H - MARGIN;
    let flush =
        |content: &mut Vec<u8>, pages: &mut Vec<Vec<u8>>| pages.push(std::mem::take(content));
    for (bold, text) in lines {
        for l in wrap(text) {
            if y < MARGIN {
                flush(&mut content, &mut pages);
                y = H - MARGIN;
            }
            let font = if *bold { "F2" } else { "F1" };
            content
                .extend_from_slice(format!("BT /{font} {SIZE} Tf {MARGIN} {y:.1} Td (").as_bytes());
            content.extend(escape(&win_ansi(&l)));
            content.extend_from_slice(b") Tj ET\n");
            y -= LEAD;
        }
    }
    flush(&mut content, &mut pages);

    // Objects: 1 catalog, 2 pages, 3 F1, 4 F2, then (page, content) pairs.
    let mut objs: Vec<Vec<u8>> = Vec::new();
    let page_ids: Vec<usize> = (0..pages.len()).map(|i| 5 + i * 2).collect();
    objs.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    let kids: String = page_ids
        .iter()
        .map(|id| format!("{id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");
    objs.push(format!("<< /Type /Pages /Kids [{kids}] /Count {} >>", pages.len()).into_bytes());
    objs.push(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    objs.push(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    for (i, c) in pages.iter().enumerate() {
        let content_id = page_ids[i] + 1;
        objs.push(
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {W} {H}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {content_id} 0 R >>")
                .into_bytes(),
        );
        let mut s = format!("<< /Length {} >>\nstream\n", c.len()).into_bytes();
        s.extend_from_slice(c);
        s.extend_from_slice(b"\nendstream");
        objs.push(s);
    }
    let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn makes_a_multi_page_pdf() {
        let lines: Vec<(bool, String)> = (0..120)
            .map(|i| {
                (
                    i % 10 == 0,
                    format!("Line {i} (with brackets) and 90 °C – λ"),
                )
            })
            .collect();
        let pdf = render(&lines);
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/Count 3"));
        assert!(text.contains("\\(with brackets\\)"));
        assert!(text.contains("lambda"));
        // startxref points at the xref table (byte offsets, so search the bytes).
        let tail = &pdf[pdf.windows(10).rposition(|w| w == b"startxref\n").unwrap() + 10..];
        let start: usize = std::str::from_utf8(tail)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(pdf[start..].starts_with(b"xref"));
    }

    #[test]
    fn wraps_long_lines() {
        let long = "word ".repeat(60);
        assert!(wrap(&long).len() >= 3);
        assert_eq!(wrap("").len(), 1);
    }
}
