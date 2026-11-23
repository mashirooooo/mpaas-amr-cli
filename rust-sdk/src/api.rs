//! MDS 私有云接口目录及强类型客户端方法。

use crate::{models::*, Error, MpaasClient};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::path::Path;

/// OpenAPI 请求的传输方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiMethod {
    /// GET 查询参数。
    Get,
    /// POST multipart/form-data 表单，可带文件。
    Multipart,
    /// POST application/json 请求体。
    Json,
}

macro_rules! endpoints {
    ($($endpoint:ident, $name:ident, $path:literal, $method:ident,
       $request:ty => $response:ty, $about:literal, [$($file:literal),*];)*) => {
        /// SDK 支持的 MDS OpenAPI 接口。
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ApiEndpoint {
            $(#[doc = $about] $endpoint,)*
        }

        impl ApiEndpoint {
            /// 接口实际用于签名的路径。
            pub const fn path(self) -> &'static str {
                match self { $(Self::$endpoint => $path,)* }
            }

            /// 服务端要求的 HTTP 请求方式。
            pub const fn method(self) -> ApiMethod {
                match self { $(Self::$endpoint => ApiMethod::$method,)* }
            }

            /// SDK 支持的全部接口。
            pub const fn all() -> &'static [Self] {
                &[$(Self::$endpoint,)*]
            }

            /// 根据原始路径查找接口，保留大小写。
            pub fn from_path(path: &str) -> Option<Self> {
                Self::all().iter().copied().find(|endpoint| endpoint.path() == path)
            }

            pub(crate) const fn file_fields(self) -> &'static [&'static str] {
                match self { $(Self::$endpoint => &[$($file),*],)* }
            }
        }

        impl MpaasClient {
            $(
                #[doc = $about]
                #[doc = concat!("\n\n接口路径 ", $path, "。")]
                pub async fn $name(&self, request: &$request) -> Result<ApiResponse<$response>, Error> {
                    let payload = serde_json::to_value(request).map_err(request_error)?;
                    let response = self.send_endpoint(ApiEndpoint::$endpoint, payload, None).await?;
                    decode_response(response)
                }
            )*

            pub(crate) async fn call_config(
                &self,
                endpoint: ApiEndpoint,
                value: Value,
                base: Option<&Path>,
            ) -> Result<ApiResponse<Value>, Error> {
                let payload = match endpoint {
                    $(ApiEndpoint::$endpoint => normalize_request::<$request>(value)?,)*
                };
                self.send_endpoint(endpoint, payload, base).await
            }
        }
    };
}

fn request_error(error: serde_json::Error) -> Error {
    Error::new("config", format!("invalid API request: {error}"))
}

fn normalize_request<T: DeserializeOwned + serde::Serialize>(value: Value) -> Result<Value, Error> {
    let request = T::deserialize(&value).map_err(request_error)?;
    let normalized = serde_json::to_value(request).map_err(request_error)?;
    for (name, field) in value.as_object().into_iter().flatten() {
        if !field.is_null() && normalized.get(name).is_none() {
            return Err(Error::new(
                "config",
                format!("unknown API parameter: {name}"),
            ));
        }
    }
    Ok(normalized)
}

fn decode_response<T: DeserializeOwned>(
    response: ApiResponse<Value>,
) -> Result<ApiResponse<T>, Error> {
    let data = serde_json::from_value(response.data)
        .map_err(|error| Error::new("response", format!("cannot decode API response: {error}")))?;
    Ok(ApiResponse {
        data,
        request_id: response.request_id,
        pagination: response.pagination,
    })
}

