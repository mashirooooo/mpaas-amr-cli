mod client;
mod config;
mod defaults;
mod resource;
mod version;

pub mod api;
pub mod models;

pub use api::{ApiEndpoint, ApiMethod};
pub use client::MpaasClient;
pub use config::{ApiConfig, Config, Id, PublishConfig, UploadConfig};

use serde::Serialize;
use serde_json::{Map, Value};
use tracing::info;

#[derive(Debug, thiserror::Error, Serialize)]
#[error("{message}")]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub kind: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_status: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h5_version: Option<String>,
}

impl Error {
    pub(crate) fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            phase: None,
            package_id: None,
            task_id: None,
            task_status: None,
            h5_version: None,
        }
    }

    fn context(
        mut self,
        phase: &str,
        package: Option<&str>,
        task: Option<&str>,
        status: Option<u8>,
    ) -> Self {
        self.phase = Some(phase.to_owned());
        self.package_id = package.map(str::to_owned);
        self.task_id = task.map(str::to_owned);
        self.task_status = status;
        self
    }

    fn with_version(mut self, version: &str) -> Self {
        self.h5_version = Some(version.to_owned());
        self
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    pub ok: bool,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_status: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h5_version: Option<String>,
    pub data: Value,
    /// 服务端请求跟踪 ID，通用 API 调用时提供。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// 通用 API 调用返回的分页信息。
    #[serde(flatten)]
    pub pagination: models::Pagination,
}

impl RunResult {
    fn new(action: &str, data: Value) -> Self {
        Self {
            ok: true,
            action: action.into(),
            package_id: None,
            task_id: None,
            task_status: None,
            h5_version: None,
            data,
            request_id: None,
            pagination: models::Pagination::default(),
        }
    }
}

pub async fn run(config: &Config) -> Result<RunResult, Error> {
    info!(action = %config.action, "validating configuration");
    config.validate()?;
    info!(action = %config.action, "configuration validated; creating API client");
    let client = MpaasClient::new(config).await?;
    let mut result = RunResult::new(&config.action, Value::Null);
    match config.action.as_str() {
        "upload" | "uploadAndPublish" => return upload_package(&client, config).await,
        "publish" => {
            return publish_package(
                &client,
                config,
                config.package_id.as_ref().unwrap().as_str().to_owned(),
                config.task_id.as_ref().map(|id| id.as_str().to_owned()),
            )
            .await
        }
        "getPackages" => result.data = client.get_packages(config).await?,
        "getNextVersion" => {
            let plan = version::next_version(
                &client,
                config,
                config.h5_id.as_deref().unwrap(),
                &config.package_types,
            )
            .await?;
            result.data = serde_json::json!({
                "latestVersion": plan.latest_version,
                "nextVersion": plan.next_version,
            });
        }
        "getUploadDefaults" => {
            let h5_id = config.h5_id.as_deref().unwrap();
            let plan = version::next_version(&client, config, h5_id, &config.package_types)
                .await
                .map_err(|e| e.context("getNextVersion", None, None, None))?;
            let upload_defaults =
                upload_defaults(&client, &plan, h5_id, None)
                    .await
                    .map_err(|e| {
                        e.context("getUploadDefaults", None, None, None)
                            .with_version(&plan.next_version)
                    })?;
            result.data = serde_json::json!({
                "latestVersion": plan.latest_version,
                "nextVersion": plan.next_version,
                "uploadDefaults": upload_defaults,
            });
        }
        "getResource" => {
            let package_id = config.package_id.as_ref().unwrap().as_str();
            result.data = client.get_resource(package_id).await?;
            result.package_id = Some(package_id.into());
        }
        "createTask" => {
            let package_id = config.package_id.as_ref().unwrap().as_str();
            let data = client
                .create_task(package_id, &config.publish)
                .await
                .map_err(|e| e.context("createTask", Some(package_id), None, None))?;
            result.task_id = Some(
                response_id(&data, "taskId")
                    .map_err(|e| e.context("createTask", Some(package_id), None, None))?,
            );
            result.package_id = Some(package_id.into());
            result.data = data;
        }
        "getTaskDetail" => {
            let task_id = config.task_id.as_ref().unwrap().as_str();
            result.data = client.get_task_detail(task_id).await?;
            result.task_id = Some(task_id.into());
            result.task_status = task_status(&result.data).ok();
        }
        "getTaskList" => {
            let package_id = config.package_id.as_ref().unwrap().as_str();
            result.data = client.get_task_list(package_id).await?;
            result.package_id = Some(package_id.into());
        }
        "changeTaskStatus" => {
            let package_id = config.package_id.as_ref().unwrap().as_str();
            let task_id = config.task_id.as_ref().unwrap().as_str();
            let status = config.task_status.unwrap();
            result.data = client
                .change_task_status(package_id, task_id, status)
                .await
                .map_err(|e| {
                    e.context("changeTaskStatus", Some(package_id), Some(task_id), None)
                })?;
            result.package_id = Some(package_id.into());
            result.task_id = Some(task_id.into());
            result.task_status = Some(status);
        }
        "api" => {
            let api = config.api.as_ref().unwrap();
            let response = client
                .call_config(
                    api.endpoint()?,
                    Value::Object(api.params.clone()),
                    api.base_directory.as_deref(),
                )
                .await?;
            result.data = response.data;
            result.request_id = response.request_id;
            result.pagination = response.pagination;
        }
        _ => unreachable!("validated action"),
    }
    Ok(result)
}

