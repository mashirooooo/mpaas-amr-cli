//! MDS 私有云 OpenAPI 的请求和响应模型。
//!
//! 请求字段使用原始接口拼写。非 Option 字段为必填，Option 字段可省略或按注释的条件填写。
//! 必填的 appId、workspaceId、tenantId 取自客户端配置。签名和时间戳由客户端生成。
//! 文件字段填写本地路径，由客户端读取并作为 multipart 附件发送。
//! 响应的 Option 字段允许服务端省略或返回 null，例如不同产品集和 mcube 版本的专属字段。

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf};

/// 兼容文档中整数和数字字符串两种表示。保留接口返回的原始表示。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Integer {
    /// JSON 整数。
    Number(u64),
    /// 数字字符串。少数旧版响应使用空字符串作为占位值。
    Text(String),
}

impl From<u64> for Integer {
    fn from(value: u64) -> Self {
        Self::Number(value)
    }
}

/// 接口声明为 JSON 字符串的字段。也接受 JSON 对象或数组作为配置文件输入。
#[derive(Debug, Clone)]
pub struct JsonText<T>(
    /// 解码后的 JSON 值。
    pub T,
);

impl<T: Serialize> Serialize for JsonText<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let text = serde_json::to_string(&self.0).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&text)
    }
}

impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for JsonText<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let result = match value {
            Value::String(text) => serde_json::from_str(&text),
            other => serde_json::from_value(other),
        };
        result.map(Self).map_err(serde::de::Error::custom)
    }
}

// 部分旧版响应使用空字符串表示没有规则或扩展数据。
fn optional_json_text<'de, D, T>(deserializer: D) -> Result<Option<JsonText<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let value = Value::deserialize(deserializer)?;
    if value.is_null() || value.as_str().is_some_and(|text| text.trim().is_empty()) {
        return Ok(None);
    }
    JsonText::deserialize(value)
        .map(Some)
        .map_err(serde::de::Error::custom)
}

/// 接口允许业务方自定义键和值的扩展配置。
pub type ExtensionInfo = BTreeMap<String, Value>;

/// 仅携带公共参数。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyRequest {}

/// 查询列表。mcube 2.57.0 起支持分页。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListRequest {
    /// 选填。页码，从 1 开始。与 pageSize 同时填写时启用分页。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_num: Option<u64>,
    /// 选填。每页数量。只填写其中一项时服务端返回全量。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u64>,
}

/// 指定应用实例。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct H5IdRequest {
    /// 必填。实例 ID。小程序为 16 位数字，离线包为 8 位，自定义资源包为 6 位。保留前导零。
    pub h5_id: String,
}

/// 创建应用实例或修改小程序名称。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAppRequest {
    /// 必填。实例 ID。小程序为 16 位数字，离线包为 8 位，自定义资源包为 6 位。
    pub h5_id: String,
    /// 必填。实例名称。修改名称时填写新值。
    pub h5_name: String,
}

/// 按资源、配置或白名单主键操作。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceIdRequest {
    /// 必填。对应记录的主键。任务列表接口使用资源包主键。
    pub id: Integer,
}

/// 查询发布任务详情。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskIdRequest {
    /// 必填。发布任务主键。
    pub task_id: Integer,
}

/// 按升级包资源主键查询。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageIdRequest {
    /// 必填。发布任务关联的升级包资源主键。
    pub package_id: Integer,
}

/// 查询指定资源的发布任务。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTaskListRequest {
    /// 必填。资源包主键。
    pub id: Integer,
    /// 可选分页字段。
    #[serde(flatten)]
    pub pagination: ListRequest,
}

/// 查询升级包发布任务。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeTaskListRequest {
    /// 必填。升级包资源主键。
    pub package_id: Integer,
    /// 可选分页字段。
    #[serde(flatten)]
    pub pagination: ListRequest,
}

/// 查询离线包或自定义资源包的资源。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct H5PackageListRequest {
    /// 必填。离线包或自定义资源包实例 ID。
    pub h5_id: String,
    /// 可选分页字段。
    #[serde(flatten)]
    pub pagination: ListRequest,
}

/// 查询小程序资源包列表。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniProgramPackageListRequest {
    /// 必填。16 位小程序 ID。
    pub h5_id: String,
    /// 必填。资源包类型，多个值用半角逗号分隔。1 功能包、2 插件包、3 真机测试、4 真机预览。
    pub package_types: String,
    /// 选填。资源子类型过滤值。旧版接口可省略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_type: Option<String>,
    /// 可选分页字段。
    #[serde(flatten)]
    pub pagination: ListRequest,
}

/// 修改小程序资源包类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMiniPackageTypeRequest {
    /// 必填。16 位小程序 ID。
    pub h5_id: String,
    /// 必填。资源包主键。
    pub package_id: Integer,
    /// 必填。此接口只支持修改为 1（功能包）。
    pub package_type: u8,
}

/// 上传小程序资源。SDK 自动上传流程的版本进位规则由独立的 upload action 处理。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniProgramUploadRequest {
    /// 必填。16 位小程序 ID。
    pub h5_id: String,
    /// 必填。小程序名称。
    pub h5_name: String,
    /// 必填。资源版本，同一小程序下须唯一。
    pub h5_version: String,
    /// 选填。小程序主入口。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_url: Option<String>,
    /// 选填。小程序次入口。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suburl: Option<String>,
    /// 必填。小程序虚拟域名，格式为小程序 ID 加点号再加基础域名。
    /// 基础域名从离线包虚拟域名查询接口取得。例如基础域名为 h5app.com 时，使用 1234567812345678.h5app.com。
    pub vhost: String,
    /// 选填。扩展字段，在线路上是 JSON 字符串。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extend_info: Option<JsonText<ExtensionInfo>>,
    /// 必填。下载时机。0 仅 Wi-Fi 自动下载，其他网络下用户使用应用时才下载。
    /// 1 在所有网络下自动下载，会消耗用户的移动数据，仅用于确有需要的场景。
    pub auto_install: u8,
    /// 必填。资源类型，固定为 4。
    pub resource_type: u8,
    /// 必填。安装时机。0 进入页面时安装，1 下载完成后预加载。
    pub install_type: u8,
    /// 必填。平台取值为 android、iOS、all。mcube 2.59.0 起支持 Harmony。
    /// 选择全部平台传 all，选择两个平台时用半角逗号连接。
    pub platform: String,
    /// 必填。每个选中平台都要填写最低版本。旧版顺序为 iOS;Android，mcube 2.59.0 起为 iOS;Android;Harmony。
    /// 未选择的平台留空，保留全部分号。例如旧版仅选 Android 时传 ;1.0.0。
    pub client_version_min: String,
    /// 选填。各平台最高版本，段顺序和分号规则与 clientVersionMin 一致。
    /// 旧版 all 模式下 iOS 与 Android 的最高版本须同时填写或同时留空。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_version_max: Option<String>,
    /// 必填。ZIP 文件。小程序 ZIP 的文件名和解压后的顶层目录均以小程序 ID 命名。
    /// mcube 2.57.1 起插件可上传仅含插件资源的旧格式，或上传 `<插件ID>.zip`。
    /// 新格式包含 `<插件ID>-miniProgram.zip`（壳小程序文件）和 `<插件ID>-plugin.zip`（插件资源）。
    pub resource_file: PathBuf,
    /// 必填。小程序图标文件，格式为 JPG 或 PNG，通过 iconFile 附件上传。
    pub icon_file: PathBuf,
    /// 必填。底部导航栏开关，0 不显示，1 显示。
    pub enable_tab_bar: u8,
    /// 必填。右上角功能菜单开关，0 不显示，1 显示。
    pub enable_option_menu: u8,
    /// 必填。后台保活开关，0 关闭，1 开启。
    pub enable_keep_alive: u8,
    /// 选填。1 功能包、2 插件包、3 真机测试、4 真机预览。默认 1。1 和 2 属于正式包，3 和 4 属于测试包。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_type: Option<u8>,
    /// 选填。ZIP 是否支持分包，mcube 2.51.1 起提供。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subpackage_flag: Option<bool>,
    /// 选填。额外的 Cube 小程序资源文件，mcube 2.51.4 起提供。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cube_resource_file: Option<PathBuf>,
    /// 选填。预览或真机调试的用户 ID，mcube 2.49.0 起与 uuid 一起用于自动发布和返回扫码链接。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// 选填。预览或真机调试标识，mcube 2.49.0 起提供，与 userId 配合使用。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// 选填。绑定插件版本的映射，键为插件 ID，值为该插件的版本号。以 JSON 字符串传输，mcube 2.57.1 起提供。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mini_plugin_id_mappings: Option<JsonText<BTreeMap<String, String>>>,
}

