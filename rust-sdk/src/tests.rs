use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::rngs::OsRng;
use rsa::{
    pkcs1::EncodeRsaPrivateKey,
    pkcs1v15::{Signature, VerifyingKey},
    pkcs8::{EncodePrivateKey, LineEnding},
    RsaPrivateKey, RsaPublicKey,
};
use serde_json::json;
use signature::Verifier;
use std::{collections::BTreeMap, io::Write, path::Path, sync::OnceLock};
use tempfile::TempDir;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, Request, ResponseTemplate,
};

const APP: &str = "0000000000000012";

fn private_key() -> &'static RsaPrivateKey {
    static KEY: OnceLock<RsaPrivateKey> = OnceLock::new();
    KEY.get_or_init(|| RsaPrivateKey::new(&mut OsRng, 2048).unwrap())
}

fn key_file(directory: &Path) -> std::path::PathBuf {
    let file = directory.join("private.pem");
    std::fs::write(
        &file,
        private_key()
            .to_pkcs8_pem(LineEnding::LF)
            .unwrap()
            .as_bytes(),
    )
    .unwrap();
    file
}

fn archive(file: &Path, entries: &[&str]) {
    let mut writer = zip::ZipWriter::new(std::fs::File::create(file).unwrap());
    for name in entries {
        writer
            .start_file(
                *name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(b"mini-program fixture").unwrap();
    }
    writer.finish().unwrap();
}

fn config(server: &MockServer, directory: &TempDir, action: &str) -> Config {
    let resource = directory.path().join(format!("{APP}.zip"));
    archive(&resource, &[&format!("{APP}/app.json")]);
    let icon = directory.path().join("icon.png");
    std::fs::write(&icon, b"test icon").unwrap();
    serde_json::from_value(json!({
        "action": action, "baseUrl": server.uri(), "appId": "test-app", "workspaceId": "dev", "tenantId": "tenant",
        "privateKeyFile": key_file(directory.path()), "loginName":"test_user", "h5Id":APP, "packageId":"189", "taskId":28,
        "upload": {"resource": resource, "icon":icon, "h5Id":APP, "h5Name":"测试小程序", "h5Version":"0.0.1", "mainUrl":"/index.html", "vhost":format!("{APP}.example.test"), "clientVersionMin":"1.0.0;1.0.0"},
        "publish":{"publishType":3,"memo":"测试发布"}
    })).unwrap()
}

fn minimal_upload_config(server: &MockServer, directory: &TempDir, fields: Value) -> Config {
    let base = config(server, directory, "upload");
    let upload = base.upload.unwrap();
    let mut settings = fields.as_object().unwrap().clone();
    settings.insert("resource".into(), json!(upload.resource));
    settings.insert("icon".into(), json!(upload.icon));
    settings.insert("h5Id".into(), json!(APP));
    serde_json::from_value(json!({
        "action":"upload","baseUrl":server.uri(),"appId":"test-app",
        "workspaceId":"dev","tenantId":"tenant","privateKeyFile":base.private_key_file,
        "upload":settings
    }))
    .unwrap()
}

fn success(data: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({"data":{"data":data,"success":true,"errorCode":null,"resultMsg":""},"requestId":"mock-request"}))
}

async fn mount(server: &MockServer, verb: &str, endpoint: &str, response: ResponseTemplate) {
    Mock::given(method(verb))
        .and(path(format!("/openapi/miniprogram/{endpoint}")))
        .respond_with(response)
        .expect(1)
        .mount(server)
        .await;
}

async fn mount_resource(server: &MockServer, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path("/openapi/nebula/getNebulaResourceById"))
        .respond_with(response)
        .expect(1)
        .mount(server)
        .await;
}

async fn mount_pending_task(server: &MockServer) {
    let statuses = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    Mock::given(method("GET"))
        .and(path("/openapi/miniprogram/getTaskDetail"))
        .respond_with(move |_: &Request| {
            let status = statuses.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            success(json!({"id":28,"packageId":189,"taskStatus":status}))
        })
        .expect(2)
        .mount(server)
        .await;
    mount(
        server,
        "POST",
        "changeTaskStatus",
        success(json!("success")),
    )
    .await;
}

fn verify_signature(request: &Request) {
    let query = request.url.query_pairs().collect::<BTreeMap<_, _>>();
    assert_eq!(query["appId"], "test-app");
    assert_eq!(query["workspaceId"], "dev");
    assert_eq!(query["tenantId"], "tenant");
    let content = format!(
        "appId=test-app&workspaceId=dev&timestamp={}&url={}",
        query["timestamp"],
        request.url.path()
    );
    let signature =
        Signature::try_from(hex::decode(query["sign"].as_ref()).unwrap().as_slice()).unwrap();
    VerifyingKey::<sha2::Sha256>::new(RsaPublicKey::from(private_key()))
        .verify(content.as_bytes(), &signature)
        .unwrap();
    assert_eq!(
        request.headers.get("cookie").unwrap(),
        "loginName=test_user"
    );
}

fn form_fields(request: &Request) -> BTreeMap<String, String> {
    let content_type = request
        .headers
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    let boundary = content_type.split("boundary=").nth(1).unwrap();
    String::from_utf8_lossy(&request.body)
        .split(&format!("--{boundary}"))
        .filter_map(|part| {
            let (headers, value) = part.split_once("\r\n\r\n")?;
            let name = headers.split("name=\"").nth(1)?.split('"').next()?;
            Some((name.to_owned(), value.trim_end_matches("\r\n").to_owned()))
        })
        .collect()
}

