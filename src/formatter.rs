use syntect::easy::HighlightLines;
use syntect::parsing::SyntaxSet;
use syntect::highlighting::{ThemeSet, Style};
use syntect::util::as_24_bit_terminal_escaped;

pub fn print_highlighted_json(text: &str) {
    print_highlighted(text, "json");
}

pub fn print_highlighted_xml(text: &str) {
    print_highlighted(text, "xml");
}

fn print_highlighted(text: &str, extension: &str) {
    let ps = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let syntax = ps.find_syntax_by_extension(extension)
        .unwrap_or_else(|| ps.find_syntax_plain_text());
    let theme = &ts.themes["base16-ocean.dark"];
    let mut h = HighlightLines::new(syntax, theme);
    for line in text.lines() {
        if let Ok(ranges) = h.highlight_line(&(line.to_string() + "\n"), &ps) {
            let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
            print!("{}", escaped);
        } else {
            println!("{}", line);
        }
    }
    println!();
}
