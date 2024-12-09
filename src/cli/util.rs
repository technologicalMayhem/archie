use std::fmt::Display;
use itertools::Itertools;

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
            format!("{all_but_last}, and {last_part}",)
        }
    }
}
