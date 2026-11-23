use crate::Error;
use reqwest::Url;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    env,
    fs::File,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct Id(pub(crate) String);

impl Id {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn from_value(value: &Value) -> Option<Self> {
        let value = value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_u64().map(|id| id.to_string()))?;
        if value.is_empty()
            || !value.bytes().all(|byte| byte.is_ascii_digit())
            || value.bytes().all(|byte| byte == b'0')
        {
            return None;
        }
        Some(Self(value.trim_start_matches('0').to_owned()))
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::from_value(&Value::deserialize(deserializer)?).ok_or_else(|| {
            serde::de::Error::custom("ID must be a positive integer or digit string")
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub action: String,
    pub base_url: String,
    pub app_id: String,
    pub workspace_id: String,
    pub tenant_id: String,
    pub private_key_file: PathBuf,
    pub timeout_seconds: u64,
    pub login_name: Option<String>,
    pub upload: Option<UploadConfig>,
    pub publish: PublishConfig,
    pub h5_id: Option<String>,
    pub package_id: Option<Id>,
    pub task_id: Option<Id>,
    pub task_status: Option<u8>,
    pub package_types: String,
    pub page_num: Option<u64>,
    pub page_size: Option<u64>,
    pub sub_type: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            action: "uploadAndPublish".into(),
            base_url: String::new(),
            app_id: String::new(),
            workspace_id: String::new(),
            tenant_id: String::new(),
            private_key_file: PathBuf::new(),
            timeout_seconds: 60,
            login_name: None,
            upload: None,
            publish: PublishConfig::default(),
            h5_id: None,
            package_id: None,
            task_id: None,
            task_status: None,
            package_types: "1".into(),
            page_num: None,
            page_size: None,
            sub_type: None,
        }
    }
}

