use crate::{Config, Error, MpaasClient};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use tracing::info;

const PAGE_SIZE: u64 = 100;
const MAX_PAGES: u64 = 1000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VersionPlan {
    pub(crate) latest_version: Option<String>,
    pub(crate) next_version: String,
    #[serde(skip)]
    pub(crate) latest_resource: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Version(u64, u64, u64);

impl Version {
    fn parse(text: &str) -> Result<Self, Error> {
        let parts: Vec<_> = text.split('.').collect();
        if parts.len() != 3
            || parts
                .iter()
                .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
        {
            return Err(Error::new(
                "version",
                format!("resource version {text:?} must contain three numeric segments"),
            ));
        }
        let numbers: Result<Vec<u64>, _> = parts.iter().map(|part| part.parse()).collect();
        let numbers =
            numbers.map_err(|_| Error::new("version", "resource version is too large"))?;
        if numbers[2] >= 20 {
            return Err(Error::new(
                "version",
                format!("resource version {text:?} has a last segment outside 0..19"),
            ));
        }
        Ok(Self(numbers[0], numbers[1], numbers[2]))
    }

    fn next(self) -> Result<String, Error> {
        let (minor, patch) = if self.2 == 19 {
            (
                self.1
                    .checked_add(1)
                    .ok_or_else(|| Error::new("version", "middle version segment is too large"))?,
                0,
            )
        } else {
            (self.1, self.2 + 1)
        };
        Ok(format!("{}.{}.{}", self.0, minor, patch))
    }
}

struct Page<'a> {
    records: &'a [Value],
    total: Option<u64>,
    total_pages: Option<u64>,
    page_size: Option<u64>,
}

fn page(value: &Value) -> Result<Page<'_>, Error> {
    if let Some(records) = value.as_array() {
        return Ok(Page {
            records,
            total: None,
            total_pages: None,
            page_size: None,
        });
    }
    let object = value.as_object().ok_or_else(|| {
        Error::new(
            "response",
            "resource list must be an array or a pagination object",
        )
    })?;
    let mut records = None;
    for field in ["list", "records", "items", "dataList", "content", "data"] {
        if let Some(items) = object.get(field).and_then(Value::as_array) {
            if records.is_some_and(|previous| previous != items.as_slice()) {
                return Err(Error::new(
                    "response",
                    "resource list contains conflicting arrays",
                ));
            }
            records = Some(items.as_slice());
        }
    }
    let records = records.ok_or_else(|| {
        Error::new(
            "response",
            "resource pagination has no list, records, items, dataList, content or data array",
        )
    })?;
    let total = pagination_number(object, &["total", "totalCount", "totalElements"])?;
    let total_pages = pagination_number(object, &["totalPages", "pageCount"])?;
    let page_size = pagination_number(object, &["pageSize", "size"])?;
    if page_size == Some(0) {
        return Err(Error::new(
            "response",
            "resource pagination page size must be positive",
        ));
    }
    Ok(Page {
        records,
        total,
        total_pages,
        page_size,
    })
}

fn pagination_number(
    object: &serde_json::Map<String, Value>,
    fields: &[&str],
) -> Result<Option<u64>, Error> {
    let mut result = None;
    for field in fields {
        if let Some(value) = object.get(*field).filter(|value| !value.is_null()) {
            let count = value
                .as_u64()
                .or_else(|| value.as_str()?.parse::<u64>().ok())
                .ok_or_else(|| {
                    Error::new(
                        "response",
                        format!("resource pagination {field} is invalid"),
                    )
                })?;
            if result.is_some_and(|previous| previous != count) {
                return Err(Error::new(
                    "response",
                    format!("resource pagination fields {} conflict", fields.join(", ")),
                ));
            }
            result = Some(count);
        }
    }
    Ok(result)
}