endpoints! {
    MiniProgramCreateApp, mini_program_create_app, "/openapi/miniprogram/createApp", Multipart, CreateAppRequest => MutationResult, "创建小程序实例。", [];
    MiniProgramListApps, mini_program_list_apps, "/openapi/miniprogram/getAppList", Get, ListRequest => Vec<AppInfo>, "查询小程序实例列表。", [];
    MiniProgramDeleteApp, mini_program_delete_app, "/openapi/miniprogram/deleteApp", Multipart, H5IdRequest => MutationResult, "删除小程序实例。", [];
    MiniProgramUpdateName, mini_program_update_name, "/openapi/miniprogram/updateH5Name", Multipart, CreateAppRequest => MutationResult, "修改小程序名称。", [];
    MiniProgramUploadPackage, mini_program_upload_package, "/openapi/miniprogram/uploadPackage", Multipart, MiniProgramUploadRequest => MiniProgramUploadResult, "上传小程序资源包。", ["resourceFile", "iconFile", "cubeResourceFile"];
    MiniProgramListPackages, mini_program_list_packages, "/openapi/miniprogram/getPackages", Get, MiniProgramPackageListRequest => Vec<ResourceInfo>, "查询小程序资源包列表。", [];
    MiniProgramUpdatePackageType, mini_program_update_package_type, "/openapi/miniprogram/updateMiniPackageType", Multipart, UpdateMiniPackageTypeRequest => MutationResult, "修改小程序资源包类型。", [];
    MiniProgramCreateTask, mini_program_create_task, "/openapi/miniprogram/createNebulaTask", Multipart, MiniProgramPublishTaskRequest => MutationResult, "创建小程序发布任务。", [];
    MiniProgramListTasks, mini_program_list_tasks, "/openapi/miniprogram/nebulaTaskList", Get, ResourceTaskListRequest => Vec<TaskInfo>, "查询小程序发布任务列表。", [];
    MiniProgramTaskDetail, mini_program_task_detail, "/openapi/miniprogram/getTaskDetail", Get, TaskIdRequest => TaskInfo, "查询小程序任务详情。", [];
    MiniProgramChangeTaskStatus, mini_program_change_task_status, "/openapi/miniprogram/changeTaskStatus", Multipart, TaskStatusRequest => MutationResult, "修改小程序任务状态。", [];
    MiniProgramAddConfig, mini_program_add_config, "/openapi/miniprogram/addMiniprogramConfig", Multipart, MiniProgramConfigRequest => MutationResult, "新增小程序权限配置。", [];
    MiniProgramGetConfig, mini_program_get_config, "/openapi/miniprogram/getMiniprogramConfig", Get, H5IdRequest => MiniProgramConfigInfo, "查询小程序权限配置。", [];
    MiniProgramDeleteConfig, mini_program_delete_config, "/openapi/miniprogram/deleteMiniprogramConfig", Multipart, DeleteMiniProgramConfigRequest => MutationResult, "删除小程序权限配置。", [];
    MiniProgramUpdateConfig, mini_program_update_config, "/openapi/miniprogram/updateMiniprogramConfig", Multipart, UpdateMiniProgramConfigRequest => MutationResult, "更新小程序权限配置。", [];
    MiniProgramGetGreyUrl, mini_program_get_grey_url, "/openapi/miniprogram/getThirdGreyUrl", Get, EmptyRequest => String, "查询第三方灰度地址。", [];
    MiniProgramCreateGreyUrl, mini_program_create_grey_url, "/openapi/miniprogram/createThirdGreyUrl", Multipart, ThirdGreyUrlRequest => MutationResult, "创建第三方灰度地址。", [];
    MiniProgramUpdateGreyUrl, mini_program_update_grey_url, "/openapi/miniprogram/updateThirdGreyUrl", Multipart, ThirdGreyUrlRequest => MutationResult, "修改第三方灰度地址。", [];
    NebulaCreateVhost, nebula_create_vhost, "/openapi/nebula/createNebulaVhost", Multipart, VhostRequest => MutationResult, "设置离线包基础虚拟域名。", [];
    NebulaGetVhost, nebula_get_vhost, "/openapi/nebula/getNebulaVhost", Get, EmptyRequest => String, "查询离线包基础虚拟域名。", [];
    NebulaCreateRsaKey, nebula_create_rsa_key, "/openapi/nebula/createNebulaRsaKey", Multipart, NebulaRsaKeyRequest => MutationResult, "上传离线包签名私钥。", ["file"];
    NebulaGetRsaKeyStatus, nebula_get_rsa_key_status, "/openapi/nebula/getNebulaRsaKey", Get, EmptyRequest => String, "查询是否上传过离线包签名私钥，返回success或fail。", [];
    NebulaCreateApp, nebula_create_app, "/openapi/nebula/createNebulaId", Multipart, CreateAppRequest => MutationResult, "创建离线包实例。", [];
    NebulaListApps, nebula_list_apps, "/openapi/nebula/getNebulaIdList", Get, ListRequest => Vec<AppInfo>, "查询离线包实例列表。", [];
    NebulaDeleteApp, nebula_delete_app, "/openapi/nebula/deleteNebulaId", Multipart, H5IdRequest => MutationResult, "删除离线包实例。", [];
    NebulaUploadResource, nebula_upload_resource, "/openapi/nebula/createNebulaResource", Multipart, NebulaResourceRequest => MutationResult, "上传离线包资源。", ["file"];
    NebulaListResources, nebula_list_resources, "/openapi/nebula/getNebulaResourceList", Get, H5PackageListRequest => Vec<ResourceInfo>, "查询离线包资源列表。", [];
    NebulaResourceDetail, nebula_resource_detail, "/openapi/nebula/getNebulaResourceById", Get, ResourceIdRequest => ResourceInfo, "查询离线包或小程序资源详情。", [];
    NebulaCreateTask, nebula_create_task, "/openapi/nebula/createNebulaTask", Multipart, NebulaPublishTaskRequest => MutationResult, "创建离线包发布任务。", [];
    NebulaListTasks, nebula_list_tasks, "/openapi/nebula/nebulaTaskList", Get, ResourceTaskListRequest => Vec<TaskInfo>, "查询离线包发布任务列表。", [];
    NebulaTaskDetail, nebula_task_detail, "/openapi/nebula/getTaskDetail", Get, TaskIdRequest => TaskInfo, "查询离线包任务详情。", [];
    NebulaChangeTaskStatus, nebula_change_task_status, "/openapi/nebula/changeTaskStatus", Multipart, TaskStatusRequest => MutationResult, "修改离线包任务状态。", [];
    NebulaCreateAppsBatch, nebula_create_apps_batch, "/openapi/nebula/createNebulaIdBatch", Json, NebulaAppBatchRequest => Vec<BatchResult>, "批量创建离线包实例。", [];
    NebulaUploadResourcesBatch, nebula_upload_resources_batch, "/openapi/nebula/createNebulaResourceBatch", Multipart, NebulaResourceBatchRequest => Vec<BatchResult>, "批量上传离线包资源。", ["files"];
    NebulaCreateTasksBatch, nebula_create_tasks_batch, "/openapi/nebula/createNebulaTaskBatch", Json, DynamicTaskBatchRequest => Vec<BatchResult>, "批量创建离线包发布任务。", [];
    NebulaChangeTaskStatusBatch, nebula_change_task_status_batch, "/openapi/nebula/changeTaskStatusBatch", Json, TaskStatusBatchRequest => Vec<BatchResult>, "批量修改离线包任务状态。", [];
    CustomCreateApp, custom_create_app, "/openapi/customResource/createCustomApp", Multipart, CreateAppRequest => MutationResult, "创建自定义资源包实例。", [];
    CustomListApps, custom_list_apps, "/openapi/customResource/getCustomAppList", Get, ListRequest => Vec<AppInfo>, "查询自定义资源包实例列表。", [];
    CustomDeleteApp, custom_delete_app, "/openapi/customResource/deleteCustomApp", Multipart, H5IdRequest => MutationResult, "删除自定义资源包实例。", [];
    CustomUploadPackage, custom_upload_package, "/openapi/customResource/uploadPackage", Multipart, CustomResourceUploadRequest => MutationResult, "上传自定义资源包。", ["file"];
    CustomListPackages, custom_list_packages, "/openapi/customResource/getPackages", Get, H5PackageListRequest => Vec<ResourceInfo>, "查询自定义资源包列表。", [];
    CustomResourceDetail, custom_resource_detail, "/openapi/customResource/getCustomResourceById", Get, ResourceIdRequest => ResourceInfo, "查询自定义资源包详情。", [];
    CustomCreateTask, custom_create_task, "/openapi/customResource/createPublishTask", Multipart, CustomPublishTaskRequest => MutationResult, "创建自定义资源包发布任务。", [];
    CustomListTasks, custom_list_tasks, "/openapi/customResource/getCustomTaskList", Get, ResourceTaskListRequest => Vec<TaskInfo>, "查询自定义资源包发布任务列表。", [];
    CustomTaskDetail, custom_task_detail, "/openapi/customResource/getTaskDetail", Get, TaskIdRequest => TaskInfo, "查询自定义资源包任务详情。", [];
    CustomChangeTaskStatus, custom_change_task_status, "/openapi/customResource/changeTaskStatus", Multipart, TaskStatusRequest => MutationResult, "修改自定义资源包任务状态。", [];
    HotpatchUploadResource, hotpatch_upload_resource, "/openapi/hotpatch/createHotpatchResource", Multipart, HotpatchResourceRequest => MutationResult, "上传热修复资源。", ["resourceFile"];
    HotpatchListResources, hotpatch_list_resources, "/openapi/hotpatch/getHotpatchResourceList", Get, ListRequest => Vec<HotpatchResourceInfo>, "查询热修复资源列表。", [];
    HotpatchDeleteResource, hotpatch_delete_resource, "/openapi/hotpatch/deleteHotpatchResource", Multipart, DeleteHotpatchResourceRequest => MutationResult, "删除热修复资源。", [];
    HotpatchCreateTask, hotpatch_create_task, "/openapi/hotpatch/createHotpatchTask", Multipart, HotpatchTaskRequest => MutationResult, "创建热修复发布任务。", [];
    HotpatchListTasks, hotpatch_list_tasks, "/openapi/hotpatch/hotpatchTaskList", Get, ResourceTaskListRequest => Vec<TaskInfo>, "查询热修复发布任务列表。", [];
    HotpatchTaskDetail, hotpatch_task_detail, "/openapi/hotpatch/getTaskDetail", Get, TaskIdRequest => TaskInfo, "查询热修复任务详情。", [];
    HotpatchChangeTaskStatus, hotpatch_change_task_status, "/openapi/hotpatch/changeTaskStatus", Multipart, TaskStatusRequest => MutationResult, "修改热修复任务状态。", [];
    HotpatchCreateRollbackTask, hotpatch_create_rollback_task, "/openapi/hotpatch/createRollbackTask", Multipart, HotpatchRollbackRequest => MutationResult, "创建热修复回滚任务。", [];
    UpgradeListResources, upgrade_list_resources, "/openapi/upgrade/getUpgradeList", Get, ListRequest => Vec<UpgradeInfo>, "查询升级资源列表。", [];
    UpgradeUploadPackage, upgrade_upload_package, "/openapi/upgrade/createUpgradPackage", Multipart, UpgradePackageRequest => MutationResult, "创建升级资源。", ["file", "android64File", "iconFile"];
    UpgradeUploadIosSymbol, upgrade_upload_ios_symbol, "/openapi/upgrade/createIOSsymbol", Multipart, UpgradeSymbolRequest => MutationResult, "上传iOS符号表。", ["iosSymbolfile"];
    UpgradeResourceDetail, upgrade_resource_detail, "/openapi/upgrade/getUpgradePackageInfoById", Get, PackageIdRequest => UpgradeDetail, "查询升级资源详情。", [];
    UpgradeDeleteResource, upgrade_delete_resource, "/openapi/upgrade/deletePublishResource", Multipart, UpgradePlatformResourceRequest => MutationResult, "删除升级资源。", [];
    UpgradeShowCheckCode, upgrade_show_check_code, "/openapi/upgrade/showCheckCode", Get, UpgradePlatformResourceRequest => String, "查询升级包扫码下载验证码。", [];
    UpgradeListTasks, upgrade_list_tasks, "/openapi/upgrade/getUpgradeTaskListByPackageId", Get, UpgradeTaskListRequest => Vec<UpgradeTaskInfo>, "查询升级包发布任务列表。", [];
    UpgradeCreateTask, upgrade_create_task, "/openapi/upgrade/createMobileAppPublishTask", Multipart, UpgradePublishTaskRequest => MutationResult, "创建升级包发布任务。", [];
    UpgradeTaskDetail, upgrade_task_detail, "/openapi/upgrade/getTaskDetail", Get, TaskIdRequest => UpgradeTaskInfo, "查询升级包任务详情。", [];
    UpgradeChangeTaskStatus, upgrade_change_task_status, "/openapi/upgrade/changeTaskStatus", Multipart, UpgradeTaskStatusRequest => MutationResult, "修改升级包任务状态。", [];
    WhitelistCreate, whitelist_create, "/openapi/whitelist/newWhiteList", Multipart, NewWhitelistRequest => MutationResult, "创建发布白名单。", [];
    WhitelistAddEntries, whitelist_add_entries, "/openapi/whitelist/modifyWhiteListKeyIds", Multipart, ModifyWhitelistRequest => WhitelistModifyResult, "添加白名单条目。", ["keyIdsFile"];
    WhitelistList, whitelist_list, "/openapi/whitelist/getWhiteList", Get, ListRequest => Vec<WhitelistInfo>, "查询白名单列表。", [];
    WhitelistDelete, whitelist_delete, "/openapi/whitelist/deleteWhiteList", Multipart, ResourceIdRequest => MutationResult, "删除发布白名单。", [];
    WhitelistDetail, whitelist_detail, "/openapi/whitelist/queryMoreWhitelistDetail", Multipart, WhitelistDetailRequest => WhitelistDetail, "查询白名单内容。", [];
    WhitelistDeleteEntries, whitelist_delete_entries, "/openapi/whitelist/deleteWhitelistContent", Multipart, DeleteWhitelistContentRequest => MutationResult, "删除白名单条目。", [];
    WhitelistUpdateName, whitelist_update_name, "/openapi/whitelist/updateWhitelist", Multipart, UpdateWhitelistRequest => MutationResult, "更新白名单名称。", [];
    ResourceInitRules, resource_init_rules, "/openapi/resource/initResourceConfig", Multipart, EmptyRequest => MutationResult, "初始化预置发布规则，仅当现有规则为空时生效。", [];
    ResourceInsertRule, resource_insert_rule, "/openapi/resource/insertResourceConfig", Multipart, ResourceConfigRequest => MutationResult, "添加发布规则。", [];
    ResourceListRules, resource_list_rules, "/openapi/resource/getResourceConfigList", Get, ListRequest => Vec<ResourceConfigInfo>, "查询发布规则列表。", [];
    ResourceDeleteRules, resource_delete_rules, "/openapi/resource/deleteResourceBatch", Multipart, DeleteResourceBatchRequest => MutationResult, "批量删除发布规则。", [];
    ResourceUpdateRule, resource_update_rule, "/openapi/resource/updateProductResourceConfig", Multipart, UpdateResourceConfigRequest => MutationResult, "更新发布规则。", [];
    PrivilegeAssignUsers, privilege_assign_users, "/openapi/privilege/assignUsers", Multipart, PrivilegeRequest => MutationResult, "分配用户与资源关系。", [];
    PrivilegeDeleteResource, privilege_delete_resource, "/openapi/privilege/deleteResource", Multipart, PrivilegeRequest => MutationResult, "移除用户与资源关系。", [];
    OpenPlatformPut, open_platform_put, "/openapi/openPlatform/putOpenPlatformInfo", Multipart, OpenPlatformRequest => MutationResult, "写入开放平台信息。", [];
    OpenPlatformGet, open_platform_get, "/openapi/openPlatform/getOpenPlatformInfo", Get, EmptyRequest => Option<OpenPlatformInfo>, "查询开放平台信息。", [];
    SwitchCreate, switch_create, "/openapi/clientswitch/saveSwitchInfo", Multipart, SwitchInfoForm => MutationResult, "创建客户端开关。", [];
    SwitchUpdate, switch_update, "/openapi/clientswitch/updateSwitchInfo", Multipart, SwitchInfoForm => MutationResult, "更新客户端开关。", [];
    SwitchList, switch_list, "/openapi/clientswitch/queryClientSwitchList", Get, SwitchListRequest => SwitchListResult, "查询客户端开关列表。", [];
    SwitchDetail, switch_detail, "/openapi/clientswitch/getSwitchConf", Get, ResourceIdRequest => SwitchRecord, "查询客户端开关详情。", [];
    SwitchSetStatus, switch_set_status, "/openapi/clientswitch/toggleSwitchStatus", Multipart, SwitchStatusRequest => MutationResult, "修改客户端开关状态。", [];
    SwitchImport, switch_import, "/openapi/clientswitch/importConfs", Multipart, SwitchImportRequest => Vec<SwitchImportResult>, "导入客户端开关配置。", ["file"];
    SwitchExport, switch_export, "/openapi/clientswitch/exportConfs", Multipart, SwitchExportRequest => String, "导出客户端开关配置并返回下载地址。", [];
    SwitchCreateLegacy, switch_create_legacy, "/openapi/clientSwitch/addSwitchInfo", Multipart, SwitchInfoForm => MutationResult, "创建客户端开关的Java示例路径。", [];
    SwitchUpdateLegacy, switch_update_legacy, "/openapi/clientSwitch/updateSwitchInfo", Multipart, SwitchInfoForm => MutationResult, "更新客户端开关的Java示例路径。", [];
    SwitchListLegacy, switch_list_legacy, "/openapi/clientSwitch/getSwitchInfoList", Get, SwitchListRequest => SwitchListResult, "查询客户端开关列表的Java示例路径。", [];
    SwitchDetailLegacy, switch_detail_legacy, "/openapi/clientSwitch/getSwitchInfoDetail", Get, ResourceIdRequest => SwitchRecord, "查询客户端开关详情的Java示例路径。", [];
    SwitchSetStatusLegacy, switch_set_status_legacy, "/openapi/clientSwitch/changeSwitchStatus", Multipart, SwitchStatusRequest => MutationResult, "修改客户端开关状态的Java示例路径。", [];
    SwitchImportLegacy, switch_import_legacy, "/openapi/clientSwitch/importConfs", Multipart, SwitchImportRequest => Vec<SwitchImportResult>, "导入客户端开关配置的Java示例路径。", ["file"];
    SwitchExportLegacy, switch_export_legacy, "/openapi/clientSwitch/exportConfs", Multipart, SwitchExportRequest => String, "导出客户端开关配置的Java示例路径。", [];
}