#[tokio::test]
async fn uploads_creates_and_starts_a_pending_formal_task() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let config = config(&server, &directory, "uploadAndPublish");
    mount(&server, "GET", "getPackages", success(json!([]))).await;
    mount(&server, "POST", "uploadPackage", success(json!("189"))).await;
    mount(&server, "POST", "createNebulaTask", success(json!(28))).await;
    let statuses = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = statuses.clone();
    Mock::given(method("GET"))
        .and(path("/openapi/miniprogram/getTaskDetail"))
        .respond_with(move |_: &Request| {
            let status = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            success(json!({"id":28,"packageId":189,"taskStatus":status}))
        })
        .expect(2)
        .mount(&server)
        .await;
    mount(
        &server,
        "POST",
        "changeTaskStatus",
        success(json!("success")),
    )
    .await;
    let result = run(&config).await.unwrap();
    assert!(result.ok);
    assert_eq!(result.package_id.as_deref(), Some("189"));
    assert_eq!(result.task_id.as_deref(), Some("28"));
    assert_eq!(result.task_status, Some(1));
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests
            .iter()
            .map(|r| r.url.path().rsplit('/').next().unwrap())
            .collect::<Vec<_>>(),
        [
            "getPackages",
            "uploadPackage",
            "createNebulaTask",
            "getTaskDetail",
            "changeTaskStatus",
            "getTaskDetail"
        ]
    );
    for request in &requests {
        verify_signature(request);
    }
    for request in requests.iter().filter(|r| r.method.as_str() == "POST") {
        let fields = form_fields(request);
        assert_eq!(fields["appId"], "test-app");
        assert_eq!(fields["workspaceId"], "dev");
        assert_eq!(fields["tenantId"], "tenant");
    }
    let upload = form_fields(&requests[1]);
    assert_eq!(upload["h5Name"], "测试小程序");
    assert!(String::from_utf8_lossy(&requests[1].body).contains("text/plain; charset=utf-8"));
    assert_eq!(upload["resourceType"], "4");
    assert!(String::from_utf8_lossy(&requests[1].body).contains(&format!("filename=\"{APP}.zip\"")));
    assert!(upload.contains_key("resourceFile") && upload.contains_key("iconFile"));
    assert_eq!(upload["h5Version"], "0.0.1");
    assert_eq!(result.h5_version.as_deref(), Some("0.0.1"));
    let create = form_fields(&requests[2]);
    assert_eq!(create["publishType"], "3");
    assert_eq!(create["id"], "0");
    assert!(!create.contains_key("publishMode"));
    assert_eq!(create["memo"], "测试发布");
    assert_eq!(form_fields(&requests[4])["taskStatus"], "1");
}

#[tokio::test]
async fn already_publishing_task_is_not_started_again() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    mount(&server, "GET", "getPackages", success(json!([]))).await;
    mount(&server, "POST", "uploadPackage", success(json!("189"))).await;
    mount(&server, "POST", "createNebulaTask", success(json!("28"))).await;
    mount(
        &server,
        "GET",
        "getTaskDetail",
        success(json!({"id":28,"packageId":189,"taskStatus":1})),
    )
    .await;
    let result = run(&config(&server, &directory, "uploadAndPublish"))
        .await
        .unwrap();
    assert_eq!(result.task_status, Some(1));
    assert_eq!(server.received_requests().await.unwrap().len(), 4);
}

#[tokio::test]
async fn upload_only_returns_a_package_without_creating_a_task() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    mount(&server, "GET", "getPackages", success(json!([]))).await;
    mount(&server, "POST", "uploadPackage", success(json!("189"))).await;
    let result = run(&config(&server, &directory, "upload")).await.unwrap();
    assert_eq!(result.package_id.as_deref(), Some("189"));
    assert!(result.task_id.is_none());
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn next_version_queries_resources_without_writing() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    mount(
        &server,
        "GET",
        "getPackages",
        success(json!([
            {"h5Id":APP,"h5Version":"0.0.9"},
            {"h5Id":APP,"h5Version":"0.0.19"},
            {"h5Id":APP,"h5Version":"0.0.2"}
        ])),
    )
    .await;
    let result = run(&config(&server, &directory, "getNextVersion"))
        .await
        .unwrap();
    assert_eq!(result.action, "getNextVersion");
    assert_eq!(
        result.data,
        json!({"latestVersion":"0.0.19","nextVersion":"0.1.0"})
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method.as_str(), "GET");
    let query = requests[0].url.query_pairs().collect::<BTreeMap<_, _>>();
    assert_eq!(query["h5Id"], APP);
    assert_eq!(query["packageTypes"], "1");
    assert_eq!(query["pageNum"], "1");
    assert_eq!(query["pageSize"], "100");
    verify_signature(&requests[0]);
}

#[tokio::test]
async fn upload_without_an_expected_version_uses_the_first_version() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let mut config = config(&server, &directory, "upload");
    config.upload.as_mut().unwrap().h5_version.clear();
    mount(&server, "GET", "getPackages", success(json!([]))).await;
    mount(&server, "POST", "uploadPackage", success(json!(189))).await;
    let result = run(&config).await.unwrap();
    assert_eq!(result.h5_version.as_deref(), Some("0.0.1"));
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(form_fields(&requests[1])["h5Version"], "0.0.1");
}

#[tokio::test]
async fn a_changed_server_version_prevents_upload() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    mount(
        &server,
        "GET",
        "getPackages",
        success(json!([{"h5Version":"0.1.0"}])),
    )
    .await;
    let error = run(&config(&server, &directory, "uploadAndPublish"))
        .await
        .unwrap_err();
    assert_eq!(error.kind, "versionConflict");
    assert_eq!(error.phase.as_deref(), Some("getNextVersion"));
    assert_eq!(error.h5_version.as_deref(), Some("0.1.1"));
    assert!(error.package_id.is_none());
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method.as_str(), "GET");
}

