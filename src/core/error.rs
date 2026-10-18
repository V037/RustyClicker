use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CoreError {
    InvalidImageDimensions,
    DimensionsTooLarge(&'static str),
    InvalidPixelBuffer {
        format: &'static str,
        expected: usize,
        actual: usize,
    },
    TemplateCreation(String),
    TemplateCompilation(String),
    MatcherInput(String),
    MatcherExecution(String),
    MatcherPool(String),
    UnsupportedThreadCount(usize),
    ScreenCapture(Box<Self>),
    CaptureDimensionsTooLarge,
    CaptureScreenDeviceContext,
    CaptureMemoryDeviceContext,
    CaptureBitmap(String),
    CaptureBitmapSelection,
    CaptureScreenSize,
    CaptureNotInitialized,
    CapturePixels(String),
    FailSafeTriggered,
    CursorPosition,
    CursorMove,
    InputDelivery {
        inserted: u32,
        expected: u32,
        release_failed: bool,
    },
}

impl Display for CoreError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidImageDimensions => {
                formatter.write_str("Image dimensions must be non-zero.")
            }
            Self::DimensionsTooLarge(context) => {
                write!(formatter, "{context} dimensions are too large.")
            }
            Self::InvalidPixelBuffer {
                format,
                expected,
                actual,
            } => write!(
                formatter,
                "{format} image data has {actual} bytes; expected {expected}."
            ),
            Self::TemplateCreation(error) => {
                write!(
                    formatter,
                    "Could not create image matcher template: {error}"
                )
            }
            Self::TemplateCompilation(error) => {
                write!(
                    formatter,
                    "Could not compile image matcher template: {error}"
                )
            }
            Self::MatcherInput(error) => {
                write!(
                    formatter,
                    "Could not prepare screenshot for matching: {error}"
                )
            }
            Self::MatcherExecution(error) => write!(formatter, "Image matching failed: {error}"),
            Self::MatcherPool(error) => {
                write!(
                    formatter,
                    "Could not initialize image-matching workers: {error}"
                )
            }
            Self::UnsupportedThreadCount(threads) => {
                write!(formatter, "Unsupported matching thread count: {threads}.")
            }
            Self::ScreenCapture(error) => write!(formatter, "Screen capture failed: {error}"),
            Self::CaptureDimensionsTooLarge => {
                formatter.write_str("Screen dimensions are too large to capture safely.")
            }
            Self::CaptureScreenDeviceContext => {
                formatter.write_str("Could not get the screen device context.")
            }
            Self::CaptureMemoryDeviceContext => {
                formatter.write_str("Could not create a compatible device context.")
            }
            Self::CaptureBitmap(error) => {
                write!(
                    formatter,
                    "Could not create a direct-access screenshot bitmap: {error}"
                )
            }
            Self::CaptureBitmapSelection => {
                formatter.write_str("Could not select the screenshot bitmap.")
            }
            Self::CaptureScreenSize => {
                formatter.write_str("Windows reported an invalid screen size.")
            }
            Self::CaptureNotInitialized => {
                formatter.write_str("Could not initialize screen capture.")
            }
            Self::CapturePixels(error) => {
                write!(formatter, "Could not copy screen pixels: {error}")
            }
            Self::FailSafeTriggered => {
                formatter.write_str("Stopped by the fail-safe (mouse moved to a screen corner).")
            }
            Self::CursorPosition => {
                formatter.write_str("Could not read the mouse cursor position.")
            }
            Self::CursorMove => formatter.write_str("Could not move the mouse cursor."),
            Self::InputDelivery {
                inserted,
                expected: _,
                release_failed,
            } if *release_failed => formatter.write_str(
                "Windows accepted only part of the mouse click and could not release the button.",
            ),
            Self::InputDelivery {
                inserted, expected, ..
            } => write!(
                formatter,
                "Windows accepted only {inserted} of {expected} mouse click events."
            ),
        }
    }
}

impl std::error::Error for CoreError {}