fn resource_version(record: &Value, h5_id: &str) -> Result<Option<(Version, String)>, Error> {
    let object = record
        .as_object()
        .ok_or_else(|| Error::new("response", "each resource must be an object"))?;
    if let Some(value) = object.get("h5Id").filter(|value| !value.is_null()) {
        let resource_id = value
            .as_str()
            .ok_or_else(|| Error::new("response", "resource h5Id must be a string"))?;
        if resource_id != h5_id {
            return Ok(None);
        }
    }
    let mut version: Option<(Version, String)> = None;
    for field in ["h5Version", "version", "packageVersion"] {
        if let Some(value) = object.get(field).filter(|value| !value.is_null()) {
            let text = value.as_str().ok_or_else(|| {
                Error::new(
                    "response",
                    format!("resource {field} must be a version string"),
                )
            })?;
            let parsed = Version::parse(text)?;
            if version
                .as_ref()
                .is_some_and(|(previous, _)| *previous != parsed)
            {
                return Err(Error::new(
                    "response",
                    "resource h5Version, version or packageVersion values conflict",
                ));
            }
            version.get_or_insert((parsed, text.to_owned()));
        }
    }
    version.map(Some).ok_or_else(|| {
        Error::new(
            "response",
            "resource has no h5Version, version or packageVersion",
        )
    })
}

fn plan(latest: Option<(Version, String, Value)>) -> Result<VersionPlan, Error> {
    match latest {
        Some((version, text, resource)) => Ok(VersionPlan {
            latest_version: Some(text),
            next_version: version.next()?,
            latest_resource: Some(resource),
        }),
        None => Ok(VersionPlan {
            latest_version: None,
            next_version: "0.0.1".into(),
            latest_resource: None,
        }),
    }
}