#[tokio::test]
async fn failed_version_query_prevents_upload() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    mount(&server, "GET", "getPackages", ResponseTemplate::new(503)).await;
    let error = run(&config(&server, &directory, "upload"))
        .await
        .unwrap_err();
    assert_eq!(error.kind, "httpStatus");
    assert_eq!(error.phase.as_deref(), Some("getNextVersion"));
    assert!(error.h5_version.is_none());
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn direct_client_upload_requires_a_resolved_version() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let mut config = config(&server, &directory, "upload");
    config.upload.as_mut().unwrap().h5_version.clear();
    let client = MpaasClient::new(&config).await.unwrap();
    let error = client
        .upload_package(config.upload.as_ref().unwrap())
        .await
        .unwrap_err();
    assert_eq!(error.kind, "config");
    assert!(error.message.contains("h5Version"));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn upload_inherits_details_and_app_defaults_but_preserves_explicit_values() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let config = minimal_upload_config(
        &server,
        &directory,
        json!({
            "autoInstall":0,"installType":false,"enableTabBar":0,
            "enableOptionMenu":false,"enableKeepAlive":0,"platform":"all",
            "clientVersionMax":null,"mainUrl":"/new.html",
            "extendInfo":{"custom":false,"launchParams":{"page":"new"}}
        }),
    );
    mount(
        &server,
        "GET",
        "getPackages",
        success(json!([{"id":189,"h5Version":"0.0.1"}])),
    )
    .await;
    mount_resource(&server, success(json!({
        "id":189,"h5Id":APP,"h5Version":"0.0.1","h5Name":"Previous mini",
        "clientVersionMin":"1.0;1.0","clientVersionMax":"9.0;9.0","platform":"ios",
        "autoInstall":"1","installType":"1","mainUrl":"/old.html",
        "extraData":"{\"enableTabBar\":\"1\",\"enableOptionMenu\":\"1\",\"enableKeepAlive\":\"1\",\"iconUrl\":\"https://server/icon.png\"}",
        "extendInfo":"{\"custom\":\"old\",\"launchParams\":{\"page\":\"old\",\"keep\":\"YES\"}}",
        "memo":"https://server/description.json","downloadUrl":"https://server/package.amr","status":1
    }))).await;
    mount(&server, "GET", "getAppList", success(json!([
        {"h5Id":"0000000000000099","vhost":"wrong.example.test"},
        {"h5Id":APP,"h5Name":"App name","vhost":"mini.example.test","subUrl":"/description.html","autoInstall":0}
    ]))).await;
    mount(&server, "POST", "uploadPackage", success(json!(190))).await;

    let result = run(&config).await.unwrap();
    assert_eq!(result.package_id.as_deref(), Some("190"));
    assert_eq!(result.h5_version.as_deref(), Some("0.0.2"));
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 4);
    let fields = form_fields(&requests[3]);
    assert_eq!(fields["h5Name"], "Previous mini");
    assert_eq!(fields["vhost"], "mini.example.test");
    assert_eq!(fields["clientVersionMin"], "1.0;1.0");
    assert_eq!(fields["platform"], "all");
    assert_eq!(fields["mainUrl"], "/new.html");
    assert_eq!(fields["suburl"], "/description.html");
    for flag in [
        "autoInstall",
        "installType",
        "enableTabBar",
        "enableOptionMenu",
        "enableKeepAlive",
    ] {
        assert_eq!(fields[flag], "0");
    }
    for field in [
        "clientVersionMax",
        "id",
        "memo",
        "downloadUrl",
        "status",
        "extraData",
        "iconUrl",
    ] {
        assert!(!fields.contains_key(field));
    }
    assert_eq!(
        serde_json::from_str::<Value>(&fields["extendInfo"]).unwrap(),
        json!({"custom":false,"launchParams":{"page":"new","keep":"YES"}})
    );
    assert!(!requests[2].url.query_pairs().any(|(key, _)| key == "h5Id"));
}

#[tokio::test]
async fn upload_defaults_preview_needs_no_local_files_and_filters_server_fields() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let mut config = config(&server, &directory, "getUploadDefaults");
    config.upload = None;
    mount(
        &server,
        "GET",
        "getPackages",
        success(json!([{"id":189,"h5Version":"0.0.19"}])),
    )
    .await;
    mount_resource(&server, success(json!({
        "id":189,"h5Id":APP,"h5Version":"0.0.19","h5Name":"Preview mini","vhost":"mini.example.test",
        "clientVersionMin":"1.0;1.0","extraData":"{\"enableKeepAlive\":\"1\"}",
        "extendInfo":"{\"launchParams\":{\"page\":\"home\"}}","downloadUrl":"private","memo":"private"
    }))).await;
    let result = run(&config).await.unwrap();
    assert_eq!(
        result.data,
        json!({
            "latestVersion":"0.0.19","nextVersion":"0.1.0",
            "uploadDefaults":{"h5Name":"Preview mini","vhost":"mini.example.test","clientVersionMin":"1.0;1.0",
                "enableKeepAlive":1,"extendInfo":{"launchParams":{"page":"home"}}}
        })
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn latest_list_fields_without_an_id_can_supply_upload_defaults() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let config = minimal_upload_config(&server, &directory, json!({"extendInfo":{}}));
    mount(&server, "GET", "getPackages", success(json!([{
        "h5Version":"0.0.1","h5Name":"List mini","vhost":"list.example.test","clientVersionMin":"1.0;1.0",
        "enableOptionMenu":0,"extendInfo":{"launchParams":{"keep":"YES"}}
    }]))).await;
    mount(&server, "POST", "uploadPackage", success(json!(190))).await;
    let result = run(&config).await.unwrap();
    assert_eq!(result.h5_version.as_deref(), Some("0.0.2"));
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    let fields = form_fields(&requests[1]);
    assert_eq!(fields["h5Name"], "List mini");
    assert_eq!(fields["enableOptionMenu"], "0");
    assert_eq!(
        serde_json::from_str::<Value>(&fields["extendInfo"]).unwrap(),
        json!({"launchParams":{"keep":"YES"}})
    );
}

