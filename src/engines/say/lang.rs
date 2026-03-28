use tokio::process::Command;

pub(super) async fn system_lang() -> String {
    Command::new("defaults")
        .args(["read", "-g", "AppleLocale"])
        .output()
        .await
        .ok()
        .and_then(|output| {
            if output.status.success() {
                let locale = String::from_utf8_lossy(&output.stdout).trim().to_string();
                Some(locale.split('@').next().unwrap_or(&locale).to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "en_US".to_string())
}

pub(super) fn detect_lang(text: &str) -> Option<&'static str> {
    let mut cjk = 0u32;
    let mut kana = 0u32;
    let mut hangul = 0u32;
    let mut cyrillic = 0u32;
    let mut arabic = 0u32;
    let mut thai = 0u32;

    for ch in text.chars() {
        match ch {
            '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}' => kana += 1,
            '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' => cjk += 1,
            '\u{AC00}'..='\u{D7AF}' | '\u{1100}'..='\u{11FF}' => hangul += 1,
            '\u{0400}'..='\u{04FF}' => cyrillic += 1,
            '\u{0600}'..='\u{06FF}' => arabic += 1,
            '\u{0E00}'..='\u{0E7F}' => thai += 1,
            _ => {}
        }
    }

    if kana > 0 {
        return Some("ja_JP");
    }

    let latin = latin_word_count(text);
    let max = *[cjk, hangul, latin, cyrillic, arabic, thai]
        .iter()
        .max()
        .unwrap_or(&0);

    if max == 0 {
        return None;
    }
    if max == hangul {
        return Some("ko_KR");
    }
    if max == cyrillic {
        return Some("ru_RU");
    }
    if max == arabic {
        return Some("ar_SA");
    }
    if max == thai {
        return Some("th_TH");
    }
    if max == cjk {
        return Some("zh_CN");
    }

    Some("en_US")
}

fn latin_word_count(text: &str) -> u32 {
    text.split_whitespace()
        .filter(|token| {
            let trimmed = token.trim_matches(|ch: char| !ch.is_alphanumeric());
            !trimmed.is_empty()
                && trimmed
                    .chars()
                    .all(|ch| is_latin_letter(ch) || ch == '\'' || ch == '-')
                && trimmed.chars().any(is_latin_letter)
        })
        .count() as u32
}

fn is_latin_letter(ch: char) -> bool {
    matches!(ch, 'A'..='Z' | 'a'..='z' | '\u{00C0}'..='\u{024F}')
}

#[cfg(test)]
mod tests {
    use super::detect_lang;

    #[test]
    fn detect_lang_prioritizes_japanese_kana() {
        assert_eq!(detect_lang("会議です。こんにちは"), Some("ja_JP"));
    }

    #[test]
    fn detect_lang_handles_supported_scripts() {
        assert_eq!(detect_lang("该开会了"), Some("zh_CN"));
        assert_eq!(detect_lang("회의 시작"), Some("ko_KR"));
        assert_eq!(detect_lang("Привет"), Some("ru_RU"));
        assert_eq!(detect_lang("مرحبا"), Some("ar_SA"));
        assert_eq!(detect_lang("สวัสดี"), Some("th_TH"));
        assert_eq!(detect_lang("Bonjour"), Some("en_US"));
    }

    #[test]
    fn detect_lang_returns_none_for_non_language_input() {
        for sample in ["12345", "abc123", "/tmp/log.txt", "!!!", "🙂", "   "] {
            assert_eq!(
                detect_lang(sample),
                None,
                "sample {sample:?} should be unknown"
            );
        }
    }

    #[test]
    fn detect_lang_keeps_plain_english_alerts_as_english() {
        assert_eq!(detect_lang("build failed: abc123"), Some("en_US"));
    }
}
