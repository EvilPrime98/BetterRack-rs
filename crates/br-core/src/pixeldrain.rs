//! `PixelDrainModel`: resolve a masked PixelDrain link (file or list) into direct download links.

use crate::rotating_fetch::RotatingFetch;
use regex::Regex;
use serde_json::Value;

pub const HOST: &str = "pixeldrain.com";

#[derive(Debug, Clone, PartialEq)]
pub struct PixelDrainFile {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub mime_type: String,
    pub can_download: bool,
    /// A direct URL with the download flag set; it streams the file bytes.
    pub download_link: String,
}

#[derive(Debug, PartialEq)]
enum Ref {
    File(String),
    List(String),
}

pub type PdResult<T> = std::result::Result<T, String>;

pub struct PixelDrain {
    api_key: Option<String>,
    fetch: RotatingFetch,
    api: String,
    file_re: Regex,
    list_re: Regex,
    bare_id_re: Regex,
}

impl PixelDrain {
    pub fn new(fetch: RotatingFetch) -> Self {
        Self::for_host(fetch, HOST, "https")
    }

    /// Point at another host (`host` may include a port); used by tests with a local mock.
    pub fn for_host(fetch: RotatingFetch, host: &str, scheme: &str) -> Self {
        let h = regex::escape(host);
        Self {
            api_key: None,
            fetch,
            api: format!("{scheme}://{host}/api"),
            file_re: Regex::new(&format!("(?i){h}/(?:u|d|api/file)/([a-zA-Z0-9]+)")).expect("file regex"),
            list_re: Regex::new(&format!("(?i){h}/(?:l|api/list)/([a-zA-Z0-9]+)")).expect("list regex"),
            bare_id_re: Regex::new("^[a-zA-Z0-9]+$").expect("id regex"),
        }
    }

    pub fn with_api_key(mut self, key: Option<String>) -> Self {
        self.api_key = key;
        self
    }

    fn parse_ref(&self, input: &str) -> PdResult<Ref> {
        let t = input.trim();
        if let Some(c) = self.list_re.captures(t) {
            return Ok(Ref::List(c[1].to_string()));
        }
        if let Some(c) = self.file_re.captures(t) {
            return Ok(Ref::File(c[1].to_string()));
        }
        if self.bare_id_re.is_match(t) {
            return Ok(Ref::File(t.to_string()));
        }
        Err(format!("Not a PixelDrain URL: {input}"))
    }

    fn direct_download_link(&self, id: &str) -> String {
        format!("{}/file/{id}?download", self.api)
    }

    fn auth_headers(&self) -> Vec<(String, String)> {
        use base64::Engine;
        match &self.api_key {
            Some(k) if !k.is_empty() => {
                let token = base64::engine::general_purpose::STANDARD.encode(format!(":{k}"));
                vec![("authorization".to_string(), format!("Basic {token}"))]
            }
            _ => vec![],
        }
    }

    async fn get_json(&self, url: &str) -> PdResult<(u16, Value)> {
        let auth = self.auth_headers();
        let extra: Vec<(&str, &str)> = auth.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let res = self.fetch.fetch(url, &extra, None).await.map_err(|e| e.to_string())?;
        let status = res.status().as_u16();
        if !res.status().is_success() {
            return Ok((status, Value::Null));
        }
        let body = res.json::<Value>().await.map_err(|e| e.to_string())?;
        Ok((status, body))
    }

    fn file_from(&self, v: &Value) -> PixelDrainFile {
        let id = v["id"].as_str().unwrap_or_default().to_string();
        PixelDrainFile {
            download_link: self.direct_download_link(&id),
            name: v["name"].as_str().unwrap_or_default().to_string(),
            size: v["size"].as_u64().unwrap_or(0),
            mime_type: v["mime_type"].as_str().unwrap_or_default().to_string(),
            can_download: v["can_download"].as_bool().unwrap_or(false),
            id,
        }
    }

    pub async fn get_file_info(&self, id_or_url: &str) -> PdResult<PixelDrainFile> {
        let (Ref::File(id) | Ref::List(id)) = self.parse_ref(id_or_url)?;
        let (status, body) = self.get_json(&format!("{}/file/{id}/info", self.api)).await?;
        if !(200..300).contains(&status) {
            return Err(format!("PixelDrain info failed for {id}: HTTP {status}"));
        }
        if body["success"].as_bool() != Some(true) {
            return Err(format!("PixelDrain: {}", body["message"].as_str().unwrap_or("file unavailable")));
        }
        Ok(self.file_from(&body))
    }

    pub async fn get_list(&self, id_or_url: &str) -> PdResult<Vec<PixelDrainFile>> {
        let (Ref::File(id) | Ref::List(id)) = self.parse_ref(id_or_url)?;
        let (status, body) = self.get_json(&format!("{}/list/{id}", self.api)).await?;
        if !(200..300).contains(&status) {
            return Err(format!("PixelDrain list failed for {id}: HTTP {status}"));
        }
        if body["success"].as_bool() != Some(true) {
            return Err(format!("PixelDrain: {}", body["message"].as_str().unwrap_or("list unavailable")));
        }
        Ok(body["files"].as_array().map(|a| a.iter().map(|f| self.file_from(f)).collect()).unwrap_or_default())
    }

    /// Follow the redirect chain of a masked link to its real PixelDrain URL.
    async fn unhash_link(&self, url: &str) -> PdResult<String> {
        let res = self.fetch.fetch(url, &[], None).await.map_err(|e| e.to_string())?;
        Ok(res.url().to_string())
    }

    pub async fn resolve(&self, id_or_url: &str) -> PdResult<Vec<PixelDrainFile>> {
        let true_url = self.unhash_link(id_or_url).await?;
        match self.parse_ref(&true_url)? {
            Ref::List(id) => self.get_list(&id).await,
            Ref::File(id) => Ok(vec![self.get_file_info(&id).await?]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pd() -> PixelDrain {
        PixelDrain::new(RotatingFetch::new(reqwest::Client::new()))
    }

    #[test]
    fn parses_refs() {
        let p = pd();
        assert_eq!(p.parse_ref("https://pixeldrain.com/u/aB3xK9m2").unwrap(), Ref::File("aB3xK9m2".into()));
        assert_eq!(p.parse_ref("https://pixeldrain.com/api/file/zz9?download").unwrap(), Ref::File("zz9".into()));
        assert_eq!(p.parse_ref("https://pixeldrain.com/l/LIST1").unwrap(), Ref::List("LIST1".into()));
        assert_eq!(p.parse_ref(" bareId1 ").unwrap(), Ref::File("bareId1".into()));
        assert!(p.parse_ref("https://example.com/x").is_err());
    }

    #[test]
    fn direct_link_has_download_flag() {
        assert_eq!(pd().direct_download_link("abc"), "https://pixeldrain.com/api/file/abc?download");
    }
}
