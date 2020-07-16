use anyhow::Result;
use pulldown_cmark::{Event, Options, Parser};
use std::io::{BufWriter, Write};

/// Converts the given Markdown to Gemini, writing it to the given output. The output will be
/// automatically buffered.
pub fn to_gemini<W: Write>(markdown: &str, out: W) -> Result<()> {
    let parser = Parser::new(markdown);
    let mut out = BufWriter::new(out);
    for event in parser {
        if let Event::Text(text) = event {
            out.write_all(text.as_bytes())?;
        }
    }
    Ok(())
}
