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
        "upload": {"resource": resource, "icon":icon, "h5Id":APP, "h5Name":"测试小程序", "h5Version":"1.0.0.0", "mainUrl":"/index.html", "vhost":format!("{APP}.example.test"), "clientVersionMin":"1.0.0;1.0.0"},
        "publish":{"publishType":3,"memo":"测试发布"}
    })).unwrap()
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
    let upload = form_fields(&requests[0]);
    assert_eq!(upload["h5Name"], "测试小程序");
    assert!(String::from_utf8_lossy(&requests[0].body).contains("text/plain; charset=utf-8"));
    assert_eq!(upload["resourceType"], "4");
    assert!(String::from_utf8_lossy(&requests[0].body).contains(&format!("filename=\"{APP}.zip\"")));
    assert!(upload.contains_key("resourceFile") && upload.contains_key("iconFile"));
    let create = form_fields(&requests[1]);
    assert_eq!(create["publishType"], "3");
    assert_eq!(create["id"], "0");
    assert!(!create.contains_key("publishMode"));
    assert_eq!(create["memo"], "测试发布");
    assert_eq!(form_fields(&requests[3])["taskStatus"], "1");
}

#[tokio::test]
async fn already_publishing_task_is_not_started_again() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
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
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn nested_business_failure_stops_before_creating_a_task() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    mount(&server, "POST", "uploadPackage", ResponseTemplate::new(200).set_body_json(json!({"success":true,"data":{"success":false,"data":"189","errorCode":"DENIED","resultMsg":"permission denied"}}))).await;
    let error = run(&config(&server, &directory, "uploadAndPublish"))
        .await
        .unwrap_err();
    assert_eq!(error.kind, "api");
    assert_eq!(error.phase.as_deref(), Some("upload"));
    assert!(error.message.contains("permission denied"));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn failed_create_retains_the_uploaded_package_id_and_is_not_retried() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
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
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn unexpected_task_state_preserves_both_ids_without_starting() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
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
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn failed_start_confirmation_preserves_recovery_ids() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
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
async fn failed_start_retains_ids_and_is_not_retried() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
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
    assert_eq!(server.received_requests().await.unwrap().len(), 4);
}

#[tokio::test]
async fn task_detail_with_wrong_package_is_not_started() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
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
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
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