#[tokio::test]
async fn explicit_null_clears_optional_historical_settings() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let config = minimal_upload_config(
        &server,
        &directory,
        json!({"extendInfo":null,"clientVersionMax":null,"mainUrl":null}),
    );
    mount(&server, "GET", "getPackages", success(json!([{
        "h5Version":"0.0.1","h5Name":"List mini","vhost":"list.example.test","clientVersionMin":"1.0;1.0",
        "mainUrl":"/old.html","clientVersionMax":"9.0;9.0","extendInfo":{"launchParams":{"keep":"YES"}}
    }]))).await;
    mount(&server, "POST", "uploadPackage", success(json!(190))).await;
    run(&config).await.unwrap();
    let requests = server.received_requests().await.unwrap();
    let fields = form_fields(&requests[1]);
    for field in ["extendInfo", "clientVersionMax", "mainUrl"] {
        assert!(!fields.contains_key(field));
    }
}

#[tokio::test]
async fn first_upload_with_missing_required_defaults_is_not_sent() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let config = minimal_upload_config(&server, &directory, json!({}));
    mount(&server, "GET", "getPackages", success(json!([]))).await;
    mount(
        &server,
        "GET",
        "getAppList",
        success(json!([{"h5Id":APP,"h5Name":"App mini","vhost":"app.example.test"}])),
    )
    .await;
    let error = run(&config).await.unwrap_err();
    assert_eq!(error.kind, "config");
    assert_eq!(error.phase.as_deref(), Some("prepareUpload"));
    assert_eq!(error.h5_version.as_deref(), Some("0.0.1"));
    assert!(error.message.contains("clientVersionMin"));
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn explicit_empty_required_values_are_not_replaced_by_history() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let config = minimal_upload_config(&server, &directory, json!({"h5Name":"","vhost":""}));
    mount(&server, "GET", "getPackages", success(json!([{
        "h5Version":"0.0.1","h5Name":"List mini","vhost":"list.example.test","clientVersionMin":"1.0;1.0"
    }]))).await;
    let error = run(&config).await.unwrap_err();
    assert_eq!(error.kind, "config");
    assert!(error.message.contains("h5Name"));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn missing_local_upload_files_stop_before_default_queries() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let config = minimal_upload_config(&server, &directory, json!({}));
    std::fs::remove_file(&config.upload.as_ref().unwrap().icon).unwrap();
    let error = run(&config).await.unwrap_err();
    assert_eq!(error.kind, "config");
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn failed_or_mismatched_resource_details_block_upload() {
    for response in [
        ResponseTemplate::new(503),
        success(json!({"id":190,"h5Id":APP,"h5Version":"0.0.1"})),
        success(json!({"id":189,"h5Id":"0000000000000099","h5Version":"0.0.1"})),
        success(json!({"id":189,"h5Id":APP,"h5Version":"0.0.2"})),
    ] {
        let server = MockServer::start().await;
        let directory = tempfile::tempdir().unwrap();
        let config = minimal_upload_config(&server, &directory, json!({}));
        mount(
            &server,
            "GET",
            "getPackages",
            success(json!([{"id":189,"h5Version":"0.0.1"}])),
        )
        .await;
        mount_resource(&server, response).await;
        let error = run(&config).await.unwrap_err();
        assert_eq!(error.phase.as_deref(), Some("getUploadDefaults"));
        assert_eq!(error.h5_version.as_deref(), Some("0.0.2"));
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }
}

#[tokio::test]
async fn publishes_an_existing_package_without_uploading() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let mut config = config(&server, &directory, "publish");
    config.upload = None;
    config.task_id = None;
    mount(&server, "POST", "createNebulaTask", success(json!(28))).await;
    mount_pending_task(&server).await;

    let result = run(&config).await.unwrap();
    assert_eq!(result.action, "publish");
    assert_eq!(result.package_id.as_deref(), Some("189"));
    assert_eq!(result.task_id.as_deref(), Some("28"));
    assert_eq!(result.task_status, Some(1));
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests
            .iter()
            .map(|r| r.url.path().rsplit('/').next().unwrap())
            .collect::<Vec<_>>(),
        [
            "createNebulaTask",
            "getTaskDetail",
            "changeTaskStatus",
            "getTaskDetail"
        ]
    );
    let create = form_fields(&requests[0]);
    assert_eq!(create["packageId"], "189");
    assert_eq!(create["publishType"], "3");
    assert_eq!(create["id"], "0");
}

#[tokio::test]
async fn resumes_a_pending_task_without_creating_or_uploading() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let mut config = config(&server, &directory, "publish");
    config.upload = None;
    config.publish.publish_mode = Some(1);
    mount_pending_task(&server).await;

    let result = run(&config).await.unwrap();
    assert_eq!(result.package_id.as_deref(), Some("189"));
    assert_eq!(result.task_id.as_deref(), Some("28"));
    assert_eq!(result.task_status, Some(1));
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests
            .iter()
            .map(|r| r.url.path().rsplit('/').next().unwrap())
            .collect::<Vec<_>>(),
        ["getTaskDetail", "changeTaskStatus", "getTaskDetail"]
    );
}

#[tokio::test]
async fn resuming_a_publishing_task_only_queries_each_time() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let mut config = config(&server, &directory, "publish");
    config.upload = None;
    Mock::given(method("GET"))
        .and(path("/openapi/miniprogram/getTaskDetail"))
        .respond_with(success(json!({"id":28,"packageId":189,"taskStatus":1})))
        .expect(2)
        .mount(&server)
        .await;
    for _ in 0..2 {
        let result = run(&config).await.unwrap();
        assert_eq!(result.task_status, Some(1));
    }
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests
        .iter()
        .all(|request| request.method.as_str() == "GET"));
}