async fn upload_package(client: &MpaasClient, config: &Config) -> Result<RunResult, Error> {
    info!("starting upload workflow");
    let mut upload = config.upload.as_ref().unwrap().clone();
    info!(h5_id = %upload.h5_id, "querying backend versions");
    let plan = version::next_version(
        client,
        config,
        &upload.h5_id,
        &upload.package_type.to_string(),
    )
    .await
    .map_err(|e| e.context("getNextVersion", None, None, None))?;
    if !upload.h5_version.is_empty() && upload.h5_version != plan.next_version {
        return Err(Error::new(
            "versionConflict",
            format!(
                "upload.h5Version {} differs from the next server version {}; rebuild or inspect the server before retrying",
                upload.h5_version, plan.next_version
            ),
        )
        .context("getNextVersion", None, None, None)
        .with_version(&plan.next_version));
    }
    upload.h5_version = plan.next_version.clone();
    info!(latest_version = ?plan.latest_version, next_version = %plan.next_version, "backend version plan calculated");
    let inherited = upload_defaults(client, &plan, &upload.h5_id, Some(&upload))
        .await
        .map_err(|e| {
            e.context("getUploadDefaults", None, None, None)
                .with_version(&upload.h5_version)
        })?;
    upload = merge_upload_defaults(&upload, inherited).map_err(|e| {
        e.context("prepareUpload", None, None, None)
            .with_version(&upload.h5_version)
    })?;
    info!(h5_name = %upload.h5_name, vhost = %upload.vhost, main_url = ?upload.main_url, "upload defaults prepared");
    upload.validate().map_err(|e| {
        e.context("prepareUpload", None, None, None)
            .with_version(&upload.h5_version)
    })?;
    info!(version = %upload.h5_version, "upload settings validated; sending resource");
    let uploaded = client.upload_package(&upload).await.map_err(|e| {
        e.context("upload", None, None, None)
            .with_version(&upload.h5_version)
    })?;
    let package_id = response_id(&uploaded, "packageId").map_err(|e| {
        e.context("upload", None, None, None)
            .with_version(&upload.h5_version)
    })?;
    info!(package_id = %package_id, version = %upload.h5_version, "resource upload completed");
    let mut result = if config.action == "uploadAndPublish" {
        publish_package(client, config, package_id, None)
            .await
            .map_err(|e| e.with_version(&upload.h5_version))?
    } else {
        let mut result = RunResult::new(&config.action, uploaded);
        result.package_id = Some(package_id);
        result
    };
    result.h5_version = Some(upload.h5_version);
    Ok(result)
}

fn resource_id(resource: &Value) -> Result<Option<String>, Error> {
    let mut result = None;
    for field in ["id", "packageId"] {
        if let Some(id) = resource.get(field).and_then(Id::from_value) {
            if result
                .as_ref()
                .is_some_and(|previous| previous != id.as_str())
            {
                return Err(Error::new(
                    "response",
                    "latest resource id and packageId differ",
                ));
            }
            result = Some(id.as_str().to_owned());
        }
    }
    Ok(result)
}

