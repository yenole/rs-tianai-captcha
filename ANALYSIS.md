# tianai-captcha 核心分析与 Rust 移植方案

## 1. 目标与范围

目标是将 [meitanzai/tianai-captcha](https://github.com/meitanzai/tianai-captcha)（仓库已迁移至 `dromara/tianai-captcha`）最核心的验证码生成、会话缓存和校验逻辑移植到当前 Rust crate。

已确认范围：

- 实现 3 类验证码：`SLIDER`、`ROTATE`、`CONCAT`。
- 同时移植生成和校验核心，包括 Java 的基础轨迹规则。
- 兼容现有 Java/Web SDK 数据契约：字段名、百分比容错、图片 MIME/data URI、单次校验语义。
- 当前阶段不实现 HTTP 服务、Redis、WASM 和 Java/Spring Boot 接入层；这些可作为后续适配 crate/模块。

上游仓库分析基于 `dromara/tianai-captcha` 源码（通过 `gitcode.com/dromara/tianai-captcha` 获取）。其许可证为 Apache-2.0（仓库根目录 `LICENSE` 为其原文副本）；移植与分发时应保留并遵守上游许可证及资源文件的授权要求。

## 2. 上游架构及请求数据流

上游 Java 核心模块可按职责归纳为：

1. **应用编排**：`application/DefaultImageCaptchaApplication.java`
   - `generateCaptcha` 生成 ID、调用 generator、从生成结果创建校验数据、写入带 TTL 的缓存，再返回 `ImageCaptchaVO`。
   - `matching` 读取并删除对应 ID 的校验数据，再调用 validator。验证码因此是一次性使用；即使答案错误，也不能再次试验同一 ID。
2. **类型分发与资源**：`generator/impl/MultiImageCaptchaGenerator.java`、`resource/*`
   - 按验证码类型选择 generator；资源管理器按类型和 tag 随机选背景图/模板图。
   - slider/rotate 模板文件包含 `active.png`、`fixed.png`，可选 `mask.png`。
3. **图片生成**：`generator/impl/Standard*ImageCaptchaGenerator.java` 与 `generator/common/util/CaptchaImageUtils.java`
   - 读背景/模板，生成缺口或重排图块，编码成前端图像字符串，并保留不返回给客户端的答案。
4. **答案转换与验证**：`validator/impl/SimpleImageCaptchaValidator.java`
   - 将内部答案转换为百分比并比对客户端轨迹。
5. **行为检测**：`validator/impl/BasicCaptchaTrackValidator.java`
   - 先做通用字段检查，再对滑动轨迹做时间、长度、起点、跳变、速度变化等基础检查。
6. **缓存**：`cache/CacheStore.java`、`cache/impl/LocalCacheStore.java`
   - 缓存以 key 保存校验数据，具有 TTL；校验使用原子 get-and-remove 语义。

应用级数据流：

```text
生成请求 -> 资源/模板随机选择 -> 对应 generator 合成图片 -> ImageCaptchaInfo
         -> 内部答案转为校验数据 -> ID + TTL 缓存 -> ImageCaptchaVO 返回
客户端提交 ID + ImageCaptchaTrack -> 原子读取并删除答案 -> 基础轨迹检查 -> 类型校验 -> 成功/失败
```

## 3. 三类验证码核心行为

### SLIDER

- 随机选择模板及背景，随机生成缺口 `(x,y)`。
- 对背景按模板 alpha 遮罩抠图，将 `fixed` 覆盖至背景缺口处；将抠出的图与 `active` 模板合成透明滑块图。
- 验证答案不直接存 x，而是 `expected_percentage = x / background_width`。
- 客户端以 `trackList` 首末两个 x 坐标差作为位移，计算 `actual_percentage = (last.x - first.x) / bgImageWidth`，与答案做绝对误差比较。默认容差 `0.02`，含边界。

### ROTATE

- 固定模板在背景中心抠取，旋转 active/fixed 后绘制；抠图被随机角度旋转并居中合成。
- 上游用背景宽度把角度映射为整数滑动坐标，校验通用滑动百分比逻辑。具体旋转比例须按 Web SDK 的 `ROTATE` 实际上报字段确认，避免只按 Java 内部命名推断。

### CONCAT

- 在背景上随机选水平/垂直切分点，水平切分上半部分后交换左右片段，再与下半部分重新拼接。
- 保留原始 x/y 作为答案/展示元数据；主要走滑动百分比校验，容差默认为 `0.05`。

## 4. Java 兼容性要求

### API JSON 字段

生成 VO 字段（使用 camelCase，不擅自重命名）：

```text
id, type, backgroundImage, templateImage,
backgroundImageTag, templateImageTag,
backgroundImageWidth, backgroundImageHeight,
templateImageWidth, templateImageHeight, data
```

滑动轨迹字段：

```text
bgImageWidth, bgImageHeight, templateImageWidth, templateImageHeight,
startTime, stopTime, left, top, trackList, data
```

轨迹点字段：`x`, `y`, `t`, `type`。类型字符串需兼容 `MOVE`、`CLICK` 等 SDK 值。JSON 对缺省/空字段的接受方式应采用 serde 默认/Option，输出字段名保持一致。

### 图片编码

- 默认背景 JPEG、模板 PNG；编码为 `data:image/{format};base64,{标准 Base64}`。
- 对 PNG alpha 必须保留；JPEG 无 alpha，需要定义与 Java ImageIO 兼容的默认底色并加编码测试。
- MIME 前缀应保留传入格式名语义（上游为 `jpg`/`jpeg`/`png` 等）。

### 校验和缓存

- 浮点百分比算法按上游公式 `pos / maxPos`；容差为 inclusive range `[expected-tolerance, expected+tolerance]`。
- ID 推荐保持 `{TYPE}_{uuid无连字符}` 格式；答案 TTL 默认 20 秒，可配置。
- 校验必须通过原子 `remove` 消费答案，避免重放/并发双验。
- 外部返回码/错误结构需与使用方定义的 Java API 封装对齐；当前核心 crate 可先提供强类型错误，未来 HTTP adapter 映射原响应码。

## 5. Rust 模块建议

当前项目采用单 crate，先保持轻量，按领域拆模块；有稳定 API 后再视需求 workspace 化：

```text
src/
  lib.rs                 # 公共导出
  model.rs               # CaptchaType、GenerateParam、CaptchaResponse、CaptchaTrack、TrackPoint
  error.rs               # CaptchaError
  image/
    mod.rs               # 解码、编码、alpha 合成、裁切、旋转、切分、拼接
    slider.rs
    rotate.rs
    concat.rs
  validator.rs           # slider percentage/track validation
  track.rs               # BasicCaptchaTrackValidator 的规则
  resource.rs            # 文件/内存图片资源、模板查找及 tag 选择
  cache.rs               # CacheStore trait、带 TTL 的内存实现、原子 take
  application.rs         # generate/check 门面与答案生命周期
```

推荐先实现 `SLIDER + cache + percentage validator + track validator` 的端到端闭环，再叠加其余类型。用户可通过文件路径配置背景/模板资源；不要将上游资源文件擅自复制进 crate，除非确认其具体许可可再分发。

## 6. 依赖选择（当前 Cargo.toml 已加入）

- `image`：解码/编码及基本像素操作（JPEG/PNG/WebP features）。
- `rand`：随机资源、位置、顺序；生产随机答案使用系统熵来源。
- `serde`：契约模型序列化；建议补 `serde_json` 测试与示例。
- `dashmap`：可选并发缓存；必须确保 get-and-remove 原子性与 TTL 清理。

实际应避免为了预生成验证码而一开始引入后台线程/复杂调度；上游 `CacheImageCaptchaGenerator` 是性能优化，不是正确性核心。

## 7. 上游实现值得注意的行为与风险

- `DefaultImageCaptchaApplication.matching` 使用 `getAndRemoveCache`，是一次性消费验证票据的重要安全语义。
- `SimpleImageCaptchaValidator` 基本滑块校验本身只比对首末位移百分比；强行为校验另由 `BasicCaptchaTrackValidator` 执行。
- `BasicCaptchaTrackValidator` 规则包括：滑动至少 300ms、轨迹点数 `10..=bgWidth*5`、首点 x/y 在 ±10 内、相邻 x/y 正向差不能大于 50、不得始终相同 y、越过背景宽度点数限制、后 30% 平均时间应慢于前 70%。必须用上游测试向量校准边界及其实现中的比较细节。
- 上游 `cutImage` 只在模板 alpha `>100` 时拷贝背景像素；合成则由 AWT `Graphics2D` 做 alpha blend。Rust 版需固定并测试 alpha 混合公式。
- Rust 应采用分离的公开 DTO 与私有 answer，禁止把目标坐标/百分比随响应暴露。

## 8. 当前 Rust 实现状态

Rust 当前范围为 `SLIDER`、`ROTATE`、`CONCAT`。实现已包含 `src/generator.rs`、`src/cache.rs`、`src/application.rs` 和 `src/image_ops.rs`：有基础图像生成、内存 TTL 一次性缓存和服务门面。

**目前尚不满足严格 Java 兼容验收**：

- ROTATE 参考 SDK `moveX / (end / 360)` 和 Java `degree = 360 - randomX * 360 / bgWidth`；目标正向拖动比例为 `degree/360 = 1 - randomX/bgWidth`。SDK 上报的 bgImageWidth 是 drag end，demo 会先将轨迹位移及宽度按生成时原图宽度归一化后比对。
- CONCAT 按 Java `splitImage` / `concatImage` 生成“顶部左右交换 + 底部原样”的重排背景；SDK 把顶区作为整幅宽度的背景切片并随拖动平移。目标百分比为 `randomX/backgroundWidth`，公开 `data.randomY` 用于裁定展示条高度，容差 0.05。
- 旋转采样从最近邻改为双线性插值，并保留 alpha；仍无法保证与 Java AWT 完全逐像素相同。SLIDER 混淆遮罩等细节也未完全复刻。
- `ResourceSet` 当前以字节 Vec 配置，不含 Java resource provider/tag 过滤机制。
- `BasicCaptchaTrackValidator` 的规则已接入滑块/旋转/CONCAT 的 app `check` 链路，并按上游检查时长、数量、初始坐标、跳变、y 波动、越界计数及后段速度。仍建议用真实 Web SDK 轨迹样本做 Java/Rust 对照测试。

因此这批实现提供可运行的端到端骨架，不应宣称完整替代 Java 版；生产使用前需完成上述兼容验证。

## 9. 里程碑及验收

1. **契约与纯算法**：强类型 DTO、图片编码 data URI、百分比计算的 Java 对照测试。
2. **SLIDER 端到端**：配置资源、生成图片与隐藏答案、缓存 TTL、单次验证、轨迹基础检查（已有固定答案正确/错误用例）。
3. **ROTATE / CONCAT**：几何变换、Java/Web SDK 真轨迹样本及逐字段对照测试。
4. **稳固性**：边界条件、并发 take、过期、错误输入、cargo fmt/clippy/test。

验收标准：测试覆盖三种验证码成功与失败用例；序列化字段名符合现有 SDK；答案数据不出现在公开响应；同一验证码 ID 只能完成一次校验。
