use crate::{
    api::{ApiEndpoint, ApiMethod},
    config::field_value,
    models::{ApiResponse, Pagination},
    Config, Error, PublishConfig, UploadConfig,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::Utc;
use reqwest::{
    header::{HeaderMap, HeaderValue, COOKIE},
    multipart::{Form, Part},
    Client, Url,
};
use rsa::{
    pkcs1::DecodeRsaPrivateKey, pkcs1v15::SigningKey, pkcs8::DecodePrivateKey,
    traits::PublicKeyParts, RsaPrivateKey,
};
use serde_json::Value;
use sha2::Sha256;
use signature::{SignatureEncoding, Signer};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};
use tracing::info;

pub struct MpaasClient {
    http: Client,
    signing_key: SigningKey<Sha256>,
    base_url: String,
    app_id: String,
    workspace_id: String,
    tenant_id: String,
}

impl MpaasClient {
    pub async fn new(config: &Config) -> Result<Self, Error> {
        config.validate_connection()?;
        let private_key = tokio::fs::read_to_string(&config.private_key_file)
            .await
            .map_err(|e| Error::new("key", format!("cannot read privateKeyFile: {e}")))?;
        let key = parse_private_key(&private_key)?;
        let mut headers = HeaderMap::new();
        if let Some(name) = &config.login_name {
            let mut cookie = HeaderValue::from_str(&format!("loginName={name}"))
                .map_err(|_| Error::new("config", "invalid loginName cookie value"))?;
            cookie.set_sensitive(true);
            headers.insert(COOKIE, cookie);
        }
        let http = Client::builder()
            .user_agent(concat!("mpaas-openapi-rs/", env!("CARGO_PKG_VERSION")))
            .default_headers(headers)
            .timeout(Duration::from_secs(config.timeout_seconds))
            .connect_timeout(Duration::from_secs(config.timeout_seconds.min(10)))
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .build()
            .map_err(http_error)?;
        Ok(Self {
            http,
            signing_key: SigningKey::new(key),
            base_url: config.base_url.trim_end_matches('/').into(),
            app_id: config.app_id.clone(),
            workspace_id: config.workspace_id.clone(),
            tenant_id: config.tenant_id.clone(),
        })
    }

    pub async fn get_packages(&self, config: &Config) -> Result<Value, Error> {
        let mut fields = vec![
            ("h5Id", config.h5_id.clone().unwrap_or_default()),
            ("packageTypes", config.package_types.clone()),
        ];
        if let Some(page) = config.page_num {
            fields.push(("pageNum", page.to_string()));
        }
        if let Some(size) = config.page_size {
            fields.push(("pageSize", size.to_string()));
        }
        if let Some(sub_type) = &config.sub_type {
            fields.push(("subType", sub_type.clone()));
        }
        self.get("/openapi/miniprogram/getPackages", fields).await
    }

    pub async fn get_resource(&self, package_id: &str) -> Result<Value, Error> {
        self.get(
            "/openapi/nebula/getNebulaResourceById",
            vec![("id", package_id.into())],
        )
        .await
    }

    pub async fn get_app_list(&self) -> Result<Value, Error> {
        self.get("/openapi/miniprogram/getAppList", Vec::new())
            .await
    }

    pub async fn get_nebula_vhost(&self) -> Result<Value, Error> {
        self.get("/openapi/nebula/getNebulaVhost", Vec::new()).await
    }

    pub async fn get_task_detail(&self, task_id: &str) -> Result<Value, Error> {
        self.get(
            "/openapi/miniprogram/getTaskDetail",
            vec![("taskId", task_id.into())],
        )
        .await
    }

    pub async fn get_task_list(&self, package_id: &str) -> Result<Value, Error> {
        self.get(
            "/openapi/miniprogram/nebulaTaskList",
            vec![("id", package_id.into())],
        )
        .await
    }

