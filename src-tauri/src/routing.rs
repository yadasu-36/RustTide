use crate::settings::{Direction, Output, Settings};

#[derive(Clone, Copy)]
pub enum Trigger {
    Read,
    Write,
}

fn language_code(language: &str) -> &str {
    match language {
        "English" => "en",
        "简体中文" => "zh-Hans",
        "繁體中文" => "zh-Hant",
        "日本語" => "ja",
        "한국어" => "ko",
        "Español" => "es",
        "Français" => "fr",
        "Deutsch" => "de",
        "Português" => "pt",
        "Русский" => "ru",
        "العربية" => "ar",
        "हिन्दी" => "hi",
        _ => "",
    }
}
fn matches_language(language: &str, detected: &str) -> bool {
    let code = language_code(language);
    if code.starts_with("zh-") && detected.starts_with("zh-") {
        return code == detected;
    }
    code.split('-').next() == detected.split('-').next()
}
impl Trigger {
    pub fn output(self, settings: &Settings) -> Output {
        match self {
            Self::Read => settings.read_output,
            Self::Write => settings.write_output,
        }
    }
    pub fn whole_input(self, settings: &Settings) -> bool {
        matches!(self, Self::Write) || self.output(settings) == Output::Replace
    }
    pub fn target(self, settings: &Settings, detected: &str) -> String {
        let direction = match self {
            Self::Read => settings.read_direction,
            Self::Write => settings.write_direction,
        };
        let native = &settings.native_language;
        let foreign = &settings.foreign_language;
        match direction {
            Direction::Native => native.clone(),
            Direction::Foreign => foreign.clone(),
            Direction::Auto => {
                if matches_language(native, detected) {
                    foreign.clone()
                } else if matches_language(foreign, detected) || matches!(self, Self::Read) {
                    native.clone()
                } else {
                    foreign.clone()
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn read_selection_and_write_whole_field_have_distinct_defaults() {
        let settings = Settings::default();
        assert!(!Trigger::Read.whole_input(&settings));
        assert!(Trigger::Write.whole_input(&settings));
        assert_eq!(Trigger::Read.output(&settings), Output::Popup);
        assert_eq!(Trigger::Write.output(&settings), Output::Replace);
        assert_eq!(Trigger::Read.target(&settings, "en"), "简体中文");
        assert_eq!(Trigger::Write.target(&settings, "zh-Hans"), "English");
        assert_eq!(Trigger::Write.target(&settings, "en"), "简体中文");
        assert_eq!(Trigger::Read.target(&settings, "und"), "简体中文");
        assert_eq!(Trigger::Write.target(&settings, "und"), "English");
    }
    #[test]
    fn write_preview_still_captures_whole_input_and_fixed_direction_wins() {
        let settings = Settings {
            write_output: Output::Popup,
            write_direction: Direction::Foreign,
            ..Settings::default()
        };
        assert!(Trigger::Write.whole_input(&settings));
        assert_eq!(Trigger::Write.target(&settings, "en"), "English");
    }
    #[test]
    fn chinese_script_direction_is_preserved() {
        let settings = Settings {
            foreign_language: "繁體中文".into(),
            ..Settings::default()
        };
        assert_eq!(Trigger::Write.target(&settings, "zh-Hant"), "简体中文");
        assert_eq!(Trigger::Write.target(&settings, "zh-Hans"), "繁體中文");
    }
}
