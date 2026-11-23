use crate::Error;
use serde_json::{Map, Value};

const RESOURCE_FIELDS: &[&str] = &[
    "h5Name",
    "vhost",
    "clientVersionMin",
    "clientVersionMax",
    "platform",
    "autoInstall",
    "installType",
    "enableTabBar",
    "enableOptionMenu",
    "enableKeepAlive",
    "mainUrl",
    "extendInfo",
    "suburl",
];

const APP_FIELDS: &[&str] = &[
    "h5Name",
    "vhost",
    "autoInstall",
    "installType",
    "mainUrl",
    "extendInfo",
    "subUrl",
];

const FLAGS: &[&str] = &[
    "autoInstall",
    "installType",
    "enableTabBar",
    "enableOptionMenu",
    "enableKeepAlive",
];

fn object(value: &Value, field: &str) -> Result<Map<String, Value>, Error> {
    match value {
        Value::Null => Ok(Map::new()),
        Value::Object(value) => Ok(value.clone()),
        Value::String(value) if value.trim().is_empty() => Ok(Map::new()),
        Value::String(value) => serde_json::from_str(value)
            .map_err(|_| Error::new("response", format!("{field} must contain a JSON object"))),
        _ => Err(Error::new(
            "response",
            format!("{field} must be an object or a JSON object string"),
        )),
    }
}

fn flag(value: &Value, field: &str) -> Result<Value, Error> {
    let number = match value {
        Value::Bool(value) => Some(u64::from(*value)),
        Value::String(value) if value == "0" || value == "1" => value.parse().ok(),
        Value::Number(value) => value.as_u64(),
        _ => None,
    };
    match number {
        Some(number @ 0..=1) => Ok(Value::from(number)),
        _ => Err(Error::new("response", format!("{field} must be 0 or 1"))),
    }
}

fn fields(record: &Value, allowed: &[&str]) -> Result<Map<String, Value>, Error> {
    let record = record
        .as_object()
        .ok_or_else(|| Error::new("response", "defaults source must be an object"))?;
    let mut result = Map::new();
    for name in allowed {
        let Some(value) = record.get(*name).filter(|value| !value.is_null()) else {
            continue;
        };
        let value = if FLAGS.contains(name) {
            flag(value, name)?
        } else if *name == "extendInfo" {
            Value::Object(object(value, "extendInfo")?)
        } else if value.is_string() {
            value.clone()
        } else {
            return Err(Error::new("response", format!("{name} must be a string")));
        };
        let name = if *name == "subUrl" { "suburl" } else { name };
        result.insert(name.to_string(), value);
    }
    Ok(result)
}

pub(crate) fn resource_defaults(record: &Value) -> Result<Map<String, Value>, Error> {
    let mut result = fields(record, RESOURCE_FIELDS)?;
    if let Some(extra) = record.get("extraData").filter(|value| !value.is_null()) {
        if extra.as_str().is_some_and(|value| value.trim() == "null") {
            return Ok(result);
        }
        let extra = object(extra, "resource.extraData")?;
        for name in ["enableKeepAlive", "enableOptionMenu", "enableTabBar"] {
            if !result.contains_key(name) {
                if let Some(value) = extra.get(name).filter(|value| !value.is_null()) {
                    result.insert(name.to_owned(), flag(value, name)?);
                }
            }
        }
    }
    Ok(result)
}

pub(crate) fn app_defaults(record: &Value) -> Result<Map<String, Value>, Error> {
    fields(record, APP_FIELDS)
}

