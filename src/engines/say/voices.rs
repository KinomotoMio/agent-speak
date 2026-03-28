use crate::engine::VoiceInfo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct VoiceEntry {
    pub name: String,
    pub locale: String,
}

const NOVELTY_VOICES: &[&str] = &[
    "Eddy",
    "Flo",
    "Grandma",
    "Grandpa",
    "Reed",
    "Rocko",
    "Sandy",
    "Shelley",
    "Albert",
    "Bad News",
    "Bahh",
    "Bells",
    "Boing",
    "Bubbles",
    "Cellos",
    "Good News",
    "Jester",
    "Organ",
    "Superstar",
    "Trinoids",
    "Whisper",
    "Wobble",
    "Zarvox",
];

pub(super) fn parse_voice_entries(output: &str) -> Vec<VoiceEntry> {
    output.lines().filter_map(parse_voice_line).collect()
}

pub(super) fn voice_for_lang(entries: &[VoiceEntry], lang: &str) -> Option<String> {
    let matches = matching_voices(entries, lang);
    choose_best_voice(&matches, lang)
}

pub(super) fn to_voice_infos(entries: &[VoiceEntry]) -> Vec<VoiceInfo> {
    entries
        .iter()
        .map(|entry| VoiceInfo {
            id: entry.name.clone(),
            name: entry.name.clone(),
            locale: Some(entry.locale.clone()),
            description: None,
        })
        .collect()
}

fn parse_voice_line(line: &str) -> Option<VoiceEntry> {
    let prefix = line.split('#').next().unwrap_or(line).trim_end();
    if prefix.is_empty() {
        return None;
    }

    let locale = prefix.split_whitespace().last()?;
    if !locale.contains('_') {
        return None;
    }

    let locale_idx = prefix.rfind(locale)?;
    let name = prefix[..locale_idx].trim_end();
    if name.is_empty() {
        return None;
    }

    Some(VoiceEntry {
        name: name.to_string(),
        locale: locale.to_string(),
    })
}

fn matching_voices(entries: &[VoiceEntry], lang: &str) -> Vec<String> {
    let mut matches: Vec<_> = entries
        .iter()
        .filter(|entry| entry.locale == lang)
        .map(|entry| entry.name.clone())
        .collect();

    if matches.is_empty() {
        let prefix = lang.split('_').next().unwrap_or(lang);
        matches = entries
            .iter()
            .filter(|entry| {
                entry.locale.starts_with(prefix)
                    && entry.locale.as_bytes().get(prefix.len()) == Some(&b'_')
            })
            .map(|entry| entry.name.clone())
            .collect();
    }

    matches
}

fn choose_best_voice(voices: &[String], lang: &str) -> Option<String> {
    if voices.is_empty() {
        return None;
    }

    for preferred in preferred_voices(lang) {
        if let Some(voice) = voices
            .iter()
            .find(|voice| base_voice_name(voice) == *preferred)
        {
            return Some(voice.clone());
        }
    }

    for voice in voices {
        if !NOVELTY_VOICES.contains(&base_voice_name(voice)) {
            return Some(voice.clone());
        }
    }

    voices.first().cloned()
}

fn base_voice_name(name: &str) -> &str {
    name.split('(').next().unwrap_or(name).trim()
}

fn preferred_voices(lang: &str) -> &'static [&'static str] {
    match lang {
        "zh_CN" => &["Tingting", "Lili", "Shanshan"],
        "zh_TW" => &["Meijia"],
        "zh_HK" => &["Sinji"],
        "ja_JP" => &["Kyoko", "Otoya"],
        "ko_KR" => &["Yuna"],
        "en_US" => &["Samantha", "Alex", "Tom", "Karen"],
        "en_GB" => &["Daniel", "Kate", "Serena"],
        "fr_FR" => &["Thomas", "Amelie", "Audrey"],
        "de_DE" => &["Anna", "Markus", "Petra"],
        "es_ES" => &["Monica", "Jorge"],
        "ru_RU" => &["Milena", "Yuri"],
        "pt_BR" => &["Luciana", "Felipe"],
        "it_IT" => &["Alice", "Luca"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_voice_entries, voice_for_lang};

    const SAMPLE_VOICES: &str = "\
Bad News            en_US    # Hello! My name is Bad News.\n\
Samantha            en_US    # Hello! My name is Samantha.\n\
Monica              es_ES    # Hola! Me llamo Monica.\n\
Paulina             es_MX    # Hola! Me llamo Paulina.\n\
Eddy (中文（中国大陆）)     zh_CN    # 你好！我叫Eddy。\n\
Tingting (中文（中国大陆）) zh_CN    # 你好！我叫婷婷。\n\
Kyoko               ja_JP    # こんにちは！\n";

    #[test]
    fn parse_voice_entries_preserves_full_voice_names() {
        let entries = parse_voice_entries(SAMPLE_VOICES);
        assert_eq!(entries[0].name, "Bad News");
        assert_eq!(entries[4].name, "Eddy (中文（中国大陆）)");
        assert_eq!(entries[5].name, "Tingting (中文（中国大陆）)");
    }

    #[test]
    fn voice_selection_falls_back_to_language_prefix() {
        let entries = parse_voice_entries(SAMPLE_VOICES);
        let voice = voice_for_lang(&entries, "es_AR");
        assert_eq!(voice, Some("Monica".to_string()));
    }

    #[test]
    fn voice_selection_prefers_real_voice_for_language() {
        let entries = parse_voice_entries(SAMPLE_VOICES);
        let voice = voice_for_lang(&entries, "zh_CN");
        assert_eq!(voice, Some("Tingting (中文（中国大陆）)".to_string()));
    }
}
