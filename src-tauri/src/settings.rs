use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use url::Url;

pub const LANGUAGES: &[&str] = &[
    "English",
    "简体中文",
    "繁體中文",
    "日本語",
    "한국어",
    "Español",
    "Français",
    "Deutsch",
    "Português",
    "Русский",
    "العربية",
    "हिन्दी",
];
pub const STYLES: &[&str] = &["natural", "formal", "concise"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    #[default]
    Auto,
    Native,
    Foreign,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Output {
    Popup,
    Replace,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub base_url: String,
    pub model: String,
    pub native_language: String,
    #[serde(alias = "targetLanguage")]
    pub foreign_language: String,
    pub read_direction: Direction,
    pub write_direction: Direction,
    pub read_output: Output,
    pub write_output: Output,
    pub style: String,
    pub timeout_seconds: u64,
    pub read_shortcut: String,
    pub replace_shortcut: String,
    pub clipboard_fallback: bool,
    pub use_system_proxy: bool,
    pub excluded_apps: Vec<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-4o-mini".into(),
            native_language: "简体中文".into(),
            foreign_language: "English".into(),
            read_direction: Direction::Auto,
            write_direction: Direction::Auto,
            read_output: Output::Popup,
            write_output: Output::Replace,
            style: "natural".into(),
            timeout_seconds: 45,
            read_shortcut: "Alt+D".into(),
            replace_shortcut: "Alt+R".into(),
            clipboard_fallback: true,
            use_system_proxy: true,
            excluded_apps: vec![],
        }
    }
}
pub fn is_loopback(url: &Url) -> bool {
    url.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    })
}
pub fn endpoint(base: &str) -> Result<Url, String> {
    let mut url = Url::parse(base.trim()).map_err(|_| "API 地址格式不正确。")?;
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("API 地址不能含账户、密钥、查询参数或片段。".into());
    }
    if url.scheme() != "https" && !(url.scheme() == "http" && is_loopback(&url)) {
        return Err("远程 API 必须使用 HTTPS；本机 API 可使用 HTTP。".into());
    }
    let path = url.path().trim_end_matches('/');
    if !path.ends_with("/chat/completions") {
        url.set_path(&format!("{path}/chat/completions"));
    }
    Ok(url)
}
impl Settings {
    pub fn validate(&mut self) -> Result<(), String> {
        self.base_url = self.base_url.trim().trim_end_matches('/').to_string();
        endpoint(&self.base_url)?;
        self.model = self.model.trim().to_string();
        if self.model.is_empty() || self.model.len() > 200 {
            return Err("请填写有效的模型名称。".into());
        }
        if !LANGUAGES.contains(&self.native_language.as_str())
            || !LANGUAGES.contains(&self.foreign_language.as_str())
            || !STYLES.contains(&self.style.as_str())
        {
            return Err("语言或翻译风格不受支持。".into());
        }
        if self.native_language == self.foreign_language {
            return Err("母语和外语需要选择不同的语言。".into());
        }
        self.excluded_apps = self
            .excluded_apps
            .iter()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .collect();
        self.excluded_apps.sort();
        self.excluded_apps.dedup();
        if self.excluded_apps.len() > 100
            || self.excluded_apps.iter().any(|id| {
                id.len() > 200
                    || !id.contains('.')
                    || !id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || ".-_".contains(c))
            })
        {
            return Err("排除应用需要填写有效的 Bundle ID，每行一个。".into());
        }
        if !(5..=120).contains(&self.timeout_seconds) {
            return Err("超时需要在 5–120 秒之间。".into());
        }
        if self.read_shortcut.is_empty() || self.replace_shortcut.is_empty() {
            return Err("请填写两个快捷键。".into());
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = fs::read(path).map_err(|_| "无法读取设置文件。")?;
        let mut settings = Self::from_json(&data)?;
        settings.validate()?;
        Ok(settings)
    }
    fn from_json(data: &[u8]) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_slice(data).map_err(|_| "设置文件无效，请修复或删除后重启。")?;
        let legacy = value.get("targetLanguage").is_some() && value.get("nativeLanguage").is_none();
        let mut settings: Self =
            serde_json::from_value(value).map_err(|_| "设置文件无效，请修复或删除后重启。")?;
        if legacy && settings.native_language == settings.foreign_language {
            settings.native_language = "English".into();
        }
        Ok(settings)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let parent = path.parent().ok_or("设置目录无效。")?;
        fs::create_dir_all(parent).map_err(|_| "无法创建设置目录。")?;
        let data = serde_json::to_vec_pretty(self).map_err(|_| "无法序列化设置。")?;
        let temp = path.with_extension("tmp");
        fs::write(&temp, data).map_err(|_| "无法写入设置文件。")?;
        fs::rename(&temp, path).map_err(|_| "无法更新设置文件。".to_string())
    }
    pub fn key_account(&self) -> Result<String, String> {
        Ok(endpoint(&self.base_url)?.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn urls_normalize_and_protect_credentials() {
        assert_eq!(
            endpoint("https://api.example.com/v1/").unwrap().as_str(),
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(
            endpoint("http://[::1]:1234/v1/chat/completions")
                .unwrap()
                .path(),
            "/v1/chat/completions"
        );
        for url in [
            "http://example.com/v1",
            "https://user:secret@example.com",
            "https://x.com/?key=secret",
            "file:///tmp/test",
            "https://x.com/#secret",
        ] {
            assert!(endpoint(url).is_err(), "{url}");
        }
        assert!(endpoint("http://127.0.0.1:11434/v1").is_ok());
        assert!(endpoint("http://localhost.evil.test/v1").is_err());
    }
    #[test]
    fn defaults_and_limits() {
        let mut settings = Settings::default();
        settings.validate().unwrap();
        let serialized = serde_json::to_string(&settings).unwrap();
        assert!(!serialized.contains("apiKey"));
        settings.timeout_seconds = 0;
        assert!(settings.validate().is_err());
        settings.timeout_seconds = 45;
        settings.foreign_language = "untrusted prompt".into();
        assert!(settings.validate().is_err());
    }
    #[test]
    fn legacy_api_shortcuts_and_target_survive_migration() {
        let mut settings = Settings::from_json(r#"{"baseUrl":"https://example.com/v1","model":"old-model","targetLanguage":"简体中文","readShortcut":"Alt+T","captureWholeInput":false}"#.as_bytes()).unwrap();
        settings.validate().unwrap();
        assert_eq!(settings.foreign_language, "简体中文");
        assert_eq!(settings.native_language, "English");
        assert_eq!(settings.read_shortcut, "Alt+T");
        assert_eq!(settings.read_output, Output::Popup);
        assert_eq!(settings.write_output, Output::Replace);
        let value = serde_json::to_value(settings).unwrap();
        assert!(value.get("targetLanguage").is_none());
        assert!(value.get("captureWholeInput").is_none());
    }
}
