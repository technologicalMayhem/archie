use itertools::Itertools;
use std::fmt::Display;

pub fn combine_for_display<S, I>(list: S) -> String
where
    S: IntoIterator<Item = I>,
    I: AsRef<str> + Display,
{
    let list = list.into_iter().collect::<Vec<I>>();
    match list.len() {
        0 => String::new(),
        1 => list[0].to_string(),
        2 => format!("{} and {}", list[0], list[1]),
        _ => {
            let all_but_last = &list[..list.len() - 1];
            let last_part = list.last().unwrap();
            let all_but_last = all_but_last.iter().join(", ");
            format!("{all_but_last} and {last_part}")
        }
    }
}

pub fn wrap_text(text: &str, max_length: usize) -> String {
    let mut last_space = 0;
    let mut last_split = 0;
    let mut chars = 0;
    let mut lines: Vec<&str> = Vec::new();

    for (index, char) in text.chars().enumerate() {
        chars += 1;
        if char == ' ' {
            last_space = index;
        }
        if chars > max_length {
            let split = if last_space == last_split {
                index
            } else {
                last_space + 1
            };
            lines.push(&text[last_split..split]);
            lines.push("\n");
            last_split = split;
            chars = 0;
        }
    }
    if last_split != text.len() - 1 {
        lines.push(&text[last_split..]);
    }

    lines.into_iter().collect()
}