#[tokio::test]
async fn publish_checks_package_and_creation_settings_before_requests() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let mut config = config(&server, &directory, "publish");
    config.package_id = None;
    assert!(run(&config)
        .await
        .unwrap_err()
        .message
        .contains("packageId"));
    config.package_id = Some(Id("189".into()));
    config.task_id = None;
    config.publish.publish_mode = Some(1);
    assert!(run(&config)
        .await
        .unwrap_err()
        .message
        .contains("publishMode"));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn resumed_task_with_mismatched_ids_is_not_started() {
    for detail in [
        json!({"id":28,"packageId":190,"taskStatus":0}),
        json!({"id":29,"packageId":189,"taskStatus":0}),
    ] {
        let server = MockServer::start().await;
        let directory = tempfile::tempdir().unwrap();
        mount(&server, "GET", "getTaskDetail", success(detail)).await;
        let error = run(&config(&server, &directory, "publish"))
            .await
            .unwrap_err();
        assert_eq!(error.kind, "response");
        assert_eq!(error.phase.as_deref(), Some("getTaskDetail"));
        assert_eq!(error.package_id.as_deref(), Some("189"));
        assert_eq!(error.task_id.as_deref(), Some("28"));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn failed_publish_creation_keeps_existing_package_and_is_not_retried() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let mut config = config(&server, &directory, "publish");
    config.upload = None;
    config.task_id = None;
    mount(
        &server,
        "POST",
        "createNebulaTask",
        ResponseTemplate::new(503),
    )
    .await;
    let error = run(&config).await.unwrap_err();
    assert_eq!(error.phase.as_deref(), Some("createTask"));
    assert_eq!(error.package_id.as_deref(), Some("189"));
    assert!(error.task_id.is_none());
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn failed_resume_start_keeps_both_ids_and_is_not_retried() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    mount(
        &server,
        "GET",
        "getTaskDetail",
        success(json!({"id":28,"packageId":189,"taskStatus":0})),
    )
    .await;
    mount(
        &server,
        "POST",
        "changeTaskStatus",
        ResponseTemplate::new(503),
    )
    .await;
    let error = run(&config(&server, &directory, "publish"))
        .await
        .unwrap_err();
    assert_eq!(error.phase.as_deref(), Some("startTask"));
    assert_eq!(error.package_id.as_deref(), Some("189"));
    assert_eq!(error.task_id.as_deref(), Some("28"));
    assert_eq!(error.task_status, Some(0));
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn resumed_stopped_task_keeps_ids_without_starting() {
    for status in [2, 3] {
        let server = MockServer::start().await;
        let directory = tempfile::tempdir().unwrap();
        mount(
            &server,
            "GET",
            "getTaskDetail",
            success(json!({"id":28,"packageId":189,"taskStatus":status})),
        )
        .await;
        let error = run(&config(&server, &directory, "publish"))
            .await
            .unwrap_err();
        assert_eq!(error.kind, "taskState");
        assert_eq!(error.package_id.as_deref(), Some("189"));
        assert_eq!(error.task_id.as_deref(), Some("28"));
        assert_eq!(error.task_status, Some(status));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn nested_business_failure_stops_before_creating_a_task() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    mount(&server, "GET", "getPackages", success(json!([]))).await;
    mount(&server, "POST", "uploadPackage", ResponseTemplate::new(200).set_body_json(json!({"success":true,"data":{"success":false,"data":"189","errorCode":"DENIED","resultMsg":"permission denied"}}))).await;
    let error = run(&config(&server, &directory, "uploadAndPublish"))
        .await
        .unwrap_err();
    assert_eq!(error.kind, "api");
    assert_eq!(error.phase.as_deref(), Some("upload"));
    assert!(error.message.contains("permission denied"));
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn failed_create_retains_the_uploaded_package_id_and_is_not_retried() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    mount(&server, "GET", "getPackages", success(json!([]))).await;
    mount(&server, "POST", "uploadPackage", success(json!(189))).await;
    mount(
        &server,
        "POST",
        "createNebulaTask",
        ResponseTemplate::new(503),
    )
    .await;
    let error = run(&config(&server, &directory, "uploadAndPublish"))
        .await
        .unwrap_err();
    assert_eq!(error.package_id.as_deref(), Some("189"));
    assert_eq!(error.phase.as_deref(), Some("createTask"));
    assert_eq!(error.h5_version.as_deref(), Some("0.0.1"));
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn unexpected_task_state_preserves_both_ids_without_starting() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    mount(&server, "GET", "getPackages", success(json!([]))).await;
    mount(&server, "POST", "uploadPackage", success(json!(189))).await;
    mount(&server, "POST", "createNebulaTask", success(json!(28))).await;
    mount(
        &server,
        "GET",
        "getTaskDetail",
        success(json!({"taskStatus":3,"id":28,"packageId":189})),
    )
    .await;
    let error = run(&config(&server, &directory, "uploadAndPublish"))
        .await
        .unwrap_err();
    assert_eq!(error.package_id.as_deref(), Some("189"));
    assert_eq!(error.task_id.as_deref(), Some("28"));
    assert_eq!(error.task_status, Some(3));
    assert_eq!(server.received_requests().await.unwrap().len(), 4);
}

#[tokio::test]
async fn failed_start_confirmation_preserves_recovery_ids() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    mount(&server, "GET", "getPackages", success(json!([]))).await;
    mount(&server, "POST", "uploadPackage", success(json!(189))).await;
    mount(&server, "POST", "createNebulaTask", success(json!(28))).await;
    Mock::given(method("GET"))
        .and(path("/openapi/miniprogram/getTaskDetail"))
        .respond_with(success(json!({"taskStatus":0})))
        .expect(2)
        .mount(&server)
        .await;
    mount(
        &server,
        "POST",
        "changeTaskStatus",
        success(json!("success")),
    )
    .await;
    let error = run(&config(&server, &directory, "uploadAndPublish"))
        .await
        .unwrap_err();
    assert_eq!(error.phase.as_deref(), Some("confirmTask"));
    assert_eq!(error.package_id.as_deref(), Some("189"));
    assert_eq!(error.task_id.as_deref(), Some("28"));
    assert_eq!(error.task_status, Some(0));
}

#[tokio::test]
async fn publish_configuration_is_checked_before_upload() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let mut config = config(&server, &directory, "uploadAndPublish");
    config.publish.publish_mode = Some(1);
    assert!(run(&config)
        .await
        .unwrap_err()
        .message
        .contains("publishMode"));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn zip_preflight_rejects_amr_wrong_root_and_traversal() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let mut config = config(&server, &directory, "upload");
    let upload = config.upload.as_mut().unwrap();
    for entry in [
        "other/app.json",
        "0000000000000012/../app.json",
        "/app.json",
    ] {
        archive(&upload.resource, &[entry]);
        assert!(upload.validate().is_err());
    }
    upload.resource = directory.path().join(format!("{APP}.amr"));
    assert!(upload.validate().unwrap_err().message.contains("AMR"));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[test]
fn resource_directory_is_packaged_under_the_app_id() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("compiled-output");
    std::fs::create_dir_all(source.join("pages")).unwrap();
    std::fs::write(source.join("index.html"), b"index").unwrap();
    std::fs::write(source.join("pages/home.js"), b"home").unwrap();
    crate::resource::validate(&source, APP).unwrap();
    let (archive_path, temporary) = crate::resource::package_directory(&source, APP).unwrap();
    let file = std::fs::File::open(&archive_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    assert!(archive.by_name(&format!("{APP}/index.html")).is_ok());
    assert!(archive.by_name(&format!("{APP}/pages/home.js")).is_ok());
    drop(archive);
    crate::resource::cleanup(&temporary);
    assert!(!archive_path.exists());
}

#[test]
fn resource_directory_rejects_uncompiled_source_projects() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source-project");
    std::fs::create_dir_all(source.join("pages/index")).unwrap();
    std::fs::write(source.join("mini.project.json"), b"{}").unwrap();
    std::fs::write(source.join("pages/index/index.axml"), b"<view />").unwrap();
    let error = crate::resource::validate(&source, APP).unwrap_err();
    assert!(error.message.contains("source project"));
}

#[cfg(unix)]
#[test]
fn resource_directory_rejects_symbolic_links_and_empty_directories() {
    let directory = tempfile::tempdir().unwrap();
    let empty = directory.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    assert!(crate::resource::validate(&empty, APP).is_err());
    let source = directory.path().join("linked");
    std::fs::create_dir(&source).unwrap();
    std::os::unix::fs::symlink("/tmp", source.join("link")).unwrap();
    assert!(crate::resource::validate(&source, APP).is_err());
}

#[tokio::test]
async fn queries_use_the_documented_endpoints_and_parameters() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    mount(&server, "GET", "getPackages", success(json!([]))).await;
    mount(&server, "GET", "nebulaTaskList", success(json!([]))).await;
    Mock::given(method("GET"))
        .and(path("/openapi/nebula/getNebulaResourceById"))
        .respond_with(success(json!({"id":189})))
        .expect(1)
        .mount(&server)
        .await;
    let mut config = config(&server, &directory, "getPackages");
    config.page_num = Some(1);
    config.page_size = Some(10);
    run(&config).await.unwrap();
    config.action = "getTaskList".into();
    run(&config).await.unwrap();
    config.action = "getResource".into();
    run(&config).await.unwrap();
    let requests = server.received_requests().await.unwrap();
    for request in &requests {
        verify_signature(request);
    }
    let query = requests[0].url.query_pairs().collect::<BTreeMap<_, _>>();
    assert_eq!(query["h5Id"], APP);
    assert_eq!(query["pageSize"], "10");
    for request in &requests[1..] {
        assert_eq!(
            request
                .url
                .query_pairs()
                .find(|(key, _)| key == "id")
                .unwrap()
                .1,
            "189"
        );
    }
}

