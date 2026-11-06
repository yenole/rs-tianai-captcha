use std::fmt::{Display, Formatter};

pub type Result<T> = std::result::Result<T, CaptchaError>;

#[derive(Debug)]
pub enum CaptchaError {
    InvalidInput(String),
    Image(image::ImageError),
}

impl Display for CaptchaError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(f, "invalid captcha input: {message}"),
            Self::Image(error) => Display::fmt(error, f),
        }
    }
}

impl std::error::Error for CaptchaError {}

impl From<image::ImageError> for CaptchaError {
    fn from(value: image::ImageError) -> Self {
        Self::Image(value)
    }
}
