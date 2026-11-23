mod client;
mod config;

pub use client::MpaasClient;
pub use config::{Config, Id, PublishConfig, UploadConfig};

use serde::Serialize;
use serde_json::Value;

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
    pub data: Value,
}

impl RunResult {
    fn new(action: &str, data: Value) -> Self {
        Self {
            ok: true,
            action: action.into(),
            package_id: None,
            task_id: None,
            task_status: None,
            data,
        }
    }
}

pub async fn run(config: &Config) -> Result<RunResult, Error> {
    config.validate()?;
    let client = MpaasClient::new(config).await?;
    let mut result = RunResult::new(&config.action, Value::Null);
    match config.action.as_str() {
        "uploadAndPublish" => return upload_and_publish(&client, config).await,
        "upload" => {
            let data = client
                .upload_package(config.upload.as_ref().unwrap())
                .await?;
            result.package_id = Some(response_id(&data, "packageId")?);
            result.data = data;
        }
        "getPackages" => result.data = client.get_packages(config).await?,
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
        _ => unreachable!("validated action"),
    }
    Ok(result)
}

async fn upload_and_publish(client: &MpaasClient, config: &Config) -> Result<RunResult, Error> {
    let uploaded = client
        .upload_package(config.upload.as_ref().unwrap())
        .await
        .map_err(|e| e.context("upload", None, None, None))?;
    let package_id =
        response_id(&uploaded, "packageId").map_err(|e| e.context("upload", None, None, None))?;
    let created = client
        .create_task(&package_id, &config.publish)
        .await
        .map_err(|e| e.context("createTask", Some(&package_id), None, None))?;
    let task_id = response_id(&created, "taskId")
        .map_err(|e| e.context("createTask", Some(&package_id), None, None))?;
    let mut detail = client
        .get_task_detail(&task_id)
        .await
        .map_err(|e| e.context("getTaskDetail", Some(&package_id), Some(&task_id), None))?;
    let status = checked_task_status(&detail, &package_id, &task_id)
        .map_err(|e| e.context("getTaskDetail", Some(&package_id), Some(&task_id), None))?;
    match status {
        0 => {
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
                "created task is neither pending nor publishing; inspect the existing task",
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
