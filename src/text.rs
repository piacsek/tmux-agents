pub fn truncate(text: &str, room: usize) -> String {
    let len = text.chars().count();
    if len <= room {
        return text.to_string();
    }
    if room == 0 {
        return String::new();
    }
    let mut out: String = text.chars().take(room - 1).collect();
    out.push('…');
    out
}