    pub async fn upload_package(&self, upload: &UploadConfig) -> Result<Value, Error> {
        info!(h5_id = %upload.h5_id, version = %upload.h5_version, resource = %upload.resource.display(), icon = %upload.icon.display(), "preparing package upload");
        upload.validate()?;
        if upload.h5_version.trim().is_empty() {
            return Err(Error::new(
                "config",
                "upload.h5Version is required for direct upload_package calls; use run() for automatic versioning",
            ));
        }
        let mut fields = BTreeMap::from([
            ("h5Id".into(), upload.h5_id.clone()),
            ("h5Name".into(), upload.h5_name.clone()),
            ("h5Version".into(), upload.h5_version.clone()),
            ("vhost".into(), upload.vhost.clone()),
            ("clientVersionMin".into(), upload.client_version_min.clone()),
            ("platform".into(), upload.platform.clone()),
            ("packageType".into(), upload.package_type.to_string()),
            ("resourceType".into(), "4".into()),
            ("autoInstall".into(), upload.auto_install.to_string()),
            ("installType".into(), upload.install_type.to_string()),
            ("enableTabBar".into(), upload.enable_tab_bar.to_string()),
            (
                "enableOptionMenu".into(),
                upload.enable_option_menu.to_string(),
            ),
            (
                "enableKeepAlive".into(),
                upload.enable_keep_alive.to_string(),
            ),
        ]);
        if let Some(url) = &upload.main_url {
            fields.insert("mainUrl".into(), url.clone());
        }
        if let Some(version) = &upload.client_version_max {
            fields.insert("clientVersionMax".into(), version.clone());
        }
        append_extra(&mut fields, &upload.extra);
        info!(fields = ?fields.keys().collect::<Vec<_>>(), "upload form fields prepared");
        let packaged = if upload.resource.is_dir() {
            Some(crate::resource::package_directory(
                &upload.resource,
                &upload.h5_id,
            )?)
        } else {
            None
        };
        let resource_path = packaged
            .as_ref()
            .map(|(path, _)| path.as_path())
            .unwrap_or(&upload.resource);
        let resource_size = tokio::fs::metadata(resource_path)
            .await
            .map(|metadata| metadata.len())
            .unwrap_or_default();
        info!(resource = %resource_path.display(), bytes = resource_size, packaged = packaged.is_some(), "upload resource is ready");
        let result = async {
            let form = self
                .form(fields)
                .part(
                    "resourceFile",
                    file_part(resource_path, "application/zip").await?,
                )
                .part(
                    "iconFile",
                    file_part(&upload.icon, icon_mime(&upload.icon)).await?,
                );
            self.post("/openapi/miniprogram/uploadPackage", form).await
        }
        .await;
        if let Some((_, directory)) = packaged {
            crate::resource::cleanup(&directory);
            info!(temporary_directory = %directory.display(), "removed temporary upload package");
        }
        result
    }

    pub async fn create_task(
        &self,
        package_id: &str,
        publish: &PublishConfig,
    ) -> Result<Value, Error> {
        publish.validate()?;
        let mut fields = BTreeMap::from([
            ("packageId".into(), package_id.into()),
            ("id".into(), "0".into()),
            ("publishType".into(), publish.publish_type.to_string()),
        ]);
        if let Some(mode) = publish.publish_mode {
            fields.insert("publishMode".into(), mode.to_string());
        }
        if let Some(memo) = &publish.memo {
            fields.insert("memo".into(), memo.clone());
        }
        append_extra(&mut fields, &publish.extra);
        self.post("/openapi/miniprogram/createNebulaTask", self.form(fields))
            .await
    }

    pub async fn change_task_status(
        &self,
        package_id: &str,
        task_id: &str,
        status: u8,
    ) -> Result<Value, Error> {
        if status > 3 {
            return Err(Error::new("config", "taskStatus must be 0, 1, 2 or 3"));
        }
        let fields = BTreeMap::from([
            ("bizType".into(), "nebula".into()),
            ("packageId".into(), package_id.into()),
            ("taskId".into(), task_id.into()),
            ("taskStatus".into(), status.to_string()),
        ]);
        self.post("/openapi/miniprogram/changeTaskStatus", self.form(fields))
            .await
    }

    fn form(&self, fields: BTreeMap<String, String>) -> Form {
        let summary = fields
            .iter()
            .map(|(name, value)| format!("{name}={}", log_value(name, value)))
            .collect::<Vec<_>>()
            .join("&");
        info!(parameters = %summary, "multipart request fields prepared");
        let mut form = Form::new()
            .part("appId", text_part(self.app_id.clone()))
            .part("workspaceId", text_part(self.workspace_id.clone()))
            .part("tenantId", text_part(self.tenant_id.clone()));
        for (name, value) in fields {
            form = form.part(name, text_part(value));
        }
        form
    }

    fn signed_url(&self, path: &str) -> Result<(Url, String), Error> {
        let timestamp = Utc::now().timestamp().to_string();
        let content = format!(
            "appId={}&workspaceId={}&timestamp={}&url={path}",
            self.app_id, self.workspace_id, timestamp
        );
        let sign = hex::encode(self.signing_key.sign(content.as_bytes()).to_bytes());
        let mut url = Url::parse(&format!("{}{path}", self.base_url))
            .map_err(|_| Error::new("config", "invalid API URL"))?;
        url.query_pairs_mut()
            .append_pair("appId", &self.app_id)
            .append_pair("workspaceId", &self.workspace_id)
            .append_pair("tenantId", &self.tenant_id)
            .append_pair("timestamp", &timestamp)
            .append_pair("sign", &sign);
        Ok((url, sign))
    }