/// 上传单个离线包资源。上传后等资源详情中的 fallbackBaseUrl 与 memo 均有值，再创建发布任务。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NebulaResourceRequest {
    /// 必填。8 位离线包实例 ID。
    pub h5_id: String,
    /// 必填。离线包实例名称。
    pub h5_name: String,
    /// 必填。离线包版本，同一实例下须唯一。
    pub h5_version: String,
    /// 选填。主入口，文档要求匹配 `^/[\w|/]+\.html$`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_url: Option<String>,
    /// 必填。H5App 的虚拟域名。
    pub vhost: String,
    /// 选填。扩展信息，在线路上是 JSON 字符串。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extend_info: Option<JsonText<ExtensionInfo>>,
    /// 必填。下载时机。0 仅 Wi-Fi 自动下载，其他网络下用户使用应用时才下载。
    /// 1 在所有网络下自动下载，会消耗用户的移动数据，仅用于确有需要的场景。
    pub auto_install: u8,
    /// 必填。0 全局资源包，1 普通资源包。同一个 H5App 只能存在一种资源类型。
    pub resource_type: u8,
    /// 必填。0 进入页面时安装，1 下载完成后预加载。
    pub install_type: u8,
    /// 必填。平台为 android、iOS 或 all。mcube 2.59.0 起支持 Harmony，选两个平台用半角逗号连接。
    pub platform: String,
    /// 必填。为每个选中的平台填写最低版本。旧版为 iOS;Android，mcube 2.59.0 起为 iOS;Android;Harmony。
    /// 未选择的平台留空，保留全部分号。例如旧版仅选 Android 时传 ;1.0.0。
    pub client_version_min: String,
    /// 选填。各平台最高版本，段顺序和分号规则与 clientVersionMin 一致。
    /// 旧版 all 模式下 iOS 与 Android 的最高版本须同时填写或同时留空。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_version_max: Option<String>,
    /// 条件必填。resourceType 为 0（全局资源包）时填写，0 不复用，1 复用。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_nebula: Option<u8>,
    /// 必填。离线包 ZIP，文件名与解压后的顶层目录名均为 H5App ID。
    pub file: PathBuf,
    /// 选填。是否生成差分包。mcube 2.49.0 起支持，默认 false。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub need_diff_flag: Option<bool>,
}

/// 上传自定义资源包。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomResourceUploadRequest {
    /// 必填。6 位自定义资源包实例 ID。
    pub h5_id: String,
    /// 必填。自定义资源包实例名称。
    pub h5_name: String,
    /// 必填。资源版本，同一实例内唯一。
    pub h5_version: String,
    /// 选填。扩展字段，在线路上是 JSON 字符串。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extend_info: Option<JsonText<ExtensionInfo>>,
    /// 必填。平台为 android、iOS 或 all。mcube 2.59.0 起支持 Harmony，选两个平台用半角逗号连接。
    pub platform: String,
    /// 必填。为每个选中的平台填写最低版本。旧版为 iOS;Android，mcube 2.59.0 起为 iOS;Android;Harmony。
    /// 未选择的平台留空，保留全部分号。例如旧版仅选 Android 时传 ;1.0.0。
    pub client_version_min: String,
    /// 选填。各平台最高版本，段顺序和分号规则与 clientVersionMin 一致。
    /// 旧版 all 模式下 iOS 与 Android 的最高版本须同时填写或同时留空。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_version_max: Option<String>,
    /// 必填。ZIP 格式的自定义资源文件。
    pub file: PathBuf,
}

/// 创建小程序、离线包或自定义资源包任务的公共字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishTaskRequest {
    /// 必填。2 灰度发布，3 正式发布。
    pub publish_type: u8,
    /// 条件必填。publishType 为 2 时填写。1 白名单、2 时间窗，小程序另支持 5 第三方灰度。正式发布时省略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_mode: Option<u8>,
    /// 选填。发布描述。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    /// 必填。创建任务时固定为 0。
    pub id: u64,
    /// 必填。发布资源包的主键。
    pub package_id: Integer,
    /// 条件必填。publishMode 为 2 时填写。格式为 yyyy-MM-dd HH:mm:ss，晚于服务端当前时间且相隔小于 7 天。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_endtime_data: Option<String>,
    /// 条件必填。publishMode 为 2 时填写时间窗灰度人数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_num: Option<u64>,
    /// 条件必填。publishMode 为 1 时填写白名单主键，多个用半角逗号分隔。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitelist_ids: Option<String>,
    /// 选填。高级灰度规则数组，在线路上是 JSON 字符串。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_config_info: Option<JsonText<Vec<RuleJson>>>,
}

/// 创建小程序或小程序插件发布任务。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniProgramPublishTaskRequest {
    /// 发布任务的类型、资源主键与灰度参数，序列化到请求顶层。
    #[serde(flatten)]
    pub settings: PublishTaskRequest,
    /// 选填。0 普通版本，1 体验版本。小程序从 mcube 2.51.5 起提供，2.52.0、2.54.0 和 2.54.1 不包含此功能。插件从 2.57.1 起提供。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experience_flag: Option<u8>,
}

/// 创建离线包发布任务。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NebulaPublishTaskRequest {
    /// 发布任务的类型、资源主键与灰度参数，序列化到请求顶层。
    #[serde(flatten)]
    pub settings: PublishTaskRequest,
    /// 选填。时间窗结束时间，Date 字段，值与 greyEndtimeData 一致。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_end_time: Option<String>,
    /// 选填。mcube 2.58.10 起支持预计生效时间。格式为 yyyy-MM-dd HH:mm:ss，采用服务端时区。
    /// 设置后，客户端在到达该时间后才能收到此版本的更新信息。留空表示未设置。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_effective_time: Option<String>,
}

/// 创建自定义资源包发布任务。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomPublishTaskRequest {
    /// 发布任务的类型、资源主键与灰度参数，序列化到请求顶层。
    #[serde(flatten)]
    pub settings: PublishTaskRequest,
    /// 选填。时间窗结束时间，Date 字段，值与 greyEndtimeData 一致。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_end_time: Option<String>,
}

/// 修改小程序、离线包、自定义资源包或热修复的任务状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusRequest {
    /// 必填。小程序、离线包和自定义资源包传 nebula，热修复传 hotpatch。
    pub biz_type: String,
    /// 必填。关联的资源包主键。
    pub package_id: Integer,
    /// 必填。发布任务主键。
    pub task_id: Integer,
    /// 必填。目标状态。0 待发布、1 发布中、2 已结束、3 暂停。
    pub task_status: u8,
}

/// 修改升级包的发布任务状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeTaskStatusRequest {
    /// 必填。升级包发布任务主键。接口将此字段作为字符串。
    pub task_id: String,
    /// 必填。0 待发布、1 发布中、2 已结束、3 暂停。表单中按文本发送。
    pub task_status: u8,
}

/// 请求中的高级灰度规则。操作范围受规则元素限制。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleJson {
    /// 必填。小程序、离线包、自定义资源包和热修复支持 city（城市）、mobileModel（设备型号）、netType（网络）、osVersion（系统版本）。
    /// 升级包另支持 productVersion（产品版本）。开关支持 productVersion、mobileModel、osVersion。
    pub rule_element: String,
    /// 必填。1 包含、2 不包含、3 范围内、4 范围外。城市、设备型号和网络仅支持 1、2，系统版本支持全部四种操作。
    pub operation: u8,
    /// 必填。多个值用半角逗号分隔。范围操作使用 aa-bb，aa 为下限，bb 为上限。
    pub value: String,
    /// 选填。旧版规则类型，Java 示例传 0。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_type: Option<String>,
    /// 选填。范围下限，用于 operation 为 3 或 4 的规则。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value1: Option<String>,
    /// 选填。范围上限，用于 operation 为 3 或 4 的规则。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value2: Option<String>,
}