pub(crate) fn validate_request(
    endpoint: ApiEndpoint,
    fields: &Map<String, Value>,
) -> Result<(), Error> {
    use ApiEndpoint::*;

    if let Some(id) = fields.get("h5Id").and_then(Value::as_str) {
        let digits = match endpoint {
            MiniProgramCreateApp
            | MiniProgramUpdateName
            | MiniProgramDeleteApp
            | MiniProgramUploadPackage
            | MiniProgramListPackages
            | MiniProgramUpdatePackageType
            | MiniProgramAddConfig
            | MiniProgramGetConfig
            | MiniProgramUpdateConfig
            | MiniProgramDeleteConfig => 16,
            NebulaCreateApp | NebulaDeleteApp | NebulaUploadResource | NebulaListResources => 8,
            CustomCreateApp | CustomDeleteApp | CustomUploadPackage | CustomListPackages => 6,
            _ => unreachable!("endpoint has no top-level h5Id request field"),
        };
        require_app_id(id, digits)?;
    }
    for name in ["pageNum", "pageSize"] {
        if fields.get(name).and_then(Value::as_u64) == Some(0) {
            return Err(Error::new("config", format!("{name} must be positive")));
        }
    }

    match endpoint {
        MiniProgramCreateTask
        | NebulaCreateTask
        | CustomCreateTask
        | HotpatchCreateTask
        | UpgradeCreateTask => validate_publish(endpoint, fields),
        MiniProgramChangeTaskStatus
        | NebulaChangeTaskStatus
        | CustomChangeTaskStatus
        | HotpatchChangeTaskStatus
        | UpgradeChangeTaskStatus => {
            let business = match endpoint {
                HotpatchChangeTaskStatus => Some("hotpatch"),
                UpgradeChangeTaskStatus => None,
                _ => Some("nebula"),
            };
            if let Some(business) = business {
                if fields.get("bizType").and_then(Value::as_str) != Some(business) {
                    return Err(Error::new("config", format!("bizType must be {business}")));
                }
            }
            validate_status(fields.get("taskStatus"))
        }
        MiniProgramUploadPackage => {
            if fields.get("resourceType").and_then(Value::as_u64) != Some(4) {
                return Err(Error::new(
                    "config",
                    "resourceType must be 4 for a mini-program",
                ));
            }
            Ok(())
        }
        MiniProgramUpdatePackageType => {
            if fields["packageType"].as_u64() != Some(1) {
                return Err(Error::new(
                    "config",
                    "packageType must be 1 for this endpoint",
                ));
            }
            Ok(())
        }
        NebulaUploadResource => {
            if fields.get("resourceType").and_then(Value::as_u64) == Some(0) {
                require_fields(fields, &["repeatNebula"])?;
            }
            Ok(())
        }
        NebulaCreateAppsBatch => {
            for item in batch_items(fields, "nebulaAppRequestList")? {
                require_app_id(item["h5Id"].as_str().unwrap_or_default(), 8)?;
            }
            Ok(())
        }
        NebulaUploadResourcesBatch => {
            let config: Map<String, Value> =
                serde_json::from_str(fields["configString"].as_str().unwrap_or_default())
                    .map_err(request_error)?;
            let items = batch_items(&config, "nebulaResourceList")?;
            if fields["files"].as_array().map(Vec::len) != Some(items.len()) {
                return Err(Error::new(
                    "config",
                    "files must contain one ZIP per nebulaResourceList item",
                ));
            }
            for item in items {
                let item = item.as_object().expect("serde checked batch item");
                require_fields(item, &["h5Id", "h5Version"])?;
                require_app_id(item["h5Id"].as_str().unwrap_or_default(), 8)?;
            }
            Ok(())
        }
        NebulaCreateTasksBatch => {
            for item in batch_items(fields, "taskList")? {
                let mut effective = fields
                    .get("sharedConfig")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                effective.extend(item.as_object().expect("serde checked batch item").clone());
                require_fields(&effective, &["packageId", "publishType"])?;
                validate_publish(endpoint, &effective)?;
            }
            Ok(())
        }
        NebulaChangeTaskStatusBatch => {
            for item in batch_items(fields, "taskChangeList")? {
                let item = item.as_object().expect("serde checked batch item");
                require_fields(item, &["packageId", "taskId"])?;
                validate_status(item.get("taskStatus").or_else(|| {
                    fields
                        .get("sharedConfig")
                        .and_then(|shared| shared.get("taskStatus"))
                }))?;
            }
            Ok(())
        }
        UpgradeUploadPackage => {
            let platform = fields["platform"].as_str().unwrap_or_default();
            if platform.eq_ignore_ascii_case("android") {
                require_fields(fields, &["file", "needCheck"])
            } else if platform.eq_ignore_ascii_case("ios") {
                match fields.get("isEnterprise").and_then(Value::as_u64) {
                    Some(0) => require_fields(fields, &["appstoreUrl"]),
                    Some(1) => {
                        require_fields(fields, &["file", "iconFile", "bundleId", "needCheck"])
                    }
                    Some(2) => {
                        require_fields(fields, &["appstoreUrl", "validDays", "installAmount"])
                    }
                    _ => Err(Error::new(
                        "config",
                        "isEnterprise must be 0, 1 or 2 for iOS",
                    )),
                }
            } else {
                Err(Error::new("config", "platform must be Android or iOS"))
            }
        }
        WhitelistAddEntries => {
            let entries = fields
                .get("keyIds")
                .and_then(Value::as_str)
                .is_some_and(|text| !text.trim().is_empty());
            if entries == fields.contains_key("keyIdsFile") {
                return Err(Error::new(
                    "config",
                    "provide exactly one of keyIds or keyIdsFile",
                ));
            }
            Ok(())
        }
        SwitchImport | SwitchImportLegacy => {
            if Path::new(fields["file"].as_str().unwrap_or_default())
                .extension()
                .and_then(|ext| ext.to_str())
                != Some("config")
            {
                return Err(Error::new(
                    "config",
                    "the switch import file must end in .config",
                ));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn require_app_id(id: &str, digits: usize) -> Result<(), Error> {
    if id.len() != digits || !id.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Error::new(
            "config",
            format!("h5Id must contain {digits} digits"),
        ));
    }
    Ok(())
}

fn batch_items<'a>(fields: &'a Map<String, Value>, key: &str) -> Result<&'a [Value], Error> {
    fields
        .get(key)
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .map(Vec::as_slice)
        .ok_or_else(|| Error::new("config", format!("{key} must be a nonempty array")))
}