#[tokio::test]
async fn redirects_are_not_followed() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    mount(
        &server,
        "GET",
        "getPackages",
        ResponseTemplate::new(302).insert_header("Location", "/unexpected"),
    )
    .await;
    let error = run(&config(&server, &directory, "getPackages"))
        .await
        .unwrap_err();
    assert_eq!(error.kind, "httpStatus");
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn transport_errors_do_not_contain_a_signed_url() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let mut config = config(&server, &directory, "getPackages");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    config.base_url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let error = run(&config).await.unwrap_err();
    assert_eq!(error.kind, "http");
    assert!(!error.message.contains("sign="));
    assert!(!format!("{error:?}").contains("sign="));
}

#[tokio::test]
async fn config_paths_are_relative_to_config_file_and_environment_is_expanded_after_json() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("config.json");
    std::env::set_var("MPAAS_TEST_WINDOWS_VALUE", "C:\\keys\\key \"name\".pem");
    std::fs::write(&file, serde_json::to_vec(&json!({"action":"getPackages","baseUrl":"http://localhost:8281","appId":"app","workspaceId":"dev","tenantId":"tenant","privateKeyFile":"./key.pem","h5Id":APP,"build":{"project":"ignored"},"subType":"${MPAAS_TEST_WINDOWS_VALUE}"})).unwrap()).unwrap();
    let config = Config::from_file(&file).await.unwrap();
    assert_eq!(config.private_key_file, directory.path().join("./key.pem"));
    assert_eq!(
        config.sub_type.as_deref(),
        Some("C:\\keys\\key \"name\".pem")
    );
    std::env::remove_var("MPAAS_TEST_WINDOWS_VALUE");
}

#[tokio::test]
async fn supports_java_base64_pkcs8_and_pkcs1_pem_keys() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let config = config(&server, &directory, "getPackages");
    for text in [
        STANDARD.encode(private_key().to_pkcs8_der().unwrap().as_bytes()),
        private_key()
            .to_pkcs1_pem(LineEnding::LF)
            .unwrap()
            .to_string(),
    ] {
        std::fs::write(&config.private_key_file, text).unwrap();
        MpaasClient::new(&config).await.unwrap();
    }
}