/// 批量创建离线包实例。SDK 将认证配置中的 appId 与 workspaceId 写入 JSON 顶层。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NebulaAppBatchRequest {
    /// 必填。待创建的实例列表。每项需要 8 位 h5Id 与 h5Name。
    pub nebula_app_request_list: Vec<CreateAppRequest>,
    /// 选填。主应用 ID。SDK 使用鉴权配置中的值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    /// 选填。工作空间 ID。SDK 使用鉴权配置中的值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

/// 批量资源的覆盖值。未填写的项目字段依次继承 sharedConfig 与服务端默认值。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NebulaBatchResourceFields {
    /// 选填。主应用 ID。省略时由服务端根据请求上下文处理。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    /// 选填。工作空间 ID。省略时由服务端根据请求上下文处理。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// 选填。离线包实例名称。未填时从共享配置继承。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h5_name: Option<String>,
    /// 选填。离线包主入口，规则同单包上传。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_url: Option<String>,
    /// 选填。离线包虚拟域名。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vhost: Option<String>,
    /// 选填。扩展信息，在线路上是 JSON 字符串。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extend_info: Option<JsonText<ExtensionInfo>>,
    /// 选填。批量接口用字符串表示下载时机，0 仅 Wi-Fi，1 所有网络。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_install: Option<String>,
    /// 选填。离线包资源类型，同单包上传。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<u8>,
    /// 选填。0 不预加载，1 预加载。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_type: Option<u8>,
    /// 选填。目标平台，含义同单包上传。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// 选填。最低客户端版本，版本段规则同单包上传。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_version_min: Option<String>,
    /// 选填。最高客户端版本，版本段规则同单包上传。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_version_max: Option<String>,
    /// 选填。0 不复用全局资源包，1 复用。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_nebula: Option<u8>,
    /// 选填。是否生成差分包，默认 false。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub need_diff_flag: Option<bool>,
    /// 选填。离线包次入口。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_url: Option<String>,
    /// 选填。用于离线包下载的自定义域名。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_domain_name: Option<String>,
}

/// 批量上传中的一个离线包。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NebulaBatchResourceItem {
    /// 必填。8 位离线包实例 ID。对应 files 中以该 ID 命名的 ZIP。
    pub h5_id: String,
    /// 必填。离线包版本，在对应实例下唯一。
    pub h5_version: String,
    /// 选填。当前资源的配置，序列化到该资源项的同一层。
    #[serde(flatten)]
    pub settings: NebulaBatchResourceFields,
}

/// 批量资源配置。每项的其他字段依次取单项配置、共享配置及系统默认值。
/// 合并后仍缺少必填字段时，服务端返回该项上传失败。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NebulaResourceBatchConfig {
    /// 选填。可被单项配置覆盖的批量默认值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_config: Option<NebulaBatchResourceFields>,
    /// 必填。每个 ZIP 对应一个配置项。
    pub nebula_resource_list: Vec<NebulaBatchResourceItem>,
}

/// 批量上传离线包资源。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NebulaResourceBatchRequest {
    /// 必填。批量资源配置 JSON 字符串。
    pub config_string: JsonText<NebulaResourceBatchConfig>,
    /// 必填。为每个配置项上传一个以其实例 ID 命名的 ZIP，重复发送 files 表单字段。
    pub files: Vec<PathBuf>,
}

/// 批量创建任务的共享字段或单项覆盖字段。未填项从 sharedConfig 继承。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NebulaBatchTaskFields {
    /// 选填。appId 与 workspaceId 组成的应用编码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_code: Option<String>,
    /// 选填。主应用 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    /// 选填。工作空间 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// 选填。业务类型，填写 nebula。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub biz_type: Option<String>,
    /// 选填。创建时传 0。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    /// 条件必填。发布资源包主键，由单项配置或共享配置提供。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_id: Option<u64>,
    /// 条件必填。2 灰度发布、3 正式发布，由单项配置或共享配置提供。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_type: Option<u8>,
    /// 条件必填。灰度发布时由单项配置或共享配置提供，1 白名单、2 时间窗。批量正式发布可填 3。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_mode: Option<u8>,
    /// 选填。发布描述。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    /// 选填。Date 格式截止时间，值与 greyEndtimeData 一致。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_end_time: Option<String>,
    /// 条件必填。时间窗灰度时由单项配置或共享配置提供，格式 yyyy-MM-dd HH:mm:ss。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_endtime_data: Option<String>,
    /// 条件必填。时间窗灰度时由单项配置或共享配置提供灰度人数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_num: Option<u64>,
    /// 条件必填。白名单灰度时由单项配置或共享配置提供，多个主键用半角逗号分隔。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitelist_ids: Option<String>,
    /// 选填。高级灰度规则 JSON 字符串。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_config_info: Option<JsonText<Vec<RuleJson>>>,
    /// 选填。预计生效时间，格式 yyyy-MM-dd HH:mm:ss。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_effective_time: Option<String>,
    /// 选填。关联资源主键列表，可填写对应 packageId 的字符串。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub res_ids: Option<String>,
    /// 选填。发布平台。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// 选填。发布百分比，PDF 示例传 0。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<u64>,
    /// 选填。任务记录状态，PDF 示例传 0。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u8>,
    /// 选填。任务发布状态，PDF 示例传 0。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_status: Option<u8>,
    /// 选填。任务类型，PDF 示例传 0。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_type: Option<u8>,
    /// 选填。任务版本，PDF 示例传 0。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_version: Option<u64>,
    /// 选填。升级提醒次数，PDF 示例传 0。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade_notice_num: Option<u64>,
    /// 选填。Java 模型中的创建时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_create: Option<String>,
    /// 选填。Java 模型中的更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<String>,
    /// 选填。格式化的更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified_str: Option<String>,
    /// 选填。任务对应的产品编码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_id: Option<String>,
    /// 选填。资源版本。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_version: Option<String>,
    /// 选填。发布任务名称。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_name: Option<String>,
    /// 选填。Java Date 字段表示的灰度结束时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_endtime: Option<String>,
    /// 选填。格式化的灰度结束时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_endtime_str: Option<String>,
    /// 选填。升级进度。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade_progress: Option<String>,
    /// 选填。创建者。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    /// 选填。更新者。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifier: Option<String>,
    /// 选填。Java 模型中的资源列表。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundles: Option<Vec<BTreeMap<String, String>>>,
    /// 选填。发布版本号。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_version: Option<String>,
    /// 选填。同步结果。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_result: Option<String>,
    /// 选填。同步模式。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_mode: Option<String>,
    /// 选填。灰度检查地址。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_url: Option<String>,
}

/// 批量创建离线包发布任务。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicTaskBatchRequest {
    /// 选填。共享发布配置。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_config: Option<NebulaBatchTaskFields>,
    /// 必填。发布任务列表。每项与共享配置合并后需具备单个任务所需字段。
    pub task_list: Vec<NebulaBatchTaskFields>,
}

/// 批量状态更新的共享字段或单项配置。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NebulaBatchStatusFields {
    /// 选填。主应用 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    /// 选填。工作空间 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// 选填。业务类型，传 nebula。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub biz_type: Option<String>,
    /// 条件必填。由单项配置或共享配置提供，0 待发布、1 发布中、2 已结束、3 暂停。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_status: Option<u8>,
}

/// 批量状态更新中的一个任务。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NebulaBatchStatusItem {
    /// 必填。关联资源包主键，以字符串传输。
    pub package_id: String,
    /// 必填。发布任务主键，以字符串传输。
    pub task_id: String,
    /// 选填。当前任务的配置，序列化到该任务项的同一层。
    #[serde(flatten)]
    pub settings: NebulaBatchStatusFields,
}