fn require_fields(fields: &Map<String, Value>, names: &[&str]) -> Result<(), Error> {
    for name in names {
        if fields.get(*name).is_none_or(|value| {
            value.is_null() || value.as_str().is_some_and(|value| value.trim().is_empty())
        }) {
            return Err(Error::new(
                "config",
                format!("{name} is required for this request"),
            ));
        }
    }
    Ok(())
}

fn validate_status(status: Option<&Value>) -> Result<(), Error> {
    if !matches!(status.and_then(Value::as_u64), Some(0..=3)) {
        return Err(Error::new("config", "taskStatus must be 0, 1, 2 or 3"));
    }
    Ok(())
}

fn validate_publish(endpoint: ApiEndpoint, fields: &Map<String, Value>) -> Result<(), Error> {
    let mode = fields.get("publishMode").and_then(Value::as_u64);
    if fields.get("id").is_some_and(|id| id.as_u64() != Some(0)) {
        return Err(Error::new("config", "id must be 0 when creating a task"));
    }
    match (fields.get("publishType").and_then(Value::as_u64), mode) {
        (Some(3), None) => Ok(()),
        (Some(3), Some(3)) if endpoint == ApiEndpoint::NebulaCreateTasksBatch => Ok(()),
        (Some(2), Some(1)) => require_fields(fields, &["whitelistIds"]),
        (Some(2), Some(2)) => {
            require_fields(fields, &["greyEndtimeData", "greyNum"])?;
            if fields["greyNum"].as_u64() == Some(0) {
                return Err(Error::new("config", "greyNum must be positive"));
            }
            Ok(())
        }
        (Some(2), Some(5)) if endpoint == ApiEndpoint::MiniProgramCreateTask => Ok(()),
        _ => Err(Error::new(
            "config",
            "publishType and publishMode do not match this endpoint",
        )),
    }
}
