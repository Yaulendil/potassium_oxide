use std::ops::Range;


const fn is_quote(char: u8) -> bool {
    matches!(char, b'"' | b'\'' | b'`')
}


const fn get_quoted(text: &str) -> Option<Range<usize>> {
    let last: usize = text.len() - 1;
    let c0: u8 = text.as_bytes()[0];
    let c1: u8 = text.as_bytes()[last];

    if c0 == c1 && is_quote(c0) {
        Some(1..last)
    } else {
        None
    }
}


pub fn unquote(text: &str) -> &str {
    match get_quoted(text) {
        Some(inner) => &text[inner],
        None => text,
    }
}


#[allow(unused_assignments)]
pub fn split_cmd(line: &str) -> (&str, Vec<&str>) {
    let bytes: &[u8] = line.as_bytes();
    let end: usize = line.len();

    let mut index: usize = 0;
    let mut quote: Option<u8> = None;
    let mut words: Vec<&str> = Vec::new();
    let mut word_idx: usize = 0;

    macro_rules! push {() => {{
        let word = &line[word_idx..index].trim();
        word_idx = index; // Assignment incorrectly flagged by rustc as unused.

        if !word.is_empty() { words.push(word); }
    }}}

    macro_rules! word_start {() => {
        index == 0 || bytes[index - 1] == b' '
    }}
    macro_rules! word_end {() => {
        index + 1 == end || matches!(bytes[index + 1], b' ' | b';')
    }}

    while index < end {
        match quote {
            None => match bytes[index] {
                b';' => break,
                b' ' => push!(),
                q => if is_quote(q) && word_start!() { quote = Some(q); }
            }
            Some(q) => if bytes[index] == q && word_end!() {
                quote = None;
            } else if index + 1 == end {
                quote = None;

                word_idx += 1;
                index = word_idx;
            }
        }

        index += 1;
    }

    push!();
    (&line[..index], words)
}


pub fn substring_to_end<'a>(main: &'a str, sub: &str) -> Option<&'a str> {
    let valid = main.as_bytes().as_ptr_range();

    if !sub.is_empty() && valid.contains(&sub.as_ptr()) {
        let idx = unsafe { sub.as_ptr().offset_from(valid.start) } as usize;

        Some(&main[idx..])
    } else {
        None
    }
}