/// 批量修改离线包发布任务状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusBatchRequest {
    /// 选填。共享配置。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_config: Option<NebulaBatchStatusFields>,
    /// 必填。任务列表。每项必须填写 packageId 与 taskId，taskStatus 可继承共享配置。
    pub task_change_list: Vec<NebulaBatchStatusItem>,
}

/// 配置离线包基础域名。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VhostRequest {
    /// 必填。使用两级或三级域名，例如 h5app.com，域名中的字母须为小写，省略 HTTP 或 HTTPS 前缀。
    /// 同一 appId 和 workspaceId 下只可配置一次，配置后不可修改。小程序与离线包共享。
    pub vhost: String,
}

/// 为当前应用与工作空间配置离线包签名私钥。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NebulaRsaKeyRequest {
    /// 必填。用于离线包签名的 RSA 私钥，上传 PEM 文件。客户端用对应的公钥验签。
    /// 小程序与离线包共享配置。同一 appId 和 workspaceId 下只可上传一次，上传后不可修改。
    pub file: PathBuf,
}

/// 新增小程序权限配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniProgramConfigRequest {
    /// 必填。应用编码，格式 appId-workspaceId。
    pub app_code: String,
    /// 必填。16 位小程序 ID。
    pub h5_id: String,
    /// 必填。小程序名称。
    pub h5_name: String,
    /// 必填。API 为 API 调用，SERVER_DOMAIN 为服务器域名，WEB_VIEW_DOMAIN 为 WebView 域名。
    /// 更新已有权限开关时使用 PRIVILEGE_SWITCH。
    pub config_type: String,
    /// 必填。对应配置类型的值。每个小程序的 PRIVILEGE_SWITCH 默认是 OFF。
    /// 设为 ON 后新上传版本会包含权限文件，设为 OFF 后新上传版本省略权限文件。
    pub config_value: String,
}

/// 更新小程序权限配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMiniProgramConfigRequest {
    /// 此操作复用的请求字段。
    #[serde(flatten)]
    pub settings: MiniProgramConfigRequest,
    /// 必填。待更新配置的主键。
    pub id: Integer,
}

/// 删除小程序权限配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteMiniProgramConfigRequest {
    /// 必填。16 位小程序 ID。
    pub h5_id: String,
    /// 必填。待删除配置的主键。
    pub id: Integer,
}

/// 创建或更新第三方灰度地址。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThirdGreyUrlRequest {
    /// 必填。供 mPaaS 检查第三方灰度的 URL。选择第三方灰度模式（publishMode 为 5）前需完成配置。
    pub grey_url: String,
}

/// 保存开放平台接入信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenPlatformRequest {
    /// 必填。需要调用的开放平台地址。
    pub app_url: String,
    /// 必填。开放平台公钥。
    pub pubkey: String,
}

/// 分配或移除账号和资源的关系。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivilegeRequest {
    /// 必填。用户账号，多个账号用半角逗号分隔。
    pub user_accounts: String,
    /// 必填。资源编码。
    pub resource_code: String,
}

/// 添加发布规则资源。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceConfigRequest {
    /// 必填。资源类型。city 城市、mobileModel 设备型号、netType 网络、osVersion 系统版本。
    pub resource_type: String,
    /// 必填。platform 平台无关，android 或 ios 表示对应平台。
    pub platform: String,
    /// 必填。资源显示名称。
    pub resource_name: String,
    /// 必填。规则使用的资源值。
    pub resource_value: String,
}

/// 更新发布规则资源。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResourceConfigRequest {
    /// 此操作复用的请求字段。
    #[serde(flatten)]
    pub settings: ResourceConfigRequest,
    /// 必填。待更新的规则资源主键。
    pub id: Integer,
}

/// 批量删除发布规则资源。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResourceBatchRequest {
    /// 必填。资源主键列表，以半角逗号分隔。
    pub resource_ids: String,
}

/// 创建热修复资源。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotpatchResourceRequest {
    /// 必填。产品版本，格式 x.x.x.x。
    pub product_version: String,
    /// 必填。android 或 ios。
    pub platform: String,
    /// 必填。修复说明。
    pub fix_desc: String,
    /// 必填。热修复资源。Android 为 JAR，iOS 为 JS。
    pub resource_file: PathBuf,
}

/// 删除热修复资源。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteHotpatchResourceRequest {
    /// 必填。热修复资源主键。
    pub id: Integer,
    /// 必填。应用编码。PDF 写为 appId_workspaceId，部分 Demo 由 AppCodeUtil 生成。
    pub app_code: String,
}

/// 创建热修复发布任务。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotpatchTaskRequest {
    /// 必填。2 灰度发布，3 正式发布。
    pub publish_type: u8,
    /// 条件必填。灰度发布时填写，1 白名单、2 时间窗。正式发布时省略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_mode: Option<u8>,
    /// 选填。发布描述。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    /// 选填。部分旧版接口使用的修复说明字段。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_desc: Option<String>,
    /// 选填。时间窗灰度截止时间，Date 字段，与 greyEndtimeData 表示同一时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_endtime: Option<String>,
    /// 条件必填。publishMode 为 2 时填写时间窗截止时间，格式 yyyy-MM-dd HH:mm:ss。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_endtime_data: Option<String>,
    /// 条件必填。publishMode 为 2 时填写时间窗灰度人数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_num: Option<u64>,
    /// 必填。热修复资源主键。
    pub package_id: Integer,
    /// 条件必填。publishMode 为 1 时填写白名单主键，多个用半角逗号分隔。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitelist_ids: Option<String>,
    /// 选填。高级灰度规则 JSON 字符串。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_config_info: Option<JsonText<Vec<RuleJson>>>,
}

/// 创建热修复回滚发布任务。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotpatchRollbackRequest {
    /// 必填。热修复包资源主键。
    pub id: Integer,
    /// 必填。待回滚热修复资源的产品版本。
    pub product_version: String,
    /// 必填。发布平台，android 或 ios。
    pub platform: String,
    /// 必填。产品编码，形如 appId_ANDROID-workspaceId，平台部分大写。
    pub product_id: String,
}

/// 创建升级资源。文件和商店地址的必填条件由平台和 iOS 版本类型决定。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradePackageRequest {
    /// 必填。Android 或 iOS。
    pub platform: String,
    /// 必填。版本号，格式 x.x.x.x。
    pub app_version: String,
    /// 必填。版本描述。
    pub desc: String,
    /// 条件必填。上传 Android 或 iOS 企业版时填写，分别上传 APK 和 IPA。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// 选填。64 位 Android 安装包，mcube 2.50.1 起支持。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub android64_file: Option<PathBuf>,
    /// 条件必填。上传 iOS 企业版时提供包图标。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_file: Option<PathBuf>,
    /// 条件必填。平台为 iOS 时填写，0 正式版、1 企业版、2 TestFlight。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_enterprise: Option<u8>,
    /// 条件必填。iOS 正式版或 TestFlight 的 App Store 下载地址。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appstore_url: Option<String>,
    /// 条件必填。上传 TestFlight 包时填写允许下载的人数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_amount: Option<u64>,
    /// 条件必填。上传 TestFlight 包时填写有效天数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_days: Option<u64>,
    /// 条件必填。上传 Android 或 iOS 企业版时填写，0 扫码下载免验证码、1 校验验证码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub need_check: Option<u8>,
    /// 条件必填。上传 iOS 企业版时填写包的 Bundle ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    /// 选填。自定义下载域名，可以为空。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_domain_name: Option<String>,
}

/// 上传 iOS 符号表。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeSymbolRequest {
    /// 必填。平台填写 iOS。
    pub platform: String,
    /// 必填。版本号，格式 x.x.x.x。
    pub app_version: String,
    /// 必填。符号表文件。接口字段的小写 f 为原始拼写。
    pub ios_symbolfile: PathBuf,
}

/// 删除升级资源或查询扫码验证码。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradePlatformResourceRequest {
    /// 必填。升级资源主键。
    pub id: Integer,
    /// 必填。Android 或 iOS。
    pub platform: String,
}

