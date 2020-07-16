use anyhow::Result;
use pulldown_cmark::{Event, Options, Parser};
use std::io::{BufWriter, Write};

/// Converts the given Markdown to Gemini, writing it to the given output. The output will be
/// automatically buffered.
pub fn to_gemini(markdown: &str) -> Result<Vec<u8>> {
    let parser = Parser::new(markdown);
    let mut vec: Vec<u8> = vec![];
    {
        let mut out = BufWriter::new(&mut vec);
        for event in parser {
            if let Event::Text(text) = event {
                out.write_all(text.as_bytes())?;
            }
        }
    }
    Ok(vec)
}
