pub fn is_space(c: char) -> bool {
    match c {
        ' ' | '\t' | '\n' | '\r' => true,
        _ => false,
    }
}

pub fn write_str(input: &str, dest: &mut Vec<char>) {
    for c in input.chars() {
        dest.push(c);
    }
}

pub fn write_str_escaped(input: &str, quote: char, escape_char: char, dest: &mut Vec<char>) {
    for c in input.chars() {
        if c == quote || c == escape_char {
            dest.push(escape_char);
        }
        dest.push(c);
    }
}
