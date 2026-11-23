# MDS 私有云 OpenAPI

Rust SDK 的请求和响应类型使用 serde。请求类型中的必填字段使用普通字段，可省略和条件必填字段使用 `Option`。源码中的中文文档注释保留字段取值、格式、适用版本和条件。

```rust
use mpaas_openapi_rs::{Config, MpaasClient, models::ListRequest};

async fn list_apps(config: &Config) -> Result<(), mpaas_openapi_rs::Error> {
    let client = MpaasClient::new(config).await?;
    let response = client.mini_program_list_apps(&ListRequest {
        page_num: Some(1),
        page_size: Some(20),
    }).await?;
    println!("{:?}", response.data);
    Ok(())
}
```

`ApiResponse<T>` 提供业务数据、请求 ID 和响应外层的分页字段。JSON 字符串字段使用 `JsonText<T>`，文件字段使用本地路径。

CLI 配置使用 `action: "api"` 和 `api.path`。按对应请求模型填写 `api.params`，文件路径相对于配置文件目录解析。SDK 根据接口确定传输方式。

| 接口路径 | 请求方式 | Rust 方法 | 请求模型 | 响应数据类型 |
| --- | --- | --- | --- | --- |
| `/openapi/miniprogram/createApp` | POST multipart | `mini_program_create_app` | `CreateAppRequest` | `MutationResult` |
| `/openapi/miniprogram/getAppList` | GET | `mini_program_list_apps` | `ListRequest` | `Vec<AppInfo>` |
| `/openapi/miniprogram/deleteApp` | POST multipart | `mini_program_delete_app` | `H5IdRequest` | `MutationResult` |
| `/openapi/miniprogram/updateH5Name` | POST multipart | `mini_program_update_name` | `CreateAppRequest` | `MutationResult` |
| `/openapi/miniprogram/uploadPackage` | POST multipart | `mini_program_upload_package` | `MiniProgramUploadRequest` | `MiniProgramUploadResult` |
| `/openapi/miniprogram/getPackages` | GET | `mini_program_list_packages` | `MiniProgramPackageListRequest` | `Vec<ResourceInfo>` |
| `/openapi/miniprogram/updateMiniPackageType` | POST multipart | `mini_program_update_package_type` | `UpdateMiniPackageTypeRequest` | `MutationResult` |
| `/openapi/miniprogram/createNebulaTask` | POST multipart | `mini_program_create_task` | `MiniProgramPublishTaskRequest` | `MutationResult` |
| `/openapi/miniprogram/nebulaTaskList` | GET | `mini_program_list_tasks` | `ResourceTaskListRequest` | `Vec<TaskInfo>` |
| `/openapi/miniprogram/getTaskDetail` | GET | `mini_program_task_detail` | `TaskIdRequest` | `TaskInfo` |
| `/openapi/miniprogram/changeTaskStatus` | POST multipart | `mini_program_change_task_status` | `TaskStatusRequest` | `MutationResult` |
| `/openapi/miniprogram/addMiniprogramConfig` | POST multipart | `mini_program_add_config` | `MiniProgramConfigRequest` | `MutationResult` |
| `/openapi/miniprogram/getMiniprogramConfig` | GET | `mini_program_get_config` | `H5IdRequest` | `MiniProgramConfigInfo` |
| `/openapi/miniprogram/deleteMiniprogramConfig` | POST multipart | `mini_program_delete_config` | `DeleteMiniProgramConfigRequest` | `MutationResult` |
| `/openapi/miniprogram/updateMiniprogramConfig` | POST multipart | `mini_program_update_config` | `UpdateMiniProgramConfigRequest` | `MutationResult` |
| `/openapi/miniprogram/getThirdGreyUrl` | GET | `mini_program_get_grey_url` | `EmptyRequest` | `String` |
| `/openapi/miniprogram/createThirdGreyUrl` | POST multipart | `mini_program_create_grey_url` | `ThirdGreyUrlRequest` | `MutationResult` |
| `/openapi/miniprogram/updateThirdGreyUrl` | POST multipart | `mini_program_update_grey_url` | `ThirdGreyUrlRequest` | `MutationResult` |
| `/openapi/nebula/createNebulaVhost` | POST multipart | `nebula_create_vhost` | `VhostRequest` | `MutationResult` |
| `/openapi/nebula/getNebulaVhost` | GET | `nebula_get_vhost` | `EmptyRequest` | `String` |
| `/openapi/nebula/createNebulaRsaKey` | POST multipart | `nebula_create_rsa_key` | `NebulaRsaKeyRequest` | `MutationResult` |
| `/openapi/nebula/getNebulaRsaKey` | GET | `nebula_get_rsa_key_status` | `EmptyRequest` | `String` |
| `/openapi/nebula/createNebulaId` | POST multipart | `nebula_create_app` | `CreateAppRequest` | `MutationResult` |
| `/openapi/nebula/getNebulaIdList` | GET | `nebula_list_apps` | `ListRequest` | `Vec<AppInfo>` |
| `/openapi/nebula/deleteNebulaId` | POST multipart | `nebula_delete_app` | `H5IdRequest` | `MutationResult` |
| `/openapi/nebula/createNebulaResource` | POST multipart | `nebula_upload_resource` | `NebulaResourceRequest` | `MutationResult` |
| `/openapi/nebula/getNebulaResourceList` | GET | `nebula_list_resources` | `H5PackageListRequest` | `Vec<ResourceInfo>` |
| `/openapi/nebula/getNebulaResourceById` | GET | `nebula_resource_detail` | `ResourceIdRequest` | `ResourceInfo` |
| `/openapi/nebula/createNebulaTask` | POST multipart | `nebula_create_task` | `NebulaPublishTaskRequest` | `MutationResult` |
| `/openapi/nebula/nebulaTaskList` | GET | `nebula_list_tasks` | `ResourceTaskListRequest` | `Vec<TaskInfo>` |
| `/openapi/nebula/getTaskDetail` | GET | `nebula_task_detail` | `TaskIdRequest` | `TaskInfo` |
| `/openapi/nebula/changeTaskStatus` | POST multipart | `nebula_change_task_status` | `TaskStatusRequest` | `MutationResult` |
| `/openapi/nebula/createNebulaIdBatch` | POST JSON | `nebula_create_apps_batch` | `NebulaAppBatchRequest` | `Vec<BatchResult>` |
| `/openapi/nebula/createNebulaResourceBatch` | POST multipart | `nebula_upload_resources_batch` | `NebulaResourceBatchRequest` | `Vec<BatchResult>` |
| `/openapi/nebula/createNebulaTaskBatch` | POST JSON | `nebula_create_tasks_batch` | `DynamicTaskBatchRequest` | `Vec<BatchResult>` |
| `/openapi/nebula/changeTaskStatusBatch` | POST JSON | `nebula_change_task_status_batch` | `TaskStatusBatchRequest` | `Vec<BatchResult>` |
| `/openapi/customResource/createCustomApp` | POST multipart | `custom_create_app` | `CreateAppRequest` | `MutationResult` |
| `/openapi/customResource/getCustomAppList` | GET | `custom_list_apps` | `ListRequest` | `Vec<AppInfo>` |
| `/openapi/customResource/deleteCustomApp` | POST multipart | `custom_delete_app` | `H5IdRequest` | `MutationResult` |
| `/openapi/customResource/uploadPackage` | POST multipart | `custom_upload_package` | `CustomResourceUploadRequest` | `MutationResult` |
| `/openapi/customResource/getPackages` | GET | `custom_list_packages` | `H5PackageListRequest` | `Vec<ResourceInfo>` |
| `/openapi/customResource/getCustomResourceById` | GET | `custom_resource_detail` | `ResourceIdRequest` | `ResourceInfo` |
| `/openapi/customResource/createPublishTask` | POST multipart | `custom_create_task` | `CustomPublishTaskRequest` | `MutationResult` |
| `/openapi/customResource/getCustomTaskList` | GET | `custom_list_tasks` | `ResourceTaskListRequest` | `Vec<TaskInfo>` |
| `/openapi/customResource/getTaskDetail` | GET | `custom_task_detail` | `TaskIdRequest` | `TaskInfo` |
| `/openapi/customResource/changeTaskStatus` | POST multipart | `custom_change_task_status` | `TaskStatusRequest` | `MutationResult` |
| `/openapi/hotpatch/createHotpatchResource` | POST multipart | `hotpatch_upload_resource` | `HotpatchResourceRequest` | `MutationResult` |
| `/openapi/hotpatch/getHotpatchResourceList` | GET | `hotpatch_list_resources` | `ListRequest` | `Vec<HotpatchResourceInfo>` |
| `/openapi/hotpatch/deleteHotpatchResource` | POST multipart | `hotpatch_delete_resource` | `DeleteHotpatchResourceRequest` | `MutationResult` |
| `/openapi/hotpatch/createHotpatchTask` | POST multipart | `hotpatch_create_task` | `HotpatchTaskRequest` | `MutationResult` |
| `/openapi/hotpatch/hotpatchTaskList` | GET | `hotpatch_list_tasks` | `ResourceTaskListRequest` | `Vec<TaskInfo>` |
| `/openapi/hotpatch/getTaskDetail` | GET | `hotpatch_task_detail` | `TaskIdRequest` | `TaskInfo` |
| `/openapi/hotpatch/changeTaskStatus` | POST multipart | `hotpatch_change_task_status` | `TaskStatusRequest` | `MutationResult` |
| `/openapi/hotpatch/createRollbackTask` | POST multipart | `hotpatch_create_rollback_task` | `HotpatchRollbackRequest` | `MutationResult` |
| `/openapi/upgrade/getUpgradeList` | GET | `upgrade_list_resources` | `ListRequest` | `Vec<UpgradeInfo>` |
| `/openapi/upgrade/createUpgradPackage` | POST multipart | `upgrade_upload_package` | `UpgradePackageRequest` | `MutationResult` |
| `/openapi/upgrade/createIOSsymbol` | POST multipart | `upgrade_upload_ios_symbol` | `UpgradeSymbolRequest` | `MutationResult` |
| `/openapi/upgrade/getUpgradePackageInfoById` | GET | `upgrade_resource_detail` | `PackageIdRequest` | `UpgradeDetail` |
| `/openapi/upgrade/deletePublishResource` | POST multipart | `upgrade_delete_resource` | `UpgradePlatformResourceRequest` | `MutationResult` |
| `/openapi/upgrade/showCheckCode` | GET | `upgrade_show_check_code` | `UpgradePlatformResourceRequest` | `String` |
| `/openapi/upgrade/getUpgradeTaskListByPackageId` | GET | `upgrade_list_tasks` | `UpgradeTaskListRequest` | `Vec<UpgradeTaskInfo>` |
| `/openapi/upgrade/createMobileAppPublishTask` | POST multipart | `upgrade_create_task` | `UpgradePublishTaskRequest` | `MutationResult` |
| `/openapi/upgrade/getTaskDetail` | GET | `upgrade_task_detail` | `TaskIdRequest` | `UpgradeTaskInfo` |
| `/openapi/upgrade/changeTaskStatus` | POST multipart | `upgrade_change_task_status` | `UpgradeTaskStatusRequest` | `MutationResult` |
| `/openapi/whitelist/newWhiteList` | POST multipart | `whitelist_create` | `NewWhitelistRequest` | `MutationResult` |
| `/openapi/whitelist/modifyWhiteListKeyIds` | POST multipart | `whitelist_add_entries` | `ModifyWhitelistRequest` | `WhitelistModifyResult` |
| `/openapi/whitelist/getWhiteList` | GET | `whitelist_list` | `ListRequest` | `Vec<WhitelistInfo>` |
| `/openapi/whitelist/deleteWhiteList` | POST multipart | `whitelist_delete` | `ResourceIdRequest` | `MutationResult` |
| `/openapi/whitelist/queryMoreWhitelistDetail` | POST multipart | `whitelist_detail` | `WhitelistDetailRequest` | `WhitelistDetail` |
| `/openapi/whitelist/deleteWhitelistContent` | POST multipart | `whitelist_delete_entries` | `DeleteWhitelistContentRequest` | `MutationResult` |
| `/openapi/whitelist/updateWhitelist` | POST multipart | `whitelist_update_name` | `UpdateWhitelistRequest` | `MutationResult` |
| `/openapi/resource/initResourceConfig` | POST multipart | `resource_init_rules` | `EmptyRequest` | `MutationResult` |
| `/openapi/resource/insertResourceConfig` | POST multipart | `resource_insert_rule` | `ResourceConfigRequest` | `MutationResult` |
| `/openapi/resource/getResourceConfigList` | GET | `resource_list_rules` | `ListRequest` | `Vec<ResourceConfigInfo>` |
| `/openapi/resource/deleteResourceBatch` | POST multipart | `resource_delete_rules` | `DeleteResourceBatchRequest` | `MutationResult` |
| `/openapi/resource/updateProductResourceConfig` | POST multipart | `resource_update_rule` | `UpdateResourceConfigRequest` | `MutationResult` |
| `/openapi/privilege/assignUsers` | POST multipart | `privilege_assign_users` | `PrivilegeRequest` | `MutationResult` |
| `/openapi/privilege/deleteResource` | POST multipart | `privilege_delete_resource` | `PrivilegeRequest` | `MutationResult` |
| `/openapi/openPlatform/putOpenPlatformInfo` | POST multipart | `open_platform_put` | `OpenPlatformRequest` | `MutationResult` |
| `/openapi/openPlatform/getOpenPlatformInfo` | GET | `open_platform_get` | `EmptyRequest` | `Option<OpenPlatformInfo>` |
| `/openapi/clientswitch/saveSwitchInfo` | POST multipart | `switch_create` | `SwitchInfoForm` | `MutationResult` |
| `/openapi/clientswitch/updateSwitchInfo` | POST multipart | `switch_update` | `SwitchInfoForm` | `MutationResult` |
| `/openapi/clientswitch/queryClientSwitchList` | GET | `switch_list` | `SwitchListRequest` | `SwitchListResult` |
| `/openapi/clientswitch/getSwitchConf` | GET | `switch_detail` | `ResourceIdRequest` | `SwitchRecord` |
| `/openapi/clientswitch/toggleSwitchStatus` | POST multipart | `switch_set_status` | `SwitchStatusRequest` | `MutationResult` |
| `/openapi/clientswitch/importConfs` | POST multipart | `switch_import` | `SwitchImportRequest` | `Vec<SwitchImportResult>` |
| `/openapi/clientswitch/exportConfs` | POST multipart | `switch_export` | `SwitchExportRequest` | `String` |
| `/openapi/clientSwitch/addSwitchInfo` | POST multipart | `switch_create_legacy` | `SwitchInfoForm` | `MutationResult` |
| `/openapi/clientSwitch/updateSwitchInfo` | POST multipart | `switch_update_legacy` | `SwitchInfoForm` | `MutationResult` |
| `/openapi/clientSwitch/getSwitchInfoList` | GET | `switch_list_legacy` | `SwitchListRequest` | `SwitchListResult` |
| `/openapi/clientSwitch/getSwitchInfoDetail` | GET | `switch_detail_legacy` | `ResourceIdRequest` | `SwitchRecord` |
| `/openapi/clientSwitch/changeSwitchStatus` | POST multipart | `switch_set_status_legacy` | `SwitchStatusRequest` | `MutationResult` |
| `/openapi/clientSwitch/importConfs` | POST multipart | `switch_import_legacy` | `SwitchImportRequest` | `Vec<SwitchImportResult>` |
| `/openapi/clientSwitch/exportConfs` | POST multipart | `switch_export_legacy` | `SwitchExportRequest` | `String` |