    async fn get(&self, path: &str, fields: Vec<(&str, String)>) -> Result<Value, Error> {
        Ok(self.get_response(path, fields).await?.data)
    }

    async fn get_response(
        &self,
        path: &str,
        fields: Vec<(&str, String)>,
    ) -> Result<ApiResponse<Value>, Error> {
        let (mut url, sign) = self.signed_url(path)?;
        let request_fields = fields
            .iter()
            .map(|(name, value)| format!("{name}={}", log_value(name, value)))
            .collect::<Vec<_>>()
            .join("&");
        url.query_pairs_mut().extend_pairs(fields);
        info!(method = "GET", path, parameters = %request_fields, "sending mPaaS request");
        let response = self.http.get(url).send().await.map_err(http_error)?;
        parse_response(response, path, &sign).await
    }

    async fn post(&self, path: &str, form: Form) -> Result<Value, Error> {
        Ok(self.post_response(path, form).await?.data)
    }

    async fn post_response(&self, path: &str, form: Form) -> Result<ApiResponse<Value>, Error> {
        let (url, sign) = self.signed_url(path)?;
        info!(method = "POST", path, "sending mPaaS request");
        let response = self
            .http
            .post(url)
            .multipart(form)
            .send()
            .await
            .map_err(http_error)?;
        parse_response(response, path, &sign).await
    }

    pub(crate) async fn send_endpoint(
        &self,
        endpoint: ApiEndpoint,
        value: Value,
        base: Option<&Path>,
    ) -> Result<ApiResponse<Value>, Error> {
        let Value::Object(mut fields) = value else {
            return Err(Error::new("config", "API parameters must be an object"));
        };
        crate::api::validate_request(endpoint, &fields)?;
        let path = endpoint.path();
        match endpoint.method() {
            ApiMethod::Get => {
                let query = fields
                    .iter()
                    .map(|(key, value)| (key.as_str(), field_value(value)))
                    .collect();
                self.get_response(path, query).await
            }
            ApiMethod::Multipart => {
                let mut attachments = Vec::new();
                for name in endpoint.file_fields() {
                    if let Some(value) = fields.remove(*name) {
                        let values = match value {
                            Value::Array(values) => values,
                            value => vec![value],
                        };
                        for value in values {
                            let mut file: PathBuf =
                                serde_json::from_value(value).map_err(|error| {
                                    Error::new(
                                        "config",
                                        format!("invalid attachment {name}: {error}"),
                                    )
                                })?;
                            if let Some(base) = base.filter(|_| file.is_relative()) {
                                file = base.join(file);
                            }
                            attachments.push((name, file));
                        }
                    }
                }
                let fields = fields
                    .into_iter()
                    .map(|(key, value)| (key, field_value(&value)))
                    .collect();
                let mut form = self.form(fields);
                for (name, file) in attachments {
                    info!(field = *name, "adding multipart attachment");
                    form = form.part(*name, file_part(&file, file_mime(&file)).await?);
                }
                self.post_response(path, form).await
            }
            ApiMethod::Json => {
                for (name, value) in [
                    ("appId", &self.app_id),
                    ("workspaceId", &self.workspace_id),
                    ("tenantId", &self.tenant_id),
                ] {
                    fields.insert(name.into(), Value::String(value.clone()));
                }
                let (url, sign) = self.signed_url(path)?;
                info!(method = "POST", path, "sending mPaaS JSON request");
                let response = self
                    .http
                    .post(url)
                    .json(&fields)
                    .send()
                    .await
                    .map_err(http_error)?;
                parse_response(response, path, &sign).await
            }
        }
    }
}

fn log_value(name: &str, value: &str) -> String {
    let name = name.to_ascii_lowercase();
    if [
        "sign",
        "signature",
        "privatekey",
        "privatekeyfile",
        "token",
        "cookie",
        "verificationcode",
        "keyids",
        "whitelistvalue",
    ]
    .iter()
    .any(|field| name == *field || name.contains(field))
    {
        "<redacted>".into()
    } else {
        value.into()
    }
}