/// 创建升级包发布任务。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradePublishTaskRequest {
    /// 条件必填。publishMode 为 2 时填写时间窗截止时间，格式 yyyy-MM-dd HH:mm:ss。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_endtime_data: Option<String>,
    /// 条件必填。publishMode 为 2 时填写时间窗灰度人数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_num: Option<u64>,
    /// 选填。是否检测历史版本的强制升级，1 是、0 否。
    /// 开启后，客户端版本与最新版之间存在强制升级版本时，客户端收到强制升级模式。当前任务结束后停止检测。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_force: Option<u8>,
    /// 选填。高级灰度规则 JSON 字符串。除通用规则外，可使用 productVersion。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_config_info: Option<JsonText<Vec<RuleJson>>>,
    /// 选填。发布描述。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    /// 必填。2 灰度发布，3 正式发布。
    pub publish_type: u8,
    /// 条件必填。灰度发布时填写，1 白名单、2 时间窗。正式发布时省略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_mode: Option<u8>,
    /// 必填。1 单次提醒、2 多次提醒、3 强制升级。
    pub upgrade_type: u8,
    /// 条件必填。publishMode 为 1 时填写白名单主键，多个用半角逗号分隔。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitelist_ids: Option<String>,
    /// 必填。升级包资源主键。
    pub package_info_id: Integer,
    /// 选填。展示给客户端用户的升级提示。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade_content: Option<String>,
    /// 选填。命中升级的时间段，mcube 2.58.5 起支持。在线路上是 JSON 字符串。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reminder_period: Option<JsonText<Vec<ReminderPeriod>>>,
}

/// 升级提醒时间段。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderPeriod {
    /// 必填。1 每天、2 每周、3 每月。
    pub reminder_type: u8,
    /// 必填。每天用 *。每周按周一至周日使用 1 到 7，每月使用 1 到 31。多个值按自然顺序用逗号连接，* 表示全选。
    pub reminder_index: String,
    /// 必填。时间段开始时间，格式 HH:mm:ss。
    pub reminder_start_time: String,
    /// 必填。时间段结束时间，格式 HH:mm:ss。
    pub reminder_end_time: String,
}

/// 创建发布白名单。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewWhitelistRequest {
    /// 必填。白名单名称，长度为 2 到 10 个字符。
    pub white_list_name: String,
    /// 必填。业务分类。hotpatch 热修复、upgrade 升级包、nebula 离线包。
    pub business: String,
    /// 必填。标识类型。文档当前仅支持 userId。
    pub id_type: String,
    /// 选填。normal 普通名单、regular 正则名单。支持白名单类型的环境按需填写，旧版可省略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitelist_type: Option<String>,
}

/// 添加白名单内容。keyIds 与 keyIdsFile 必须二选一。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifyWhitelistRequest {
    /// 必填。白名单主键。
    pub id: Integer,
    /// 条件必填。未上传 keyIdsFile 时填写，多个用户 ID 用半角逗号分隔。正则名单仅使用此字段。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_ids: Option<String>,
    /// 条件必填。普通名单未填写 keyIds 时上传文件，文件小于 500 MB，每行一个用户 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_ids_file: Option<PathBuf>,
}

/// 分页查询白名单详情。需要 mcube 高于 2.44.0 且 whitelist_storage_type 为 bigdata。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhitelistDetailRequest {
    /// 必填。白名单主键。
    pub whitelist_id: Integer,
    /// 选填。查询的完整关键字，服务端采用精确匹配。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_word: Option<String>,
    /// 选填。上一页末尾的白名单内容。首次查询可省略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_id_value: Option<String>,
}

/// 删除指定白名单内容。需要 mcube 高于 2.44.0 且 whitelist_storage_type 为 bigdata。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteWhitelistContentRequest {
    /// 必填。白名单主键。
    pub whitelist_id: Integer,
    /// 必填。待删除的记录，多个值使用换行符分隔。
    pub whitelist_value: String,
}

/// 修改白名单名称。需要 mcube 高于 2.44.0 且 whitelist_storage_type 为 bigdata。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWhitelistRequest {
    /// 必填。白名单主键。
    pub whitelist_id: Integer,
    /// 必填。新的白名单名称。
    pub whitelist_name: String,
}

/// 客户端开关列表过滤条件。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchListRequest {
    /// 选填。开关键的模糊搜索关键词。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyword: Option<String>,
    /// 可选分页字段。
    #[serde(flatten)]
    pub pagination: ListRequest,
}

/// 设置客户端开关状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchStatusRequest {
    /// 必填。待修改的开关主键。
    pub switch_id: Integer,
    /// 必填。0 失效，1 激活。
    pub status: u8,
}

/// 导入客户端开关。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchImportRequest {
    /// 必填。待导入配置，文件后缀必须为 .config。
    pub file: PathBuf,
}

/// 导出客户端开关。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchExportRequest {
    /// 必填。配置主键列表，以半角逗号分隔。
    pub conf_key_ids: String,
}

/// 新增或更新客户端开关。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchInfoForm {
    /// 必填。开关配置，在线路上是 JSON 字符串。
    pub switch_info_string: JsonText<SwitchInfo>,
}

/// 客户端开关配置正文。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchInfo {
    /// 必填。新增填 0，更新填写已有开关主键。
    pub id: u64,
    /// 必填。配置键，最多 50 个字符，支持字母、数字和下划线。
    pub conf_key: String,
    /// 必填。业务类型，填写 default。
    pub business: String,
    /// 选填。开关备注，最多 200 个字符。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    /// 必填。0 失效、1 激活。
    pub status: u8,
    /// 必填。配置值列表，排序越靠前优先级越高。
    pub conf_value_infos: Vec<SwitchValueInfo>,
}

/// 客户端开关的单项配置值。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchValueInfo {
    /// 必填。配置值主键，新增填 0。
    pub id: u64,
    /// 必填。all 全平台、ios 或 android。
    pub platform: String,
    /// 必填。文档要求填 0。
    pub pre: u8,
    /// 必填。0 待发布、1 发布中、2 已结束、3 暂停。实际使用时填 1。
    pub task_status: u8,
    /// 必填。1 白名单、3 百分比、4 全量。
    pub publish_mode: u8,
    /// 必填。百分比发布时取 1 到 99，其他模式填 100。
    pub percent: u8,
    /// 必填。配置值，最多 4000 字符。
    pub conf_value: String,
    /// 条件必填。publishMode 为 1 时指定白名单。支持逗号分隔的主键或白名单分组的 JSON 字符串。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitelist_ids: Option<SwitchWhitelistSelection>,
    /// 选填。高级灰度规则列表，支持 productVersion、mobileModel 和 osVersion。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_json_list: Option<Vec<RuleJson>>,
}

/// 开关灰度使用的白名单选择。两种形式均以字符串传输。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SwitchWhitelistSelection {
    /// 白名单分组，序列化成 JSON 字符串。配置文件也可写成数组。
    Groups(JsonText<Vec<SwitchWhitelistGroup>>),
    /// 半角逗号分隔的白名单主键。
    Ids(String),
}

/// 开关灰度使用的一组白名单记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchWhitelistGroup {
    /// 必填。白名单来源，MDS 白名单填 mds。
    #[serde(rename = "type")]
    pub kind: String,
    /// 必填。白名单组名称。
    pub group_name: String,
    /// 必填。白名单主键。
    pub id: Integer,
}

/// 成功响应。业务数据在 data 中，分页元数据来自响应外层。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    /// 接口返回的业务数据。
    pub data: T,
    /// 服务端生成的请求跟踪 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// 列表接口的分页信息。
    #[serde(flatten)]
    pub pagination: Pagination,
}

/// 接口可能返回旧版成功提示或新版资源 ID。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MutationResult {
    /// 响应文本，可能是成功提示或数字字符串 ID。
    Text(String),
    /// 整数主键或服务端结果码。
    Number(i64),
    /// 服务端业务结果。
    Boolean(bool),
    /// 服务端声明成功但响应没有业务正文。
    Empty,
}

/// 小程序上传的结果。常规上传返回资源 ID，预览或调试可返回二维码相关数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MiniProgramUploadResult {
    /// 资源主键，旧版也可能返回 success。
    Resource(MutationResult),
    /// 版本相关的预览或调试结果。文档变更记录未给出该对象的固定字段。
    Preview(BTreeMap<String, Value>),
}