#[tokio::test]
async fn serde_api_registry_calls_java_demo_endpoint() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    Mock::given(method("GET"))
        .and(path("/openapi/miniprogram/getAppList"))
        .respond_with(success(json!([{"h5Id": APP, "h5Name": "测试小程序"}])))
        .expect(1)
        .mount(&server)
        .await;
    let config = config(&server, &directory, "getPackages");
    let client = MpaasClient::new(&config).await.unwrap();
    let response = client
        .mini_program_list_apps(&models::ListRequest::default())
        .await
        .unwrap();
    assert_eq!(response.data[0].h5_id.as_deref(), Some(APP));
    assert_eq!(response.request_id.as_deref(), Some("mock-request"));
}

#[tokio::test]
async fn typed_api_keeps_outer_pagination_metadata() {
    use models::{Integer, ListRequest, MiniProgramPackageListRequest};

    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    mount(
        &server,
        "GET",
        "getPackages",
        ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "success": true,
                "data": [{
                    "id": 683,
                    "h5Id": APP,
                    "h5Version": "1.1.1.1",
                    "resourceType": "4",
                    "extraData": "{\"enableKeepAlive\":\"1\",\"resourceType\":\"4\"}"
                }]
            },
            "requestId": "paginated",
            "currentPage": 2,
            "pageSize": 1,
            "totalCount": 3,
            "hasMore": true
        })),
    )
    .await;
    let client = MpaasClient::new(&config(&server, &directory, "getPackages"))
        .await
        .unwrap();
    let result = client
        .mini_program_list_packages(&MiniProgramPackageListRequest {
            h5_id: APP.into(),
            package_types: "1,2".into(),
            sub_type: None,
            pagination: ListRequest {
                page_num: Some(2),
                page_size: Some(1),
            },
        })
        .await
        .unwrap();
    assert_eq!(result.data[0].id, Some(Integer::Number(683)));
    let extra = result.data[0].extra_data.as_ref().unwrap();
    assert_eq!(extra.0.enable_keep_alive, Some(Integer::Text("1".into())));
    assert_eq!(result.pagination.current_page, Some(2));
    assert_eq!(result.pagination.has_more, Some(true));
    assert_eq!(result.request_id.as_deref(), Some("paginated"));
    let requests = server.received_requests().await.unwrap();
    verify_signature(&requests[0]);
    let query: BTreeMap<_, _> = requests[0].url.query_pairs().collect();
    assert_eq!(query["h5Id"], APP);
    assert_eq!(query["packageTypes"], "1,2");
    assert_eq!(query["pageNum"], "2");
}

#[tokio::test]
async fn typed_api_sends_json_batches_with_authentication_context() {
    use models::{CreateAppRequest, NebulaAppBatchRequest};

    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    Mock::given(method("POST"))
        .and(path("/openapi/nebula/createNebulaIdBatch"))
        .respond_with(success(json!([
            {"h5Id": "12345678", "h5Version": "", "messge": "", "success": true}
        ])))
        .expect(1)
        .mount(&server)
        .await;
    let client = MpaasClient::new(&config(&server, &directory, "getPackages"))
        .await
        .unwrap();
    let result = client
        .nebula_create_apps_batch(&NebulaAppBatchRequest {
            app_id: None,
            workspace_id: None,
            nebula_app_request_list: vec![CreateAppRequest {
                h5_id: "12345678".into(),
                h5_name: "批量示例".into(),
            }],
        })
        .await
        .unwrap();
    assert!(result.data[0].success);
    let requests = server.received_requests().await.unwrap();
    verify_signature(&requests[0]);
    assert_eq!(requests[0].headers["content-type"], "application/json");
    let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(
        body,
        json!({
            "appId": "test-app",
            "workspaceId": "dev",
            "tenantId": "tenant",
            "nebulaAppRequestList": [{"h5Id": "12345678", "h5Name": "批量示例"}]
        })
    );
}

#[tokio::test]
async fn api_config_resolves_batch_files_and_rejects_missing_required_parameters() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    for app in ["12345678", "87654321"] {
        archive(
            &directory.path().join(format!("{app}.zip")),
            &[&format!("{app}/index.html")],
        );
    }
    let file = directory.path().join("api.json");
    let mut config = json!({
        "action": "api", "baseUrl": server.uri(), "appId": "test-app", "workspaceId": "dev",
        "tenantId": "tenant", "loginName": "test_user", "privateKeyFile": key_file(directory.path()),
        "api": {
            "path": "/openapi/nebula/createNebulaResourceBatch",
            "params": {
                "configString": {
                    "sharedConfig": {"platform": "all", "autoInstall": "0"},
                    "nebulaResourceList": [
                        {"h5Id": "12345678", "h5Version": "1.0.0.1"},
                        {"h5Id": "87654321", "h5Version": "1.0.0.2"}
                    ]
                },
                "files": ["12345678.zip", "87654321.zip"]
            }
        }
    });
    let mut invalid = config.clone();
    invalid["api"]["params"]
        .as_object_mut()
        .unwrap()
        .remove("files");
    std::fs::write(&file, invalid.to_string()).unwrap();
    let error = run(&Config::from_file(&file).await.unwrap())
        .await
        .unwrap_err();
    assert!(error.message.contains("files"), "{error}");

    let mut invalid = config.clone();
    invalid["api"]["params"]["configString"]["sharedConfig"]["h5Version"] = json!("1.0.0.1");
    invalid["api"]["params"]["configString"]["nebulaResourceList"][0]
        .as_object_mut()
        .unwrap()
        .remove("h5Version");
    std::fs::write(&file, invalid.to_string()).unwrap();
    let error = run(&Config::from_file(&file).await.unwrap())
        .await
        .unwrap_err();
    assert!(error.message.contains("h5Version"), "{error}");
    assert!(server.received_requests().await.unwrap().is_empty());

    Mock::given(method("POST"))
        .and(path("/openapi/nebula/createNebulaResourceBatch"))
        .respond_with(success(json!([])))
        .expect(1)
        .mount(&server)
        .await;
    std::fs::write(&file, config.take().to_string()).unwrap();
    run(&Config::from_file(&file).await.unwrap()).await.unwrap();
    let requests = server.received_requests().await.unwrap();
    verify_signature(&requests[0]);
    let body = String::from_utf8_lossy(&requests[0].body);
    assert_eq!(body.matches("name=\"files\"").count(), 2);
    assert!(body.contains("filename=\"12345678.zip\""));
    assert!(body.contains("filename=\"87654321.zip\""));
    let fields = form_fields(&requests[0]);
    let nested: Value = serde_json::from_str(&fields["configString"]).unwrap();
    assert_eq!(nested["sharedConfig"]["autoInstall"], "0");
    assert_eq!(nested["nebulaResourceList"][1]["h5Version"], "1.0.0.2");
}

