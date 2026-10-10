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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub base_url: String,
    pub model: String,
    pub target_language: String,
    pub style: String,
    pub timeout_seconds: u64,
    pub read_shortcut: String,
    pub replace_shortcut: String,
    pub clipboard_fallback: bool,
    pub capture_whole_input: bool,
    pub use_system_proxy: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-4o-mini".into(),
            target_language: "English".into(),
            style: "natural".into(),
            timeout_seconds: 45,
            read_shortcut: "Alt+D".into(),
            replace_shortcut: "Alt+R".into(),
            clipboard_fallback: true,
            capture_whole_input: false,
            use_system_proxy: true,
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
        if !LANGUAGES.contains(&self.target_language.as_str())
            || !STYLES.contains(&self.style.as_str())
        {
            return Err("语言或翻译风格不受支持。".into());
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
        let mut settings: Self =
            serde_json::from_slice(&data).map_err(|_| "设置文件无效，请修复或删除后重启。")?;
        settings.validate()?;
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
        settings.target_language = "untrusted prompt".into();
        assert!(settings.validate().is_err());
    }
}