/// 开关列表。旧路径使用内层分页对象，部分版本返回列表。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SwitchListResult {
    /// 内层分页对象。
    Page(SwitchPage),
    /// 开关列表。
    Rows(Vec<SwitchRecord>),
}

/// 服务端返回的高级规则内容。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GreyRules {
    /// 编译后的规则树。
    Tree(GreyRuleTree),
    /// 详情中返回的原始规则。
    Rules(Vec<RuleResult>),
}

/// 高级规则的比较参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuleOperand {
    /// 用于包含或排除判断的值。
    Values(Vec<String>),
    /// 用于版本范围判断的上下限。
    Range(RuleRange),
}
/// mcube 2.57.0 起返回的外层分页信息。全量查询时分页字段为空。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    /// 当前页码，从 1 开始。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_page: Option<u64>,
    /// 符合条件的记录总数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_count: Option<u64>,
    /// 每页记录数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u64>,
    /// 是否还有下一页。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_more: Option<bool>,
}

/// 小程序、离线包或自定义资源包实例详情。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    /// 实例记录主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Integer>,
    /// 应用编码，通常为 appId-workspaceId。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_code: Option<String>,
    /// 实例 ID。小程序 16 位，离线包 8 位，自定义资源包 6 位。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h5_id: Option<String>,
    /// 实例名称。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h5_name: Option<String>,
    /// 0 仅 Wi-Fi 自动下载，1 所有网络下载。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_install: Option<u8>,
    /// 0 进入页面时安装，1 预加载。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_type: Option<u8>,
    /// 主入口。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_url: Option<String>,
    /// 次入口。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_url: Option<String>,
    /// 虚拟域名。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vhost: Option<String>,
    /// 资源编码，用于账号与资源的授权关系。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_code: Option<String>,
    /// 扩展信息 JSON 字符串。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extend_info: Option<String>,
    /// 实例类型，自定义资源固定为 6。
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<u8>,
    /// 创建时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_create: Option<String>,
    /// 更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<String>,
}

/// 小程序、离线包和自定义资源包的列表项或详情。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInfo {
    /// 资源包主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Integer>,
    /// 应用编码，通常为 appId-workspaceId。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_code: Option<String>,
    /// 资源所属的应用实例 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h5_id: Option<String>,
    /// 应用实例名称。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h5_name: Option<String>,
    /// 资源包版本。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h5_version: Option<String>,
    /// 下载时机，0 仅 Wi-Fi，1 所有网络。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_install: Option<u8>,
    /// 安装时机，0 不预加载，1 预加载。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_type: Option<u8>,
    /// 客户端最低版本。旧版为 iOS;Android，mcube 2.59.0 起为 iOS;Android;Harmony。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_version_min: Option<String>,
    /// 客户端最高版本。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_version_max: Option<String>,
    /// 创建者。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    /// 更新者。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifier: Option<String>,
    /// 预留的调试地址，通常为空。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug_url: Option<String>,
    /// 资源文件下载地址。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    /// 扩展信息，服务端返回 JSON 字符串。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extend_info: Option<String>,
    /// 资源额外数据，包含图标、资源类型和小程序功能开关。
    #[serde(
        default,
        deserialize_with = "optional_json_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub extra_data: Option<JsonText<ResourceExtraData>>,
    /// 回退资源地址。用分号分隔内网地址和外网地址，内网在前。资源尚未生成时可为空。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_base_url: Option<String>,
    /// 包大小。有服务端返回数字字符串。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<Integer>,
    /// 创建时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_create: Option<String>,
    /// 更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<String>,
    /// 启动加载标志。文档中的小程序和离线包记录返回 0。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lazy_load: Option<u8>,
    /// 主入口。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_url: Option<String>,
    /// 次入口。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suburl: Option<String>,
    /// 资源文件的 MD5 校验值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
    /// 资源元数据下载地址。小程序使用 h5.json，离线包回退资源生成完成后会有值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    /// 元数据主键，文档标为无业务意义。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta_id: Option<Integer>,
    /// 小程序资源包类型，值为 1 至 4。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_type: Option<u8>,
    /// 资源支持的平台。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// 发布阶段。0 初始、1 内部灰度、2 外部灰度、3 正式、4 回滚、5 发布结束。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_period: Option<u8>,
    /// 资源类型。文档响应示例为字符串，0 全局离线包、1 普通离线包、4 小程序。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<Integer>,
    /// 发布版本标识。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_version: Option<String>,
    /// 记录状态。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u8>,
    /// 虚拟域名。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vhost: Option<String>,
    /// 小程序源码下载地址，mcube 2.44.0 起提供，无源码时为空。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mini_program_source_code_url: Option<String>,
    /// 绑定的插件，以半角逗号分隔。每项是插件 ID 或 ID:版本。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bind_mini_plugin_ids: Option<String>,
}

/// 资源 extraData 字段中的已知键。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceExtraData {
    /// 资源类型，服务端可能返回数字字符串。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<Integer>,
    /// 小程序图标地址。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    /// 底部导航栏开关，0 否、1 是。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_tab_bar: Option<Integer>,
    /// 右上角功能菜单开关，0 否、1 是。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_option_menu: Option<Integer>,
    /// 后台保活开关，0 否、1 是。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_keep_alive: Option<Integer>,
}

/// 热修复资源列表项。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotpatchResourceInfo {
    /// 热修复资源主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Integer>,
    /// 应用编码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_code: Option<String>,
    /// 原子修复标志。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub atomic: Option<u8>,
    /// 创建者。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    /// 调度表达式。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cronexpress: Option<String>,
    /// 热修复资源下载地址。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    /// 扩展信息。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_data: Option<String>,
    /// 热修复文件大小。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<Integer>,
    /// 全量修复标志。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_repair: Option<u8>,
    /// 创建时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_create: Option<String>,
    /// 更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<String>,
    /// 热修复包版本。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotpatch_version: Option<String>,
    /// 问题描述。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue_desc: Option<String>,
    /// 懒加载标志。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lazy_load: Option<u8>,
    /// 资源文件的 MD5 校验值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
    /// 修复备注。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    /// 更新者。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifier: Option<String>,
    /// 文件在对象存储中的路径。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oss_path: Option<String>,
    /// 关联的包主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_id: Option<Integer>,
    /// Android 或 iOS 平台。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// 产品编码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_id: Option<String>,
    /// 客户端产品版本。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_version: Option<String>,
    /// 当前发布阶段。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_period: Option<u8>,
    /// 发布版本号。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_version: Option<String>,
    /// 资源来源主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<Integer>,
    /// 资源来源主键列表。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ids: Option<String>,
    /// 来源名称。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    /// 来源类型。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    /// 资源状态。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u8>,
    /// 运行环境类型。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vm_type: Option<String>,
    /// 运行环境类型说明。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vm_type_desc: Option<String>,
}