#[tokio::test]
async fn typed_api_checks_conditional_required_fields_before_requests() {
    use models::{MiniProgramPublishTaskRequest, ModifyWhitelistRequest, PublishTaskRequest};

    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let client = MpaasClient::new(&config(&server, &directory, "getPackages"))
        .await
        .unwrap();
    let task = MiniProgramPublishTaskRequest {
        settings: PublishTaskRequest {
            publish_type: 2,
            publish_mode: Some(2),
            memo: None,
            id: 0,
            package_id: 189.into(),
            grey_endtime_data: None,
            grey_num: Some(10),
            whitelist_ids: None,
            grey_config_info: None,
        },
        experience_flag: None,
    };
    let error = client.mini_program_create_task(&task).await.unwrap_err();
    assert!(error.message.contains("greyEndtimeData"), "{error}");
    let error = client
        .whitelist_add_entries(&ModifyWhitelistRequest {
            id: 189.into(),
            key_ids: Some("user-a".into()),
            key_ids_file: Some("users.txt".into()),
        })
        .await
        .unwrap_err();
    assert!(error.message.contains("exactly one"), "{error}");
    let error = client
        .mini_program_update_package_type(&models::UpdateMiniPackageTypeRequest {
            h5_id: APP.into(),
            package_id: 189.into(),
            package_type: 4,
        })
        .await
        .unwrap_err();
    assert!(error.message.contains("packageType must be 1"), "{error}");
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn failed_start_retains_ids_and_is_not_retried() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    mount(&server, "GET", "getPackages", success(json!([]))).await;
    mount(&server, "POST", "uploadPackage", success(json!(189))).await;
    mount(&server, "POST", "createNebulaTask", success(json!(28))).await;
    mount(
        &server,
        "GET",
        "getTaskDetail",
        success(json!({"taskStatus":0})),
    )
    .await;
    mount(
        &server,
        "POST",
        "changeTaskStatus",
        ResponseTemplate::new(200).set_body_json(
            json!({"data":{"success":false,"errorCode":"DENIED","resultMsg":"cannot publish"}}),
        ),
    )
    .await;
    let error = run(&config(&server, &directory, "uploadAndPublish"))
        .await
        .unwrap_err();
    assert_eq!(error.phase.as_deref(), Some("startTask"));
    assert_eq!(error.package_id.as_deref(), Some("189"));
    assert_eq!(error.task_id.as_deref(), Some("28"));
    assert_eq!(server.received_requests().await.unwrap().len(), 5);
}

#[tokio::test]
async fn task_detail_with_wrong_package_is_not_started() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    mount(&server, "GET", "getPackages", success(json!([]))).await;
    mount(&server, "POST", "uploadPackage", success(json!(189))).await;
    mount(&server, "POST", "createNebulaTask", success(json!(28))).await;
    mount(
        &server,
        "GET",
        "getTaskDetail",
        success(json!({"id":28,"packageId":190,"taskStatus":0})),
    )
    .await;
    let error = run(&config(&server, &directory, "uploadAndPublish"))
        .await
        .unwrap_err();
    assert_eq!(error.kind, "response");
    assert_eq!(error.task_id.as_deref(), Some("28"));
    assert_eq!(server.received_requests().await.unwrap().len(), 4);
}

#[tokio::test]
async fn configured_timeout_ends_a_stalled_request() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    mount(
        &server,
        "GET",
        "getPackages",
        success(json!([])).set_delay(std::time::Duration::from_secs(2)),
    )
    .await;
    let mut config = config(&server, &directory, "getPackages");
    config.timeout_seconds = 1;
    let error = run(&config).await.unwrap_err();
    assert_eq!(error.kind, "http");
    assert!(!error.message.contains("sign="));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn server_echoed_signature_is_redacted_from_business_errors() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    Mock::given(method("GET")).and(path("/openapi/miniprogram/getPackages")).respond_with(|request: &Request| {
        let sign = request.url.query_pairs().find(|(key,_)| key == "sign").unwrap().1;
        ResponseTemplate::new(200).set_body_json(json!({"data":{"success":false,"errorCode":"SIGN_ERROR","resultMsg":format!("invalid sign={sign}")}}))
    }).expect(1).mount(&server).await;
    let error = run(&config(&server, &directory, "getPackages"))
        .await
        .unwrap_err();
    let request = server.received_requests().await.unwrap().pop().unwrap();
    let sign = request
        .url
        .query_pairs()
        .find(|(key, _)| key == "sign")
        .unwrap()
        .1;
    assert!(!error.message.contains(sign.as_ref()));
    assert!(error.message.contains("<redacted>"));
}

#[tokio::test]
async fn base_url_with_a_path_is_rejected_before_a_request() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let mut config = config(&server, &directory, "uploadAndPublish");
    config.base_url.push_str("/gateway");
    let error = run(&config).await.unwrap_err();
    assert_eq!(error.kind, "config");
    assert!(error.message.contains("origin"));
    assert!(server.received_requests().await.unwrap().is_empty());
}
