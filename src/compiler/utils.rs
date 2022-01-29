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