fn check_resource_detail(
    detail: &Value,
    package_id: &str,
    h5_id: &str,
    version: &str,
) -> Result<(), Error> {
    if !detail.is_object() {
        return Err(Error::new("response", "resource detail must be an object"));
    }
    for field in ["id", "packageId"] {
        if let Some(value) = detail.get(field).filter(|value| !value.is_null()) {
            if Id::from_value(value).as_ref().map(Id::as_str) != Some(package_id) {
                return Err(Error::new(
                    "response",
                    format!("resource detail {field} does not match the latest resource"),
                ));
            }
        }
    }
    for (field, expected) in [
        ("h5Id", h5_id),
        ("h5Version", version),
        ("version", version),
        ("packageVersion", version),
    ] {
        if let Some(value) = detail.get(field).filter(|value| !value.is_null()) {
            if value.as_str() != Some(expected) {
                return Err(Error::new(
                    "response",
                    format!("resource detail {field} does not match the latest resource"),
                ));
            }
        }
    }
    Ok(())
}

fn missing_text(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .is_none_or(|value| value.trim().is_empty())
}

fn merge_default_layers(
    mut previous: Map<String, Value>,
    mut current: Map<String, Value>,
) -> Result<Map<String, Value>, Error> {
    if let Some(extend_info) = current.remove("extendInfo") {
        let empty = serde_json::json!({});
        let merged = defaults::merge_extend_info(
            previous.get("extendInfo").unwrap_or(&empty),
            &extend_info,
        )?;
        previous.insert("extendInfo".into(), merged);
    }
    previous.extend(current);
    Ok(previous)
}

async fn upload_defaults(
    client: &MpaasClient,
    plan: &version::VersionPlan,
    h5_id: &str,
    explicit: Option<&UploadConfig>,
) -> Result<Map<String, Value>, Error> {
    let mut inherited = Map::new();
    if let Some(resource) = &plan.latest_resource {
        inherited = defaults::resource_defaults(resource)?;
        if let Some(package_id) = resource_id(resource)? {
            let detail = client.get_resource(&package_id).await?;
            check_resource_detail(
                &detail,
                &package_id,
                h5_id,
                plan.latest_version.as_deref().unwrap(),
            )?;
            inherited = merge_default_layers(inherited, defaults::resource_defaults(&detail)?)?;
        }
    }
    let needs_app = ["h5Name", "vhost"].iter().any(|field| {
        !explicit.is_some_and(|upload| upload.provided_fields.contains(*field))
            && missing_text(inherited.get(*field))
    });
    if needs_app {
        let response = client.get_app_list().await?;
        let apps = response
            .as_array()
            .ok_or_else(|| Error::new("response", "app list must be an array"))?;
        let mut matches = apps
            .iter()
            .filter(|app| app.get("h5Id").and_then(Value::as_str) == Some(h5_id));
        if let Some(app) = matches.next() {
            if matches.next().is_some() {
                return Err(Error::new(
                    "response",
                    "app list contains duplicate h5Id entries",
                ));
            }
            for field in ["h5Name", "vhost"] {
                if missing_text(inherited.get(field)) {
                    inherited.remove(field);
                }
            }
            inherited = merge_default_layers(defaults::app_defaults(app)?, inherited)?;
        }
    }
    if missing_text(inherited.get("vhost"))
        && !explicit.is_some_and(|upload| upload.provided_fields.contains("vhost"))
    {
        info!("resource and app metadata have no vhost; querying nebula vhost");
        let response = client.get_nebula_vhost().await?;
        let base_vhost = response
            .as_str()
            .or_else(|| response.get("vhost").and_then(Value::as_str))
            .or_else(|| response.get("host").and_then(Value::as_str))
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| Error::new("response", "nebula vhost response has no vhost"))?;
        let vhost = format_vhost(h5_id, base_vhost);
        info!(base_vhost, generated_vhost = %vhost, "generated mini-program vhost");
        inherited.insert("vhost".into(), Value::String(vhost));
    }
    Ok(inherited)
}

fn format_vhost(h5_id: &str, base_vhost: &str) -> String {
    let base_vhost = base_vhost.trim().trim_matches('.');
    if base_vhost.is_empty() {
        h5_id.to_owned()
    } else if base_vhost == h5_id || base_vhost.starts_with(&format!("{h5_id}.")) {
        base_vhost.to_owned()
    } else {
        format!("{h5_id}.{base_vhost}")
    }
}

fn merge_upload_defaults(
    upload: &UploadConfig,
    mut inherited: Map<String, Value>,
) -> Result<UploadConfig, Error> {
    let mut value = serde_json::to_value(upload)
        .map_err(|_| Error::new("config", "cannot read upload settings"))?;
    let fields = value.as_object_mut().unwrap();
    let previous_extend = inherited.remove("extendInfo");
    for (field, default) in inherited {
        if !upload.provided_fields.contains(&field) {
            fields.insert(field, default);
        }
    }
    if upload.provided_fields.contains("extendInfo") {
        let empty = serde_json::json!({});
        let merged = defaults::merge_extend_info(
            previous_extend.as_ref().unwrap_or(&empty),
            fields.get("extendInfo").unwrap(),
        )?;
        fields.insert("extendInfo".into(), merged);
    } else if let Some(previous) = previous_extend {
        fields.insert("extendInfo".into(), previous);
    }
    serde_json::from_value(value)
        .map_err(|e| Error::new("config", format!("invalid inherited upload settings: {e}")))
}