impl Config {
    pub async fn from_file(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let raw = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| Error::new("config", format!("cannot read configuration: {e}")))?;
        let mut value: Value = serde_json::from_str(&raw)
            .map_err(|e| Error::new("config", format!("invalid JSON: {e}")))?;
        expand_environment(&mut value)?;
        let mut config: Self = serde_json::from_value(value)
            .map_err(|e| Error::new("config", format!("invalid configuration: {e}")))?;
        let parent = path.parent().unwrap_or(Path::new("."));
        resolve_path(&mut config.private_key_file, parent);
        if let Some(upload) = &mut config.upload {
            resolve_path(&mut upload.resource, parent);
            resolve_path(&mut upload.icon, parent);
        }
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), Error> {
        if !matches!(
            self.action.as_str(),
            "uploadAndPublish"
                | "upload"
                | "getPackages"
                | "getResource"
                | "createTask"
                | "getTaskDetail"
                | "getTaskList"
                | "changeTaskStatus"
        ) {
            return Err(Error::new(
                "config",
                format!("unsupported action: {}", self.action),
            ));
        }
        for (name, value) in [
            ("baseUrl", self.base_url.as_str()),
            ("appId", &self.app_id),
            ("workspaceId", &self.workspace_id),
            ("tenantId", &self.tenant_id),
        ] {
            required(name, value)?;
        }
        let base = Url::parse(&self.base_url)
            .map_err(|_| Error::new("config", "baseUrl must be an absolute HTTP(S) URL"))?;
        if !matches!(base.scheme(), "http" | "https")
            || base.host_str().is_none()
            || base.path() != "/"
            || base.query().is_some()
            || base.fragment().is_some()
            || !base.username().is_empty()
            || base.password().is_some()
        {
            return Err(Error::new(
                "config",
                "baseUrl must be an HTTP(S) origin without a path, credentials, query or fragment",
            ));
        }
        if self.private_key_file.as_os_str().is_empty() {
            return Err(Error::new("config", "privateKeyFile is required"));
        }
        if self.timeout_seconds == 0 {
            return Err(Error::new(
                "config",
                "timeoutSeconds must be greater than zero",
            ));
        }
        if let Some(name) = &self.login_name {
            if name.is_empty()
                || !name.is_ascii()
                || name
                    .bytes()
                    .any(|b| b <= 32 || matches!(b, b';' | b',' | b'"' | b'\\'))
            {
                return Err(Error::new(
                    "config",
                    "loginName must be a nonempty ASCII cookie value",
                ));
            }
        }
        if self.page_num.is_some() != self.page_size.is_some()
            || self.page_num == Some(0)
            || self.page_size == Some(0)
        {
            return Err(Error::new(
                "config",
                "pageNum and pageSize must be positive and provided together",
            ));
        }
        match self.action.as_str() {
            "upload" | "uploadAndPublish" => {
                self.upload
                    .as_ref()
                    .ok_or_else(|| Error::new("config", "upload configuration is required"))?
                    .validate()?;
                if self.action == "uploadAndPublish" {
                    if self.upload.as_ref().unwrap().package_type != 1 {
                        return Err(Error::new(
                            "config",
                            "uploadAndPublish requires packageType=1",
                        ));
                    }
                    self.publish.validate()?;
                }
            }
            "getPackages" => {
                required("h5Id", self.h5_id.as_deref().unwrap_or(""))?;
                required("packageTypes", &self.package_types)?;
            }
            "createTask" => {
                self.require_package()?;
                self.publish.validate()?;
            }
            "getResource" | "getTaskList" => self.require_package()?,
            "getTaskDetail" => self.require_task()?,
            "changeTaskStatus" => {
                self.require_package()?;
                self.require_task()?;
                if !matches!(self.task_status, Some(0..=3)) {
                    return Err(Error::new("config", "taskStatus must be 0, 1, 2 or 3"));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn require_package(&self) -> Result<(), Error> {
        self.package_id
            .as_ref()
            .map(|_| ())
            .ok_or_else(|| Error::new("config", "packageId is required"))
    }

    fn require_task(&self) -> Result<(), Error> {
        self.task_id
            .as_ref()
            .map(|_| ())
            .ok_or_else(|| Error::new("config", "taskId is required"))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UploadConfig {
    pub resource: PathBuf,
    pub icon: PathBuf,
    pub h5_id: String,
    pub h5_name: String,
    pub h5_version: String,
    pub main_url: Option<String>,
    pub vhost: String,
    pub client_version_min: String,
    pub client_version_max: Option<String>,
    pub platform: String,
    pub package_type: u8,
    pub resource_type: u8,
    pub auto_install: u8,
    pub install_type: u8,
    pub enable_tab_bar: u8,
    pub enable_option_menu: u8,
    pub enable_keep_alive: u8,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            resource: PathBuf::new(),
            icon: PathBuf::new(),
            h5_id: String::new(),
            h5_name: String::new(),
            h5_version: String::new(),
            main_url: None,
            vhost: String::new(),
            client_version_min: String::new(),
            client_version_max: None,
            platform: "all".into(),
            package_type: 1,
            resource_type: 4,
            auto_install: 0,
            install_type: 0,
            enable_tab_bar: 0,
            enable_option_menu: 1,
            enable_keep_alive: 0,
            extra: BTreeMap::new(),
        }
    }
}

impl UploadConfig {
    pub fn validate(&self) -> Result<(), Error> {
        for (name, value) in [
            ("upload.h5Id", &self.h5_id),
            ("upload.h5Name", &self.h5_name),
            ("upload.h5Version", &self.h5_version),
            ("upload.vhost", &self.vhost),
            ("upload.clientVersionMin", &self.client_version_min),
            ("upload.platform", &self.platform),
        ] {
            required(name, value)?;
        }
        if self.h5_id.len() != 16 || !self.h5_id.bytes().all(|b| b.is_ascii_digit()) {
            return Err(Error::new("config", "upload.h5Id must contain 16 digits"));
        }
        if self.resource_type != 4 || !(1..=4).contains(&self.package_type) {
            return Err(Error::new(
                "config",
                "resourceType must be 4 and packageType must be 1..4",
            ));
        }
        for (name, value) in [
            ("autoInstall", self.auto_install),
            ("installType", self.install_type),
            ("enableTabBar", self.enable_tab_bar),
            ("enableOptionMenu", self.enable_option_menu),
            ("enableKeepAlive", self.enable_keep_alive),
        ] {
            if value > 1 {
                return Err(Error::new(
                    "config",
                    format!("upload.{name} must be 0 or 1"),
                ));
            }
        }
        validate_extra(
            &self.extra,
            &[
                "appId",
                "workspaceId",
                "tenantId",
                "resourceFile",
                "iconFile",
                "sign",
                "timestamp",
            ],
        )?;
        validate_file(&self.icon, "upload.icon")?;
        let extension = self
            .icon
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(extension.as_str(), "png" | "jpg" | "jpeg") {
            return Err(Error::new("config", "upload.icon must be PNG or JPG"));
        }
        validate_zip(&self.resource, &self.h5_id)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PublishConfig {
    pub publish_type: u8,
    pub publish_mode: Option<u8>,
    pub memo: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for PublishConfig {
    fn default() -> Self {
        Self {
            publish_type: 3,
            publish_mode: None,
            memo: None,
            extra: BTreeMap::new(),
        }
    }
}

impl PublishConfig {
    pub fn validate(&self) -> Result<(), Error> {
        validate_extra(
            &self.extra,
            &[
                "appId",
                "workspaceId",
                "tenantId",
                "packageId",
                "id",
                "sign",
                "timestamp",
            ],
        )?;
        match (self.publish_type, self.publish_mode) {
            (3, None) => Ok(()),
            (3, Some(_)) => Err(Error::new(
                "config",
                "publishMode must be omitted for formal publishType=3",
            )),
            (2, Some(1)) => self.require_extra("whitelistIds"),
            (2, Some(2)) => {
                self.require_extra("greyEndtimeData")?;
                let count = self
                    .extra
                    .get("greyNum")
                    .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()));
                if count.is_none_or(|count| count == 0) {
                    return Err(Error::new(
                        "config",
                        "publish.greyNum must be positive for time-window publishing",
                    ));
                }
                Ok(())
            }
            (2, Some(5)) => Ok(()),
            _ => Err(Error::new(
                "config",
                "publishType must be 3 (formal) or 2 with publishMode 1, 2 or 5",
            )),
        }
    }

    fn require_extra(&self, field: &str) -> Result<(), Error> {
        let value = self.extra.get(field).map(field_value).unwrap_or_default();
        required(&format!("publish.{field}"), &value)
    }
}

pub(crate) fn field_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => String::new(),
        value => value.to_string(),
    }
}

fn required(name: &str, value: &str) -> Result<(), Error> {
    if value.trim().is_empty() {
        Err(Error::new("config", format!("{name} is required")))
    } else {
        Ok(())
    }
}

fn validate_extra(extra: &BTreeMap<String, Value>, reserved: &[&str]) -> Result<(), Error> {
    if let Some(name) = extra.keys().find(|name| reserved.contains(&name.as_str())) {
        return Err(Error::new(
            "config",
            format!("{name} is controlled by the client and cannot be overridden"),
        ));
    }
    Ok(())
}

fn resolve_path(path: &mut PathBuf, parent: &Path) {
    if !path.as_os_str().is_empty() && path.is_relative() {
        *path = parent.join(&*path);
    }
}

fn validate_file(path: &Path, name: &str) -> Result<(), Error> {
    let metadata = path
        .metadata()
        .map_err(|e| Error::new("config", format!("cannot read {name}: {e}")))?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(Error::new(
            "config",
            format!("{name} must be a nonempty regular file"),
        ));
    }
    Ok(())
}

fn validate_zip(path: &Path, h5_id: &str) -> Result<(), Error> {
    if path.file_name().and_then(|s| s.to_str()) != Some(format!("{h5_id}.zip").as_str()) {
        return Err(Error::new(
            "config",
            "resource must be named <h5Id>.zip; AMR cannot be uploaded directly",
        ));
    }
    validate_file(path, "upload.resource")?;
    let file =
        File::open(path).map_err(|e| Error::new("config", format!("cannot open ZIP: {e}")))?;
    let archive = zip::ZipArchive::new(file)
        .map_err(|_| Error::new("config", "upload.resource is not a valid ZIP archive"))?;
    let prefix = format!("{h5_id}/");
    let mut has_file = false;
    for name in archive.file_names() {
        if !name.starts_with(&prefix)
            || name.contains('\\')
            || name.split('/').any(|part| matches!(part, "." | ".."))
        {
            return Err(Error::new(
                "config",
                "ZIP entries must all be inside the <h5Id>/ directory",
            ));
        }
        has_file |= !name.ends_with('/');
    }
    if !has_file {
        return Err(Error::new(
            "config",
            "ZIP must contain files inside the <h5Id>/ directory",
        ));
    }
    Ok(())
}

fn expand_environment(value: &mut Value) -> Result<(), Error> {
    match value {
        Value::String(text) => {
            let mut result = String::new();
            let mut rest = text.as_str();
            while let Some(start) = rest.find("${") {
                result.push_str(&rest[..start]);
                let placeholder = &rest[start + 2..];
                let end = placeholder
                    .find('}')
                    .ok_or_else(|| Error::new("config", "unterminated environment placeholder"))?;
                let name = &placeholder[..end];
                if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                {
                    return Err(Error::new("config", "invalid environment variable name"));
                }
                result.push_str(&env::var(name).map_err(|_| {
                    Error::new("config", format!("environment variable {name} is required"))
                })?);
                rest = &placeholder[end + 1..];
            }
            result.push_str(rest);
            *text = result;
        }
        Value::Array(values) => {
            for value in values {
                expand_environment(value)?;
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                expand_environment(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}