/// 小程序、离线包、热修复及自定义资源包的发布任务列表项和详情。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInfo {
    /// 任务主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Integer>,
    /// 应用编码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_code: Option<String>,
    /// 主应用 ID，部分详情接口中未使用。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    /// 工作空间 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// 原子修复标志。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub atomic: Option<u8>,
    /// 关联资源基础信息主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_info_id: Option<Integer>,
    /// 业务类型，例如 nebula 或 hotpatch。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub biz_type: Option<String>,
    /// 保留的资源列表，文档中未使用且示例为空数组。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundles: Option<Vec<Value>>,
    /// 创建者，旧版中未使用。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    /// 调度表达式，旧版中未使用。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cronexpress: Option<String>,
    /// 关联资源的下载地址。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    /// 关联资源的扩展信息。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_data: Option<String>,
    /// 文件大小，旧版可返回空字符串。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<Integer>,
    /// 全量修复标志。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_repair: Option<u8>,
    /// 创建时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_create: Option<String>,
    /// 更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<String>,
    /// 格式化的更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified_str: Option<String>,
    /// 服务端解析后的规则树。部分详情返回原始的规则数组。
    #[serde(
        default,
        deserialize_with = "optional_json_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub grey_config_info: Option<JsonText<GreyRules>>,
    /// 时间窗灰度截止日期时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_endtime: Option<String>,
    /// 时间窗灰度截止时间字符串。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_endtime_data: Option<String>,
    /// 时间窗灰度人数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_num: Option<u64>,
    /// 第三方灰度检查地址。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_url: Option<String>,
    /// 问题说明，旧版中未使用。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue_desc: Option<String>,
    /// 资源 MD5 校验值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
    /// 发布描述。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    /// 更改任务的账号。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifier: Option<String>,
    /// 文件在对象存储中的路径。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oss_path: Option<String>,
    /// 要发布的资源包主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_id: Option<Integer>,
    /// 灰度发布百分比。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<u64>,
    /// 发布平台。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// 由 appId、workspaceId 和 H5App ID 等组成的产品编码。
    #[serde(alias = "product_id", skip_serializing_if = "Option::is_none")]
    pub product_id: Option<String>,
    /// 发布资源的产品版本。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_version: Option<String>,
    /// 0 默认、1 白名单、2 时间窗、5 第三方灰度。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_mode: Option<u8>,
    /// 资源发布阶段。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_period: Option<u8>,
    /// 2 灰度发布、3 正式发布。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_type: Option<u8>,
    /// 快速回滚标志。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quick_rollback: Option<u8>,
    /// 发布版本号。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_version: Option<String>,
    /// 关联资源 ID 列表。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub res_ids: Option<String>,
    /// 高级规则列表，操作值有数字和字符串两种格式。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_json_list: Option<Vec<RuleResult>>,
    /// 来源主键，旧版中未使用。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<Integer>,
    /// 来源名称。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    /// 来源类型。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    /// 任务记录状态。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u8>,
    /// 同步结果。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_result: Option<String>,
    /// 同步类型。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_type: Option<u8>,
    /// 任务名称。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_name: Option<String>,
    /// 0 待发布、1 发布中、2 已结束、3 暂停。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_status: Option<u8>,
    /// 任务类型。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_type: Option<u8>,
    /// 任务版本时间戳。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_version: Option<u64>,
    /// 升级提醒次数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade_notice_num: Option<u64>,
    /// 升级进度。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade_progress: Option<String>,
    /// 运行环境类型。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vm_type: Option<String>,
    /// 白名单详情。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitelist: Option<Vec<WhitelistInfo>>,
    /// 白名单主键列表，以半角逗号分隔。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitelist_ids: Option<String>,
    /// 0 普通版本、1 体验版本。是否返回取决于 mcube 版本。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experience_flag: Option<u8>,
    /// 预计生效时间，以服务端时区为准。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_effective_time: Option<String>,
}

/// 响应中返回的高级灰度规则。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleResult {
    /// 规则维度，例如 city 或 productVersion。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_element: Option<String>,
    /// 1 包含、2 不包含、3 范围内、4 范围外。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<Integer>,
    /// 旧规则类型字段，示例为 0。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_type: Option<String>,
    /// 规则值，多个值以半角逗号分隔。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// 范围下限。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value1: Option<String>,
    /// 范围上限。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value2: Option<String>,
}

/// 服务端把高级规则转换为可执行表达式后的规则树。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GreyRuleTree {
    /// 根节点逻辑。and 表示对子规则执行逻辑与。
    pub operator: String,
    /// 规则集合。
    pub sub_rules: Vec<GreySubRule>,
    /// 未命中规则时的默认返回值。
    pub default_result: bool,
}

/// 高级规则树中的一个条件。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GreySubRule {
    /// contains 包含、excludes 不包含、vLimitIn 范围内、vLimitOut 范围外。
    pub operator: String,
    /// 包含或排除操作使用字符串列表，范围操作使用 lower 和 upper。
    pub left: RuleOperand,
    /// 规则维度名称。
    pub right: String,
    /// 边界是否排除。服务端范围规则可能返回。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclusive: Option<bool>,
    /// 默认判定结果。
    pub default_result: bool,
}

/// 服务端返回的版本范围。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleRange {
    /// 范围下限。
    pub lower: String,
    /// 范围上限。
    pub upper: String,
}

/// 批量操作中的单项结果。外层调用成功时，仍需检查每项的 success。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    /// 对应实例 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h5_id: Option<String>,
    /// 对应版本。非资源操作可为空。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h5_version: Option<String>,
    /// 创建任务时对应 packageId，修改状态时对应 taskId。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Integer>,
    /// 失败原因。文档响应中的字段拼写为 messge，部分服务端使用 message。
    #[serde(alias = "message", skip_serializing_if = "Option::is_none")]
    pub messge: Option<String>,
    /// 该项是否处理成功。
    pub success: bool,
}

/// 小程序 API、域名或权限开关配置。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRecord {
    /// 配置主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Integer>,
    /// 应用编码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_code: Option<String>,
    /// 小程序 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h5_id: Option<String>,
    /// 小程序名称。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h5_name: Option<String>,
    /// 配置状态。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_status: Option<u8>,
    /// API、SERVER_DOMAIN、WEB_VIEW_DOMAIN 或 PRIVILEGE_SWITCH。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_type: Option<String>,
    /// 配置值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_value: Option<String>,
    /// 配置描述。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 创建时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_create: Option<String>,
    /// 更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<String>,
}

/// 小程序的权限配置详情。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniProgramConfigInfo {
    /// 应用编码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_code: Option<String>,
    /// 小程序 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h5_id: Option<String>,
    /// 已启用的服务端域名配置数量。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_server_domain_count: Option<Integer>,
    /// API 权限配置列表。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_config_list: Option<Vec<ConfigRecord>>,
    /// 服务器域名配置列表。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_domain_config_list: Option<Vec<ConfigRecord>>,
    /// WebView 域名配置列表。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webview_domain_config_list: Option<Vec<ConfigRecord>>,
}

/// 已配置的开放平台信息。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenPlatformInfo {
    /// 记录主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Integer>,
    /// 主应用 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    /// 工作空间 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// 开放平台公钥。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_pubkey: Option<String>,
    /// 开放平台调用地址。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_url: Option<String>,
    /// 创建时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_create: Option<String>,
    /// 更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<String>,
}

/// 高级发布规则资源。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceConfigInfo {
    /// 规则资源主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Integer>,
    /// 应用编码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_code: Option<String>,
    /// 创建者。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    /// 更新者。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifier: Option<String>,
    /// 创建时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_create: Option<String>,
    /// 更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<String>,
    /// 配置备注。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    /// 平台。platform 表示平台无关。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// 显示名称。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_name: Option<String>,
    /// 资源类型，例如 city。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    /// 资源类型的显示名称。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type_name: Option<String>,
    /// 规则匹配值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_value: Option<String>,
}

/// 升级包的资源列表项或资源基础信息。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeInfo {
    /// 升级资源主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Integer>,
    /// 应用编码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_code: Option<String>,
    /// iOS 正式版和 TestFlight 的下载地址。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appstore_url: Option<String>,
    /// 版本描述。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_log: Option<String>,
    /// 创建时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_create: Option<String>,
    /// 更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<String>,
    /// iOS 符号表下载地址。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ios_symbol: Option<String>,
    /// iOS 包类型，0 正式、1 企业、2 TestFlight。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_enterprise: Option<u8>,
    /// 0 扫码免验证码，1 需要验证码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub need_check: Option<u8>,
    /// Android 或 iOS 平台。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// 产品编码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_id: Option<String>,
    /// 升级资源版本。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_version: Option<String>,
    /// 0 初始、2 灰度、3 正式、5 结束。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_period: Option<u8>,
    /// needCheck 为 1 时生成的校验码。服务端可展示掩码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_code: Option<String>,
    /// 安装包下载地址。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    /// 64 位 Android 安装包下载地址，mcube 2.50.1 起提供。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub android64_download_url: Option<String>,
}

/// TestFlight 升级包附加信息。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestFlightConfig {
    /// 配置主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Integer>,
    /// 创建时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_create: Option<String>,
    /// 更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<String>,
    /// 允许下载的人数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_amount: Option<u64>,
    /// TestFlight 到期时间，格式为 yyyy-MM-dd HH:mm:ss。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalid_time: Option<String>,
    /// 关联的升级包主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade_id: Option<Integer>,
}

