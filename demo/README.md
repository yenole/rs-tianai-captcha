# Rust CAPTCHA Demo

这是参考 `tianaiyouqing/tianai-captcha-demo` 最小接入方式的 Rust 示例：页面使用 TAC Web SDK，服务启动后提供 `/`、`POST /gen` 和 `POST /check`。SDK 及 jQuery 静态文件随 demo 放在 `demo/vendor/`，避免依赖外部 CDN。

## 缺省资源

不配置资源目录时，服务尝试使用 `$HOME/download/1.jpg` 作为背景图（可用 `CAPTCHA_BACKGROUND_IMAGE=/path/to/image.jpg` 指定）；若文件不存在或不合适，则使用程序绘制的占位背景以及内置模板 PNG。内置 SLIDER 模板包含白色兔子脸和狐狸脸两种模板（均约 39×39），并保留深色五官以便辨识。生成接口可传 `{"type":"SLIDER","obfuscate":true}`，在真实缺口旁增加随机尺寸、半透明的干扰缺口；CONCAT 可传 `{"type":"CONCAT","obfuscate":true}`，在横向接缝叠加从背景随机采色的密集波形。网页有“显示干扰接缝 / 缺口”选项。ROTATE 模板为 128×128 橙色宽边圆环，中心透明以显示被抠出的背景图。ROTATE 蒙版为完整圆盘，圆环仅用于前景显示。动物模板文件在 `demo/resources/slider/{cat,bunny,fox}/`，每种有 `active.png`、`fixed.png`、`mask.png`。

## 自定义资源

服务默认从 `demo/resources` 读取资源，也可用 `CAPTCHA_RESOURCE_DIR` 指向自定义目录。根目录中的多张 `.jpg/.jpeg/.png/.webp/.bmp` 背景图以及 `backgrounds/` 子目录中的图片会在每次生成时随机挑选。将图片放入该目录即可，例如当前 `demo/resources/634-300x175.jpg`、`923-300x175.jpg`、`973-300x175.jpg`。

自定义模板可按以下结构放入资源目录；只使用你有权使用的图片：

```text
resources/
  background.jpg
  slider/
    active.png
    fixed.png
    mask.png       # 可选；缺省时使用 fixed.png
  rotate/          # 可选；缺省时回退到 slider 模板
    active.png
    fixed.png
    mask.png       # 可选
```

`active.png` 是透明滑块，`fixed.png` 是放置在背景上的缺口轮廓/遮罩。图片尺寸需满足模板小于背景。

## 启动

```sh
cargo run --bin demo
```

打开 <http://127.0.0.1:8080>，选择类型并拖动提交。

## API

生成请求：`POST /gen`，body `{"type":"SLIDER"}`（也支持 `ROTATE`、`CONCAT`）。成功响应为 TAC 结构 `{code:200,msg:"OK",data:{id,type,backgroundImage,templateImage,...}}`。

校验接口：`POST /check`，body 为 `{"id":"...","data":{"bgImageWidth":320,"bgImageHeight":180,"startTime":...,"stopTime":...,"trackList":[...]}}`。验证票据一次性使用，默认 120 秒过期。

CONCAT 页面会把上方移动切片的背景尺寸校准到完整图像比例，以避免 SDK 缩放切片时造成图案错乱。上下区域的横向拼接处额外绘制一条约 11px 高、沿整幅图宽度延伸的密集波形遮缝，不改变随机切分答案。

Demo 默认使用 TAC 的简单位置校验，而不是严格行为轨迹启发式检查；资源和图片几何算法目前用于演示，不应当作生产风控方案。