async fn publish_package(
    client: &MpaasClient,
    config: &Config,
    package_id: String,
    task_id: Option<String>,
) -> Result<RunResult, Error> {
    info!(package_id = %package_id, task_id = ?task_id, "starting publish workflow");
    let task_id = match task_id {
        Some(task_id) => task_id,
        None => {
            info!(package_id = %package_id, "creating publish task");
            let created = client
                .create_task(&package_id, &config.publish)
                .await
                .map_err(|e| e.context("createTask", Some(&package_id), None, None))?;
            response_id(&created, "taskId")
                .map_err(|e| e.context("createTask", Some(&package_id), None, None))?
        }
    };
    info!(package_id = %package_id, task_id = %task_id, "publish task is available; reading task status");
    let mut detail = client
        .get_task_detail(&task_id)
        .await
        .map_err(|e| e.context("getTaskDetail", Some(&package_id), Some(&task_id), None))?;
    let status = checked_task_status(&detail, &package_id, &task_id)
        .map_err(|e| e.context("getTaskDetail", Some(&package_id), Some(&task_id), None))?;
    info!(package_id = %package_id, task_id = %task_id, task_status = status, "publish task status received");
    match status {
        0 => {
            info!(package_id = %package_id, task_id = %task_id, "starting publish task");
            client
                .change_task_status(&package_id, &task_id, 1)
                .await
                .map_err(|e| e.context("startTask", Some(&package_id), Some(&task_id), Some(0)))?;
            detail = client
                .get_task_detail(&task_id)
                .await
                .map_err(|e| e.context("confirmTask", Some(&package_id), Some(&task_id), None))?;
            let confirmed = checked_task_status(&detail, &package_id, &task_id)
                .map_err(|e| e.context("confirmTask", Some(&package_id), Some(&task_id), None))?;
            info!(package_id = %package_id, task_id = %task_id, task_status = confirmed, "publish task status confirmed");
            if confirmed != 1 {
                return Err(Error::new(
                    "taskState",
                    "task start was not confirmed; inspect the existing task before retrying",
                )
                .context(
                    "confirmTask",
                    Some(&package_id),
                    Some(&task_id),
                    Some(confirmed),
                ));
            }
        }
        1 => {}
        _ => {
            return Err(Error::new(
                "taskState",
                "task is neither pending nor publishing; inspect the existing task",
            )
            .context(
                "getTaskDetail",
                Some(&package_id),
                Some(&task_id),
                Some(status),
            ))
        }
    }
    let mut result = RunResult::new(&config.action, detail);
    result.package_id = Some(package_id);
    result.task_id = Some(task_id);
    result.task_status = Some(1);
    Ok(result)
}

fn response_id(value: &Value, name: &str) -> Result<String, Error> {
    let candidate = value.get(name).or_else(|| value.get("id")).unwrap_or(value);
    Id::from_value(candidate).map(|id| id.0).ok_or_else(|| {
        Error::new(
            "response",
            format!("successful response has no valid {name}; inspect the server before retrying"),
        )
    })
}

fn task_status(detail: &Value) -> Result<u8, Error> {
    let value = detail
        .get("taskStatus")
        .ok_or_else(|| Error::new("response", "task detail has no taskStatus"))?;
    let status = value.as_u64().or_else(|| value.as_str()?.parse().ok());
    status
        .filter(|status| *status <= 3)
        .map(|status| status as u8)
        .ok_or_else(|| Error::new("response", "task detail has an invalid taskStatus"))
}

fn checked_task_status(detail: &Value, package_id: &str, task_id: &str) -> Result<u8, Error> {
    for (field, expected) in [("packageId", package_id), ("id", task_id)] {
        if let Some(value) = detail.get(field) {
            if Id::from_value(value).as_ref().map(Id::as_str) != Some(expected) {
                return Err(Error::new(
                    "response",
                    format!("task detail {field} does not match the requested resource"),
                ));
            }
        }
    }
    task_status(detail)
}

#[cfg(test)]
mod tests;