/// 升级包资源详情。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeDetail {
    /// 资源的基础信息。
    #[serde(rename = "upgradeBaseInfoDO")]
    pub upgrade_base_info_do: UpgradeInfo,
    /// TestFlight 配置，其他包类型返回 null。
    #[serde(rename = "mobileTestFlightConfigDO")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mobile_test_flight_config_do: Option<TestFlightConfig>,
}

/// 升级包发布任务的列表项和详情。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeTaskInfo {
    /// 任务主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Integer>,
    /// 应用编码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_code: Option<String>,
    /// 创建时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_create: Option<String>,
    /// 更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<String>,
    /// 高级规则。列表通常返回规则树，详情可能返回规则数组。
    #[serde(
        default,
        deserialize_with = "optional_json_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub grey_config_info: Option<JsonText<GreyRules>>,
    /// 时间窗截止时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_endtime: Option<String>,
    /// 时间窗截止时间字符串。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_endtime_data: Option<String>,
    /// 时间窗灰度人数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_num: Option<u64>,
    /// 是否检测历史版本的强制升级，0 否、1 是。开启后，客户端版本与最新版之间存在强制升级版本时，
    /// 客户端收到强制升级模式。当前任务结束后停止检测。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_force: Option<u8>,
    /// iOS 包类型，0 正式、1 企业、2 TestFlight。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_enterprise: Option<u8>,
    /// 发布描述。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    /// 对应的升级资源包主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_info_id: Option<Integer>,
    /// 升级包平台。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// 产品编码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_id: Option<String>,
    /// 目标版本号。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_version: Option<String>,
    /// 1 白名单、2 时间窗。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_mode: Option<u8>,
    /// 2 灰度、3 正式。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_type: Option<u8>,
    /// 高级规则对象列表，任务详情提供。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_json_list: Option<Vec<RuleResult>>,
    /// 0 待发布、1 发布中、2 已结束、3 暂停。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_status: Option<u8>,
    /// 客户端显示的升级提示。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade_content: Option<String>,
    /// 1 单次提醒、2 多次提醒、3 强制升级。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade_type: Option<u8>,
    /// 白名单主键列表。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitelist_ids: Option<String>,
    /// 按日、周或月设置的可升级时间段。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reminder_period: Option<JsonText<Vec<ReminderPeriod>>>,
}

/// 发布白名单的列表项或任务详情中的白名单。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhitelistInfo {
    /// 白名单主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Integer>,
    /// 应用编码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_code: Option<String>,
    /// 业务分类。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub business: Option<String>,
    /// 创建时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_create: Option<String>,
    /// 更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<String>,
    /// 用户标识类型。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_type: Option<String>,
    /// 白名单内容。采用独立存储的服务端可返回空值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_value: Option<String>,
    /// 平台字段，旧版未使用。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// 规则过滤信息，列表接口可为空或省略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_data: Option<WhitelistFilter>,
    /// 白名单记录状态。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u8>,
    /// 白名单条数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub white_list_count: Option<u64>,
    /// 白名单名称。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub white_list_name: Option<String>,
    /// 普通名单或正则名单。列表可返回大写 NORMAL 或 REGULAR。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitelist_type: Option<String>,
    /// 白名单的存储方式。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_type: Option<String>,
    /// 白名单的用户类型。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_type: Option<String>,
    /// 正则白名单的模式列表。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern_list: Option<Vec<String>>,
}

/// 任务白名单中的过滤信息。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhitelistFilter {
    /// 第一级过滤值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fst_filter: Option<ExtensionInfo>,
    /// 第二级过滤值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snd_filter: Option<ExtensionInfo>,
}

/// 添加白名单内容的结果。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhitelistModifyResult {
    /// 插入失败的用户数。
    pub fail_num: u64,
    /// 插入失败的用户 ID。
    pub fail_user_ids: String,
    /// 本次批次是否成功。
    pub success: bool,
    /// 添加成功的用户数。
    pub success_num: u64,
}

/// 单页白名单内容。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhitelistDetail {
    /// 当前页的白名单条目。
    pub data: Vec<String>,
    /// 是否还有下一页。
    pub has_more: bool,
    /// 主应用 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    /// 工作空间 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// 白名单主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitelist_id: Option<Integer>,
    /// 白名单名称。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitelist_name: Option<String>,
    /// normal 普通名单，regular 正则名单。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitelist_type: Option<String>,
}

/// 开关列表项或详情。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchRecord {
    /// 开关键的记录主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Integer>,
    /// 应用编码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_code: Option<String>,
    /// 业务名称，文档默认 default。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub business: Option<String>,
    /// 配置键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conf_key: Option<String>,
    /// 配置值或列表展示值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conf_value: Option<String>,
    /// 配置值的详情列表。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conf_value_infos: Option<Vec<SwitchValueRecord>>,
    /// 创建者。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    /// 创建时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_create: Option<String>,
    /// 格式化的创建时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_create_str: Option<String>,
    /// 更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<String>,
    /// 格式化的更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified_str: Option<String>,
    /// 配置备注。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    /// 更新者。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifier: Option<String>,
    /// 0 失效、1 激活。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u8>,
    /// 同步模式。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_mode: Option<String>,
    /// 同步结果。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_result: Option<String>,
    /// 传输状态。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trans_status: Option<u8>,
    /// 开关记录类型。
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<u8>,
}

/// 开关详情中的配置值。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchValueRecord {
    /// 配置值主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Integer>,
    /// 应用编码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_code: Option<String>,
    /// 配置键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conf_key: Option<String>,
    /// 配置值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conf_value: Option<String>,
    /// 创建者。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    /// 创建时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_create: Option<String>,
    /// 更新时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gmt_modified: Option<String>,
    /// 服务端生成的高级灰度规则。
    #[serde(
        default,
        deserialize_with = "optional_json_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub grey_config_info: Option<JsonText<GreyRules>>,
    /// 更新者。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifier: Option<String>,
    /// 百分比灰度取 1 到 99，其他模式为 100。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
    /// all、ios 或 android。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// 预置标志，文档示例为 0。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre: Option<u8>,
    /// 优先级，值越大优先级越高。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u64>,
    /// 1 白名单、3 百分比、4 全量。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_mode: Option<u8>,
    /// 2 灰度、3 正式。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_type: Option<u8>,
    /// 高级灰度规则。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_json_list: Option<Vec<RuleResult>>,
    /// 文档中配置值的任务状态为 1。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_status: Option<u8>,
    /// 白名单主键列表，多个以半角逗号分隔。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitelist_ids: Option<String>,
    /// 白名单名称和标识。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitelist_infos: Option<Vec<SwitchWhitelistInfo>>,
}

/// 开关详情中展示的白名单。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchWhitelistInfo {
    /// 白名单主键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<Integer>,
    /// 白名单显示名称。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// 单个开关键的导入结果。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchImportResult {
    /// 待导入的配置键。
    pub conf_key: String,
    /// 配置值级的处理详情。
    pub import_detail_list: Vec<SwitchImportDetail>,
    /// 配置键级的说明或错误。
    pub import_results: Vec<String>,
    /// 该配置键是否导入成功。
    pub success: bool,
}

/// 开关配置值的导入结果。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchImportDetail {
    /// 该条配置值。
    pub conf_value: String,
    /// 该配置值是否处理成功。
    pub success: bool,
    /// 配置值级的说明或错误。
    pub value_results: Vec<String>,
}

/// 旧版开关列表接口使用的内层分页对象。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchPage {
    /// 当前页的开关配置。
    pub list: Vec<SwitchRecord>,
    /// 是否第一页。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_page: Option<bool>,
    /// 本页记录的起始位置。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_result: Option<u64>,
    /// 是否最后一页。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_page: Option<bool>,
    /// 下一页页码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page: Option<i64>,
    /// 当前页码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_no: Option<u64>,
    /// 每页记录数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u64>,
    /// 上一页页码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_page: Option<i64>,
    /// 符合条件的记录总数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_count: Option<u64>,
    /// 总页数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_page: Option<u64>,
}