fn parse_private_key(text: &str) -> Result<RsaPrivateKey, Error> {
    let parsed = if text.trim_start().starts_with("-----BEGIN") {
        RsaPrivateKey::from_pkcs8_pem(text)
            .or_else(|_| RsaPrivateKey::from_pkcs1_pem(text))
            .map_err(|_| {
                Error::new(
                    "key",
                    "privateKeyFile is not an unencrypted PKCS#8 or PKCS#1 RSA PEM key",
                )
            })?
    } else {
        let encoded: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let der = STANDARD
            .decode(encoded)
            .map_err(|_| Error::new("key", "privateKeyFile is not valid base64 PKCS#8"))?;
        RsaPrivateKey::from_pkcs8_der(&der)
            .map_err(|_| Error::new("key", "privateKeyFile is not a PKCS#8 RSA key"))?
    };
    if parsed.n().bits() != 2048 {
        return Err(Error::new(
            "key",
            "mPaaS requires an RSA 2048-bit private key",
        ));
    }
    Ok(parsed)
}

async fn file_part(path: &Path, mime: &str) -> Result<Part, Error> {
    Part::file(path)
        .await
        .map_err(|e| Error::new("file", format!("cannot open upload file: {e}")))?
        .mime_str(mime)
        .map_err(http_error)
}

fn icon_mime(path: &Path) -> &'static str {
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("png"))
    {
        "image/png"
    } else {
        "image/jpeg"
    }
}

fn file_mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "json" => "application/json",
        "zip" => "application/zip",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "pem" | "key" => "application/octet-stream",
        _ => "application/octet-stream",
    }
}

fn text_part(value: String) -> Part {
    Part::text(value)
        .mime_str("text/plain; charset=utf-8")
        .expect("static MIME type is valid")
}

fn append_extra(fields: &mut BTreeMap<String, String>, extra: &BTreeMap<String, Value>) {
    for (name, value) in extra {
        if !value.is_null() {
            fields.insert(name.clone(), field_value(value));
        }
    }
}

fn http_error(error: reqwest::Error) -> Error {
    Error::new(
        "http",
        format!(
            "{}; server outcome may be unknown for writes; inspect before retrying",
            error.without_url()
        ),
    )
}

async fn parse_response(
    response: reqwest::Response,
    path: &str,
    sign: &str,
) -> Result<ApiResponse<Value>, Error> {
    let status = response.status();
    let bytes = response.bytes().await.map_err(http_error)?;
    info!(path, status = status.as_u16(), "mPaaS response received");
    if !status.is_success() {
        return Err(Error::new(
            "httpStatus",
            format!(
                "mPaaS returned HTTP {}; redirects are disabled",
                status.as_u16()
            ),
        ));
    }
    let body: Value = serde_json::from_slice(&bytes)
        .map_err(|_| Error::new("response", "mPaaS response is not valid JSON"))?;
    let mut logged = body.clone();
    redact_response(&mut logged, sign);
    if matches!(
        path,
        "/openapi/upgrade/showCheckCode" | "/openapi/whitelist/queryMoreWhitelistDetail"
    ) {
        info!(path, "mPaaS response body contains sensitive data");
    } else {
        info!(path, body = %logged, "mPaaS response body");
    }
    let envelope = body
        .get("data")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("response", "mPaaS response has no data envelope"))?;
    if envelope.get("success").and_then(Value::as_bool) != Some(true) {
        let code = envelope
            .get("errorCode")
            .map(field_value)
            .unwrap_or_default()
            .replace(sign, "<redacted>");
        let message = envelope
            .get("resultMsg")
            .and_then(Value::as_str)
            .unwrap_or("missing or false data.success")
            .replace(sign, "<redacted>");
        return Err(Error::new(
            "api",
            format!("mPaaS rejected request: {message} (code: {code})"),
        ));
    }
    Ok(ApiResponse {
        data: envelope.get("data").cloned().unwrap_or(Value::Null),
        request_id: body
            .get("requestId")
            .or_else(|| envelope.get("requestId"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        pagination: Pagination {
            current_page: body.get("currentPage").and_then(Value::as_u64),
            total_count: body.get("totalCount").and_then(Value::as_u64),
            page_size: body.get("pageSize").and_then(Value::as_u64),
            has_more: body.get("hasMore").and_then(Value::as_bool),
        },
    })
}

fn redact_response(value: &mut Value, sign: &str) {
    match value {
        Value::Object(fields) => {
            for (name, field) in fields {
                if log_value(name, "") == "<redacted>" {
                    *field = Value::String("<redacted>".into());
                } else {
                    redact_response(field, sign);
                }
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| redact_response(item, sign)),
        Value::String(text) if text.contains(sign) => *text = text.replace(sign, "<redacted>"),
        _ => {}
    }
}
