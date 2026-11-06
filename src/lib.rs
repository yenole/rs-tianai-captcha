//! Rust 核心验证码库：SLIDER、ROTATE、CONCAT 生成与校验。
//! HTTP 框架和外部存储后端留给调用方接入。

pub mod application;
pub mod cache;
pub mod error;
pub mod generator;
pub mod image_ops;
pub mod model;
pub mod validator;

pub use application::{CaptchaService, CheckResult};
pub use error::{CaptchaError, Result};
pub use generator::{CaptchaResponse, GenerateParam, ImageResource, ResourceSet, SliderTemplate};
pub use model::{CaptchaType, TrackPoint, UserTrack};
pub use validator::{DEFAULT_TOLERANT, check_percentage, validate_slider_percentage};