pub(crate) async fn next_version(
    client: &MpaasClient,
    config: &Config,
    h5_id: &str,
    package_types: &str,
) -> Result<VersionPlan, Error> {
    let mut query = config.clone();
    query.action = "getPackages".into();
    query.h5_id = Some(h5_id.to_owned());
    query.package_types = package_types.to_owned();
    query.page_size = Some(PAGE_SIZE);
    query.sub_type = None;
    let mut latest = None;
    let mut read = 0_u64;
    let mut seen_pages = HashSet::new();
    for number in 1..=MAX_PAGES {
        query.page_num = Some(number);
        info!(
            page = number,
            h5_id, package_types, "querying resource version page"
        );
        let response = client.get_packages(&query).await?;
        let current = page(&response)?;
        info!(
            page = number,
            records = current.records.len(),
            "resource version page received"
        );
        if !current.records.is_empty() {
            let bytes = serde_json::to_vec(current.records)
                .map_err(|_| Error::new("response", "cannot read resource list"))?;
            let fingerprint: [u8; 32] = Sha256::digest(bytes).into();
            if !seen_pages.insert(fingerprint) {
                return Err(Error::new(
                    "response",
                    "resource pagination repeated a page; cannot determine the latest version",
                ));
            }
        }
        for record in current.records {
            if let Some(candidate) = resource_version(record, h5_id)? {
                if latest
                    .as_ref()
                    .is_none_or(|(version, _, _)| candidate.0 > *version)
                {
                    latest = Some((candidate.0, candidate.1, record.clone()));
                }
            }
        }
        read += current.records.len() as u64;
        if let Some(total) = current.total {
            if read > total || (read < total && current.records.is_empty()) {
                return Err(Error::new(
                    "response",
                    "resource pagination does not match its total",
                ));
            }
            if read == total {
                let result = plan(latest)?;
                info!(latest_version = ?result.latest_version, next_version = %result.next_version, "resource version plan ready");
                return Ok(result);
            }
        } else if let Some(total_pages) = current.total_pages {
            if total_pages == 0 && current.records.is_empty() {
                let result = plan(latest)?;
                info!(latest_version = ?result.latest_version, next_version = %result.next_version, "resource version plan ready");
                return Ok(result);
            }
            if number > total_pages || (number < total_pages && current.records.is_empty()) {
                return Err(Error::new(
                    "response",
                    "resource pagination does not match its page count",
                ));
            }
            if number == total_pages {
                let result = plan(latest)?;
                info!(latest_version = ?result.latest_version, next_version = %result.next_version, "resource version plan ready");
                return Ok(result);
            }
        } else if (current.records.len() as u64) < current.page_size.unwrap_or(PAGE_SIZE) {
            let result = plan(latest)?;
            info!(latest_version = ?result.latest_version, next_version = %result.next_version, "resource version plan ready");
            return Ok(result);
        }
    }
    Err(Error::new(
        "response",
        "resource pagination exceeded 1000 pages; cannot determine the latest version",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;
    use rsa::{pkcs8::EncodePrivateKey, RsaPrivateKey};
    use serde_json::json;
    use std::sync::OnceLock;
    use tempfile::TempDir;
    use wiremock::{
        matchers::{method, path, query_param},
        Mock, MockServer, ResponseTemplate,
    };

    const APP: &str = "0000000000000012";

    async fn client(server: &MockServer, directory: &TempDir) -> (MpaasClient, Config) {
        static KEY: OnceLock<RsaPrivateKey> = OnceLock::new();
        let key = KEY.get_or_init(|| RsaPrivateKey::new(&mut OsRng, 2048).unwrap());
        let file = directory.path().join("key.pem");
        std::fs::write(&file, key.to_pkcs8_pem(Default::default()).unwrap()).unwrap();
        let config: Config = serde_json::from_value(json!({
            "action":"getPackages", "baseUrl":server.uri(), "appId":"test-app",
            "workspaceId":"dev", "tenantId":"tenant", "privateKeyFile":file,
            "h5Id":APP, "pageNum":99,"pageSize":1,"subType":"old-filter"
        }))
        .unwrap();
        (MpaasClient::new(&config).await.unwrap(), config)
    }

    fn success(data: Value) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(json!({"data":{"success":true,"data":data}}))
    }

    #[test]
    fn increments_patch_and_carries_at_twenty() {
        assert_eq!(Version::parse("0.0.18").unwrap().next().unwrap(), "0.0.19");
        assert_eq!(Version::parse("0.0.19").unwrap().next().unwrap(), "0.1.0");
        assert_eq!(Version::parse("7.19.19").unwrap().next().unwrap(), "7.20.0");
    }

    #[test]
    fn compares_segments_numerically() {
        assert!(Version::parse("0.10.0").unwrap() > Version::parse("0.9.19").unwrap());
        assert!(Version::parse("0.0.10").unwrap() > Version::parse("0.0.9").unwrap());
    }

    #[test]
    fn rejects_unsupported_versions_and_middle_overflow() {
        for value in ["1.0.0.1", "0.0", "0.0.20", "0.-1.0", "0.0.x", "0.0.1 "] {
            assert!(Version::parse(value).is_err(), "{value}");
        }
        assert!(Version(0, u64::MAX, 19).next().is_err());
    }

    #[test]
    fn starts_empty_history_at_zero_zero_one() {
        let value = plan(None).unwrap();
        assert!(value.latest_version.is_none());
        assert_eq!(value.next_version, "0.0.1");
        assert!(value.latest_resource.is_none());
    }

    #[test]
    fn rejects_unknown_resource_shapes_and_conflicting_versions() {
        assert!(page(&json!({"unexpected":[]})).is_err());
        assert!(page(&json!({"list":[],"total":-1})).is_err());
        assert!(resource_version(&json!({"clientVersionMin":"1.0.0"}), APP).is_err());
        assert!(resource_version(&json!({"h5Version":"0.0.1","version":"0.0.2"}), APP).is_err());
        assert_eq!(
            resource_version(&json!({"version":"0.0.2"}), APP)
                .unwrap()
                .unwrap()
                .1,
            "0.0.2"
        );
        assert!(
            resource_version(&json!({"h5Id":"other","h5Version":"legacy"}), APP)
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn reads_all_pages_before_selecting_latest() {
        let server = MockServer::start().await;
        let directory = TempDir::new().unwrap();
        let (client, config) = client(&server, &directory).await;
        let first: Vec<_> = (0..100)
            .map(|minor| json!({"h5Id":APP,"h5Version":format!("0.{minor}.0")}))
            .collect();
        for (number, data) in [
            (1, json!({"list":first,"total":101})),
            (
                2,
                json!({"list":[{"h5Id":APP,"h5Version":"0.101.19"}],"total":"101"}),
            ),
        ] {
            Mock::given(method("GET"))
                .and(path("/openapi/miniprogram/getPackages"))
                .and(query_param("pageNum", number.to_string()))
                .and(query_param("pageSize", "100"))
                .and(query_param("h5Id", APP))
                .and(query_param("packageTypes", "1"))
                .respond_with(success(data))
                .expect(1)
                .mount(&server)
                .await;
        }
        let result = next_version(&client, &config, APP, "1").await.unwrap();
        assert_eq!(result.latest_version.as_deref(), Some("0.101.19"));
        assert_eq!(result.next_version, "0.102.0");
        for request in server.received_requests().await.unwrap() {
            assert!(!request.url.query_pairs().any(|(name, _)| name == "subType"));
        }
    }

    #[tokio::test]
    async fn direct_array_selects_numeric_maximum_and_preserves_first_segment() {
        let server = MockServer::start().await;
        let directory = TempDir::new().unwrap();
        let (client, config) = client(&server, &directory).await;
        Mock::given(method("GET"))
            .and(path("/openapi/miniprogram/getPackages"))
            .respond_with(success(json!([
                {"h5Version":"2.0.9"},
                {"h5Version":"1.999.19"},
                {"id":189,"h5Version":"2.0.19","h5Name":"latest resource"},
                {"h5Version":"2.0.10"}
            ])))
            .expect(1)
            .mount(&server)
            .await;
        let result = next_version(&client, &config, APP, "1").await.unwrap();
        assert_eq!(result.latest_version.as_deref(), Some("2.0.19"));
        assert_eq!(result.next_version, "2.1.0");
        assert_eq!(result.latest_resource.as_ref().unwrap()["id"], 189);
        assert_eq!(
            result.latest_resource.as_ref().unwrap()["h5Name"],
            "latest resource"
        );
        assert_eq!(
            serde_json::to_value(&result).unwrap(),
            json!({"latestVersion":"2.0.19","nextVersion":"2.1.0"})
        );
    }

    #[tokio::test]
    async fn respects_server_page_count_and_page_size() {
        for field in ["totalPages", "pageSize"] {
            let server = MockServer::start().await;
            let directory = TempDir::new().unwrap();
            let (client, config) = client(&server, &directory).await;
            let page_count = if field == "totalPages" { 2 } else { 3 };
            for number in 1..=page_count {
                let records = match number {
                    1 => json!([{"h5Version":"0.0.1"}]),
                    2 => json!([{"h5Version":"0.0.19"}]),
                    _ => json!([]),
                };
                let mut response = json!({"records":records});
                response[field] = if field == "totalPages" {
                    json!(2)
                } else {
                    json!(1)
                };
                Mock::given(method("GET"))
                    .and(path("/openapi/miniprogram/getPackages"))
                    .and(query_param("pageNum", number.to_string()))
                    .respond_with(success(response))
                    .expect(1)
                    .mount(&server)
                    .await;
            }
            let result = next_version(&client, &config, APP, "1").await.unwrap();
            assert_eq!(result.latest_version.as_deref(), Some("0.0.19"));
            assert_eq!(result.next_version, "0.1.0");
        }
    }

    #[tokio::test]
    async fn refuses_repeated_pages_instead_of_uploading_a_guessed_version() {
        let server = MockServer::start().await;
        let directory = TempDir::new().unwrap();
        let (client, config) = client(&server, &directory).await;
        let records: Vec<_> = (0..100)
            .map(|id| json!({"id":id,"h5Version":"0.0.1"}))
            .collect();
        Mock::given(method("GET"))
            .and(path("/openapi/miniprogram/getPackages"))
            .respond_with(success(json!(records)))
            .expect(2)
            .mount(&server)
            .await;
        let error = next_version(&client, &config, APP, "1").await.unwrap_err();
        assert_eq!(error.kind, "response");
        assert!(error.message.contains("repeated a page"));
    }

    #[tokio::test]
    async fn empty_list_uses_initial_version_but_bad_existing_version_fails() {
        let server = MockServer::start().await;
        let directory = TempDir::new().unwrap();
        let (client, config) = client(&server, &directory).await;
        Mock::given(method("GET"))
            .and(path("/openapi/miniprogram/getPackages"))
            .respond_with(success(json!([])))
            .expect(1)
            .mount(&server)
            .await;
        assert_eq!(
            next_version(&client, &config, APP, "1")
                .await
                .unwrap()
                .next_version,
            "0.0.1"
        );
        server.reset().await;
        Mock::given(method("GET"))
            .and(path("/openapi/miniprogram/getPackages"))
            .respond_with(success(json!([{"h5Version":"1.0.0.1"}])))
            .expect(1)
            .mount(&server)
            .await;
        let error = next_version(&client, &config, APP, "1").await.unwrap_err();
        assert_eq!(error.kind, "version");
    }
}
