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