pub(crate) fn merge_extend_info(history: &Value, explicit: &Value) -> Result<Value, Error> {
    if explicit.is_null() {
        return Ok(Value::Null);
    }
    let mut merged = object(history, "historical extendInfo")?;
    for (name, value) in object(explicit, "explicit extendInfo")? {
        if name == "launchParams" && value.is_object() {
            let mut params = merged
                .get(&name)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            params.extend(value.as_object().unwrap().clone());
            merged.insert(name, Value::Object(params));
        } else {
            merged.insert(name, value);
        }
    }
    Ok(Value::Object(merged))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resource_fields_exclude_server_ids_versions_urls_and_unknown_values() {
        let defaults = resource_defaults(&json!({
            "id":189,"packageId":189,"h5Id":"0000000000000012","h5Version":"0.0.1",
            "h5Name":"mini","vhost":"mini.example.test","mainUrl":"/index.html",
            "clientVersionMin":"1.0;1.0","clientVersionMax":null,"platform":"all",
            "suburl":"https://mini.example.test/description","memo":"old memo",
            "iconUrl":"https://server/icon.png","downloadUrl":"https://server/package.amr",
            "tenantId":"tenant","status":1,"unknown":"ignored"
        }))
        .unwrap();
        assert_eq!(
            Value::Object(defaults),
            json!({
                "h5Name":"mini","vhost":"mini.example.test","mainUrl":"/index.html",
                "clientVersionMin":"1.0;1.0","platform":"all",
                "suburl":"https://mini.example.test/description"
            })
        );
    }

    #[test]
    fn extra_data_only_fills_missing_flags_and_normalizes_binary_values() {
        let defaults = resource_defaults(&json!({
            "autoInstall":"1","installType":false,"enableTabBar":0,
            "extraData":"{\"enableTabBar\":\"1\",\"enableOptionMenu\":\"0\",\"enableKeepAlive\":\"1\",\"id\":99}"
        })).unwrap();
        assert_eq!(
            Value::Object(defaults),
            json!({
                "autoInstall":1,"installType":0,"enableTabBar":0,
                "enableOptionMenu":0,"enableKeepAlive":1
            })
        );
        assert_eq!(
            resource_defaults(&json!({"extraData":{"enableKeepAlive":true}})).unwrap()
                ["enableKeepAlive"],
            1
        );
        for value in [Value::Null, json!(""), json!(" null ")] {
            assert!(resource_defaults(&json!({"extraData":value}))
                .unwrap()
                .is_empty());
        }
        for value in [json!(2), json!(-1), json!("YES"), json!(0.5)] {
            assert!(resource_defaults(&json!({"enableTabBar":value})).is_err());
        }
    }

    #[test]
    fn app_fields_use_the_documented_subset_and_map_sub_url() {
        let defaults = app_defaults(&json!({
            "h5Name":"mini","vhost":"mini.example.test","autoInstall":"0","installType":1,
            "mainUrl":"/index.html","extendInfo":"{\"launchParams\":{\"page\":\"home\"}}",
            "subUrl":"https://mini.example.test/description","suburl":"ignored",
            "clientVersionMin":"ignored","platform":"ignored","enableKeepAlive":1,
            "extraData":{"enableTabBar":1},"memo":"ignored","downloadUrl":"ignored"
        }))
        .unwrap();
        assert_eq!(
            Value::Object(defaults),
            json!({
                "h5Name":"mini","vhost":"mini.example.test","autoInstall":0,"installType":1,
                "mainUrl":"/index.html","extendInfo":{"launchParams":{"page":"home"}},
                "suburl":"https://mini.example.test/description"
            })
        );
    }

    #[test]
    fn extend_info_merges_objects_and_launch_parameters_with_new_values_first() {
        let history = json!("{\"old\":true,\"shared\":\"old\",\"launchParams\":{\"page\":\"old\",\"keep\":\"YES\"}}");
        let explicit = json!({"shared":"new","launchParams":{"page":"new","added":"YES"}});
        assert_eq!(
            merge_extend_info(&history, &explicit).unwrap(),
            json!({
                "old":true,"shared":"new","launchParams":{"page":"new","keep":"YES","added":"YES"}
            })
        );
        assert_eq!(
            merge_extend_info(&history, &json!({})).unwrap(),
            json!({
                "old":true,"shared":"old","launchParams":{"page":"old","keep":"YES"}
            })
        );
        assert_eq!(
            merge_extend_info(
                &json!({"launchParams":{"old":true}}),
                &json!({"launchParams":{}})
            )
            .unwrap(),
            json!({"launchParams":{"old":true}})
        );
        assert!(merge_extend_info(&history, &Value::Null).unwrap().is_null());
        assert_eq!(
            merge_extend_info(&Value::Null, &json!("")).unwrap(),
            json!({})
        );
    }

    #[test]
    fn malformed_default_objects_are_rejected() {
        for value in [
            json!([]),
            json!(7),
            json!("[]"),
            json!("null"),
            json!("invalid"),
        ] {
            assert!(resource_defaults(&json!({"extendInfo":value})).is_err());
            assert!(merge_extend_info(&json!({}), &value).is_err());
        }
        assert!(resource_defaults(&json!({"extraData":"[]"})).is_err());
        assert!(resource_defaults(&json!({"h5Name":7})).is_err());
    }
}
