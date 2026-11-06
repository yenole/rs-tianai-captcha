# rs-tianai-captcha

用 Rust 移植 [dromara/tianai-captcha](https://github.com/dromara/tianai-captcha) 的验证码核心：支持 `SLIDER`（滑块）、`ROTATE`（旋转）、`CONCAT`（拼接）三类行为验证码的生成与校验，并尽量保持与官方 Java / TAC Web SDK 的数据契约兼容。

- 单 crate，只做核心逻辑：图片生成、一次性答案缓存、百分比校验、基础轨迹校验。
- 不包含 HTTP 框架、Redis、WASM、Spring Boot 适配层；`demo` 里有一个 axum 示例服务可以直接跑起来。
- 移植物与上游许可证（MulanPSL-2.0）的兼容说明见 [`ANALYSIS.md`](./ANALYSIS.md)。

## 快速开始

```sh
cargo test          # 运行单元测试与端到端用例
cargo run --bin demo
```

打开 <http://127.0.0.1:8080>，选择验证码类型后拖动提交。Demo 的资源配置、接口字段和页面行为说明见 [`demo/README.md`](./demo/README.md)。

## 作为库使用

```rust
use std::time::Duration;
use tianai_captcha::{
    CaptchaService, CaptchaType, CheckResult, GenerateParam, ImageResource, ResourceSet,
    SliderTemplate, UserTrack,
};

// 1. 配置资源：背景图 + 滑块模板（active / fixed，可选 mask）
let resources = ResourceSet {
    backgrounds: vec![ImageResource { tag: "bg1".into(), bytes: bg_bytes }],
    slider_templates: vec![SliderTemplate {
        tag: "default".into(),
        active: active_png_bytes,
        fixed: fixed_png_bytes,
        mask: None, // 缺省时用 fixed 作为抠图遮罩
    }],
    rotate_templates: vec![/* 同上，ROTATE 用 */],
};

// 2. 创建服务（共享缓存、分配 TTL）
let service = CaptchaService::new(resources)
    .with_ttl(Duration::from_secs(20))   // 默认 20 秒
    .with_behavior_check(false);         // 默认关闭轨迹启发式检查

// 3. 生成
let resp = service.generate(GenerateParam {
    r#type: CaptchaType::Slider,
    obfuscate: true, // 额外生成干扰缺口
    ..Default::default()
})?;
// resp.id / resp.background_image / resp.template_image 可直接下发前端

// 4. 校验（id 与前端回传的轨迹）
match service.check(&resp.id, &track)? {
    CheckResult::Success => {}
    CheckResult::Failed => {}
    CheckResult::Expired => {}
}
```

要点：

- `generate` 生成的答案只保存在内存缓存中，**不会随响应下发**。
- `check` 先原子取出并删除答案，再比对结果；同一 `id` 无论成功失败都只能校验一次（与 Java `getAndRemoveCache` 一致）。
- `id` 形如 `SLIDER_{32位十六进制}`，`check` 从前缀推断验证码类型。

## 校验规则

| 类型 | 答案（内部） | 客户端位移 | 容差 |
| --- | --- | --- | --- |
| `SLIDER` | `缺口x / 背景宽` | `(末点x - 首点x) / bgImageWidth` | `0.02`（含边界） |
| `ROTATE` | `1 - randomX / 背景宽` | 同滑块公式；若 `bgImageWidth` 是滑动行程而非图片宽，会按答案里的 `backgroundWidth` 归一 | `0.03` |
| `CONCAT` | `切分x / 背景宽` | 同滑块公式 | `0.05` |

`with_behavior_check(true)` 会额外执行上游 `BasicCaptchaTrackValidator` 的基础规则：总时长 ≥ 300ms、轨迹点数在 `10..=bgWidth*5`、首点 x/y 在 ±10 内、相邻点正向跳动不超过 50、y 不能全程相同、越界点数限制、后 30% 平均时间需慢于前 70%。此检查是启发式的，用于演示与轻量风控，不能代替生产级风控。

> 注意：SDK 上报的是 `pageX/pageY` 绝对坐标，而轨迹校验期望拖动相对坐标。调用方需先把首个点平移到 `(0,0)` 附近（demo 在 `normalize_track_origin` 中演示）。

## 模块结构

```text
src/
  lib.rs           # 公共导出
  model.rs         # CaptchaType、TrackPoint、UserTrack（camelCase 序列化）
  error.rs         # CaptchaError / Result
  generator.rs     # 三类验证码的图片合成与答案构造
  image_ops.rs     # 解码、data URI 编码、按遮罩抠图、alpha 叠加、双线性旋转
  validator.rs     # 百分比校验与基础轨迹规则
  cache.rs         # DashMap 实现的内存 TTL 缓存（原子 take）
  application.rs   # CaptchaService：generate / check 门面
  bin/demo.rs      # axum 演示服务
demo/              # 前端页面、TAC Web SDK、示例资源
tests/core_flow.rs # 端到端用例
```

## 响应字段

生成响应字段与 TAC SDK 对齐（camelCase）：

```text
id, type, backgroundImage, templateImage,
backgroundImageTag, templateImageTag,
backgroundImageWidth, backgroundImageHeight,
templateImageWidth, templateImageHeight, data
```

图片为 `data:image/{format};base64,...`，默认背景 JPEG、模板 PNG（PNG 保留 alpha）。`CONCAT` 会额外返回 `data.randomY`。

提交轨迹字段：

```text
bgImageWidth, bgImageHeight, templateImageWidth, templateImageHeight,
startTime, stopTime, left, top, trackList, data
```

其中 `trackList` 元素为 `{x, y, t, type}`，`type` 兼容 `MOVE` / `CLICK` / `down` / `up` 等 SDK 取值。

## 资源要求

- `active.png`：透明滑块图；`fixed.png`：缺口轮廓 / 遮罩；`mask.png` 可选，缺省用 `fixed.png`。
- 模板需要能被背景容纳（`SLIDER`/`ROTATE` 要求 `fixed.width() * 2 + 15 < 背景宽` 且高度不超过背景）。
- 仓库内的示例图片仅用于演示，请只使用你有权使用的素材；上游资源文件的重分发需遵守其许可证。

## 已知限制

- 图像合成（抠图 alpha 处理、旋转插值、CONCAT 波形遮缝）与 Java AWT 并非逐像素一致。
- `ResourceSet` 只提供字节数组形式，没有移植上游的 resource provider / tag 过滤机制。
- 轨迹规则按上游实现推断，尚未用大量真实 SDK 轨迹做 Java/Rust 对照回归。
- 缓存只有进程内实现，多实例部署需自行替换为外部存储。

细节、上游行为对照与后续里程碑见 [`ANALYSIS.md`](./ANALYSIS.md)。

## 许可证

移植物基于上游 `dromara/tianai-captcha`（MulanPSL-2.0）。分发前请确认并保留上游许可证与资源授权要求。
