use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

/// Represents a successful template match on the screen or in an image.
#[pyclass]
#[derive(Clone)]
pub struct Match {
    #[pyo3(get)]
    pub x: u32,
    #[pyo3(get)]
    pub y: u32,
    #[pyo3(get)]
    pub width: u32,
    #[pyo3(get)]
    pub height: u32,
    #[pyo3(get)]
    pub center_x: u32,
    #[pyo3(get)]
    pub center_y: u32,
    #[pyo3(get)]
    pub score: f32,
}

#[pymethods]
impl Match {
    fn __repr__(&self) -> String {
        format!(
            "Match(x={}, y={}, width={}, height={}, center=({}, {}), score={:.4})",
            self.x, self.y, self.width, self.height, self.center_x, self.center_y, self.score
        )
    }

    /// Unpack match as (x, y, width, height, score)
    fn as_tuple(&self) -> (u32, u32, u32, u32, f32) {
        (self.x, self.y, self.width, self.height, self.score)
    }

    /// Unpack center coordinates as (center_x, center_y)
    fn as_center(&self) -> (u32, u32) {
        (self.center_x, self.center_y)
    }
}

/// Represents an individual OCR word with its bounding rectangle.
#[pyclass]
#[derive(Clone, Debug)]
pub struct OcrWord {
    #[pyo3(get)]
    pub text: String,
    #[pyo3(get)]
    pub x: i32,
    #[pyo3(get)]
    pub y: i32,
    #[pyo3(get)]
    pub width: i32,
    #[pyo3(get)]
    pub height: i32,
    #[pyo3(get)]
    pub center_x: i32,
    #[pyo3(get)]
    pub center_y: i32,
}

#[pymethods]
impl OcrWord {
    fn __repr__(&self) -> String {
        format!(
            "OcrWord(text='{}', x={}, y={}, width={}, height={}, center=({}, {}))",
            self.text, self.x, self.y, self.width, self.height, self.center_x, self.center_y
        )
    }

    fn as_center(&self) -> (i32, i32) {
        (self.center_x, self.center_y)
    }

    fn as_box(&self) -> (i32, i32, i32, i32) {
        (self.x, self.y, self.width, self.height)
    }
}

/// Represents a detected phrase and its clickable bounding area on screen.
#[pyclass]
#[derive(Clone, Debug)]
pub struct TextBox {
    #[pyo3(get)]
    pub phrase: String,
    #[pyo3(get)]
    pub center_x: i32,
    #[pyo3(get)]
    pub center_y: i32,
    #[pyo3(get)]
    pub x: i32,
    #[pyo3(get)]
    pub y: i32,
    #[pyo3(get)]
    pub width: i32,
    #[pyo3(get)]
    pub height: i32,
}

#[pymethods]
impl TextBox {
    fn __repr__(&self) -> String {
        format!(
            "TextBox(phrase='{}', center=({}, {}), box=({}, {}, {}, {}))",
            self.phrase, self.center_x, self.center_y, self.x, self.y, self.width, self.height
        )
    }

    fn as_center(&self) -> (i32, i32) {
        (self.center_x, self.center_y)
    }

    fn as_box(&self) -> (i32, i32, i32, i32) {
        (self.x, self.y, self.width, self.height)
    }
}

/// Pre-compiled multi-scale pyramid template for fast SIMD ZNCC search.
#[pyclass]
pub struct PreparedTemplate {
    pub(crate) inner: rusty_clicker_core::matching::PreparedTemplate,
}

#[pymethods]
impl PreparedTemplate {
    #[new]
    #[pyo3(signature = (source))]
    fn new(source: &str) -> PyResult<Self> {
        let img = image::open(source)
            .map_err(|e| PyValueError::new_err(format!("Failed to open image '{}': {}", source, e)))?
            .to_rgba8();
        let template = rusty_clicker_core::matching::PreparedTemplate::new(img)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to prepare template: {}", e)))?;
        Ok(Self { inner: template })
    }

    #[staticmethod]
    #[pyo3(signature = (data))]
    fn from_bytes(data: &[u8]) -> PyResult<Self> {
        let img = image::load_from_memory(data)
            .map_err(|e| PyValueError::new_err(format!("Failed to decode image from bytes: {}", e)))?
            .to_rgba8();
        let template = rusty_clicker_core::matching::PreparedTemplate::new(img)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to prepare template: {}", e)))?;
        Ok(Self { inner: template })
    }

    #[staticmethod]
    #[pyo3(signature = (rgba_bytes, width, height))]
    fn from_raw_rgba(rgba_bytes: &[u8], width: u32, height: u32) -> PyResult<Self> {
        let img = image::RgbaImage::from_raw(width, height, rgba_bytes.to_vec())
            .ok_or_else(|| PyValueError::new_err("Invalid buffer size for RGBA image"))?;
        let template = rusty_clicker_core::matching::PreparedTemplate::new(img)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to prepare template: {}", e)))?;
        Ok(Self { inner: template })
    }

    #[getter]
    fn width(&self) -> u32 {
        self.inner.width
    }

    #[getter]
    fn height(&self) -> u32 {
        self.inner.height
    }

    #[pyo3(signature = (screen_gray, width, height, confidence=0.90, threads=2))]
    fn find_in_gray(
        &self,
        py: Python<'_>,
        screen_gray: &[u8],
        width: u32,
        height: u32,
        confidence: f32,
        threads: usize,
    ) -> PyResult<Option<Match>> {
        let res = py
            .detach(|| {
                self.inner.find_match_detailed(
                    screen_gray,
                    width,
                    height,
                    confidence,
                    None,
                    threads,
                )
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Matching error: {}", e)))?;

        Ok(res.map(|m| Match {
            x: m.x,
            y: m.y,
            width: m.width,
            height: m.height,
            center_x: m.center_x(),
            center_y: m.center_y(),
            score: m.score,
        }))
    }

    #[pyo3(signature = (confidence=0.90, threads=2))]
    fn find_on_screen(
        &self,
        py: Python<'_>,
        confidence: f32,
        threads: usize,
    ) -> PyResult<Option<Match>> {
        let res = py
            .detach(|| {
                let mut cap = None;
                let mut gray = Vec::new();
                let (w, h, orig_x, orig_y) =
                    rusty_clicker_core::capture::capture_screen(&mut cap, &mut gray)
                        .map_err(|e| format!("Capture error: {}", e))?;
                let m = self
                    .inner
                    .find_match_detailed(&gray, w, h, confidence, None, threads)
                    .map_err(|e| format!("Match error: {}", e))?;

                Ok::<_, String>(m.map(|m| Match {
                    x: (m.x as i32 + orig_x) as u32,
                    y: (m.y as i32 + orig_y) as u32,
                    width: m.width,
                    height: m.height,
                    center_x: ((m.center_x() as i32) + orig_x) as u32,
                    center_y: ((m.center_y() as i32) + orig_y) as u32,
                    score: m.score,
                }))
            })
            .map_err(PyRuntimeError::new_err)?;

        Ok(res)
    }
}

/// High-performance GPU-accelerated DXGI desktop screen capture session.
/// Keeps DirectX 11 staging textures alive between frames for sub-millisecond captures.
#[pyclass]
pub struct ScreenCapture {
    cap: Option<rusty_clicker_core::capture::ScreenCapture>,
    gray_buffer: Vec<u8>,
    bgra_buffer: Vec<u8>,
    ocr: Option<rusty_clicker_core::ocr::FastOcr>,
}

#[pymethods]
impl ScreenCapture {
    #[new]
    fn new() -> Self {
        Self {
            cap: None,
            gray_buffer: Vec::new(),
            bgra_buffer: Vec::new(),
            ocr: None,
        }
    }

    /// Capture the screen to a 1-channel 8-bit grayscale buffer.
    /// Returns (width, height, bytes).
    fn capture_gray<'py>(&mut self, py: Python<'py>) -> PyResult<(u32, u32, Bound<'py, PyBytes>)> {
        let (w, h) = py
            .detach(|| {
                let (w, h, _, _) =
                    rusty_clicker_core::capture::capture_screen(&mut self.cap, &mut self.gray_buffer)
                        .map_err(|e| format!("Screen capture failed: {}", e))?;
                Ok::<_, String>((w, h))
            })
            .map_err(PyRuntimeError::new_err)?;

        let bytes = PyBytes::new(py, &self.gray_buffer);
        Ok((w, h, bytes))
    }

    /// Capture the screen to a 32-bit BGRA buffer.
    /// Returns (width, height, bytes).
    fn capture_bgra<'py>(&mut self, py: Python<'py>) -> PyResult<(u32, u32, Bound<'py, PyBytes>)> {
        let (w, h) = py
            .detach(|| {
                let (w, h, _, _) =
                    rusty_clicker_core::capture::capture_screen_bgra(&mut self.cap, &mut self.bgra_buffer)
                        .map_err(|e| format!("Screen capture failed: {}", e))?;
                Ok::<_, String>((w, h))
            })
            .map_err(PyRuntimeError::new_err)?;

        let bytes = PyBytes::new(py, &self.bgra_buffer);
        Ok((w, h, bytes))
    }

    /// Captures the screen and searches for a PreparedTemplate directly on it.
    #[pyo3(signature = (template, confidence=0.90, threads=2))]
    fn find_template(
        &mut self,
        py: Python<'_>,
        template: &PreparedTemplate,
        confidence: f32,
        threads: usize,
    ) -> PyResult<Option<Match>> {
        let res = py
            .detach(|| {
                let (w, h, orig_x, orig_y) =
                    rusty_clicker_core::capture::capture_screen(&mut self.cap, &mut self.gray_buffer)
                        .map_err(|e| format!("Screen capture failed: {}", e))?;
                let m = template
                    .inner
                    .find_match_detailed(&self.gray_buffer, w, h, confidence, None, threads)
                    .map_err(|e| format!("Match error: {}", e))?;

                Ok::<_, String>(m.map(|m| Match {
                    x: (m.x as i32 + orig_x) as u32,
                    y: (m.y as i32 + orig_y) as u32,
                    width: m.width,
                    height: m.height,
                    center_x: ((m.center_x() as i32) + orig_x) as u32,
                    center_y: ((m.center_y() as i32) + orig_y) as u32,
                    score: m.score,
                }))
            })
            .map_err(PyRuntimeError::new_err)?;

        Ok(res)
    }

    /// Reads all text on screen (or within a specified bounding region).
    /// region: optional (x, y, width, height) tuple to crop
    #[pyo3(signature = (region=None))]
    fn read_text(
        &mut self,
        py: Python<'_>,
        region: Option<(u32, u32, u32, u32)>,
    ) -> PyResult<String> {
        let ocr = self.ensure_ocr()?;
        py.detach(|| {
            let (text, _) = ocr
                .recognize_screen(&mut self.cap, &mut self.bgra_buffer, region)
                .map_err(|e| format!("OCR recognition failed: {}", e))?;
            Ok::<String, String>(text)
        })
        .map_err(PyRuntimeError::new_err)
    }

    /// Reads all text on screen and returns (full_text, list_of_words).
    #[pyo3(signature = (region=None))]
    fn read_text_words(
        &mut self,
        py: Python<'_>,
        region: Option<(u32, u32, u32, u32)>,
    ) -> PyResult<(String, Vec<OcrWord>)> {
        let ocr = self.ensure_ocr()?;
        let (text, words) = py
            .detach(|| {
                ocr.recognize_screen(&mut self.cap, &mut self.bgra_buffer, region)
                    .map_err(|e| format!("OCR recognition failed: {}", e))
            })
            .map_err(PyRuntimeError::new_err)?;

        let py_words = words
            .into_iter()
            .map(|w| OcrWord {
                center_x: w.center_x(),
                center_y: w.center_y(),
                text: w.text,
                x: w.x,
                y: w.y,
                width: w.width,
                height: w.height,
            })
            .collect();

        Ok((text, py_words))
    }

    /// Checks if a phrase is present on screen (or within a specified region).
    /// Case-insensitive, robust to spacing differences.
    #[pyo3(signature = (phrase, region=None))]
    fn contains_text(
        &mut self,
        py: Python<'_>,
        phrase: &str,
        region: Option<(u32, u32, u32, u32)>,
    ) -> PyResult<bool> {
        let text = self.read_text(py, region)?;
        Ok(rusty_clicker_core::ocr::text_contains_phrase(&text, phrase))
    }

    /// Locates the clickable center and bounding box of a phrase on screen.
    #[pyo3(signature = (phrase, region=None))]
    fn find_text(
        &mut self,
        py: Python<'_>,
        phrase: &str,
        region: Option<(u32, u32, u32, u32)>,
    ) -> PyResult<Option<TextBox>> {
        let ocr = self.ensure_ocr()?;
        let (_, words) = py
            .detach(|| {
                ocr.recognize_screen(&mut self.cap, &mut self.bgra_buffer, region)
                    .map_err(|e| format!("OCR recognition failed: {}", e))
            })
            .map_err(PyRuntimeError::new_err)?;

        if let Some((cx, cy, x, y, w, h)) =
            rusty_clicker_core::ocr::find_phrase_box(&words, phrase)
        {
            Ok(Some(TextBox {
                phrase: phrase.to_string(),
                center_x: cx,
                center_y: cy,
                x,
                y,
                width: w,
                height: h,
            }))
        } else {
            Ok(None)
        }
    }

    /// Extracts (x, y, z) numeric coordinates from a designated region on screen.
    #[pyo3(signature = (region=None))]
    fn read_coords(
        &mut self,
        py: Python<'_>,
        region: Option<(u32, u32, u32, u32)>,
    ) -> PyResult<Option<(f64, f64, f64)>> {
        let text = self.read_text(py, region)?;
        Ok(rusty_clicker_core::ocr::extract_coords_from_text(&text))
    }

    /// High-speed check for trick-or-treat prompt ("Knock" / "knok").
    /// Solves Issue 2.1 & 2.2 in arcadie-macro.
    #[pyo3(signature = (region=None))]
    fn is_knock_visible(
        &mut self,
        py: Python<'_>,
        region: Option<(u32, u32, u32, u32)>,
    ) -> PyResult<bool> {
        let text = self.read_text(py, region)?;
        Ok(rusty_clicker_core::ocr::text_contains_phrase(&text, "knock")
            || rusty_clicker_core::ocr::text_contains_phrase(&text, "knok"))
    }
}

impl ScreenCapture {
    fn ensure_ocr(&mut self) -> PyResult<rusty_clicker_core::ocr::FastOcr> {
        if self.ocr.is_none() {
            let ocr = rusty_clicker_core::ocr::FastOcr::new()
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to initialize Windows OCR: {}", e)))?;
            self.ocr = Some(ocr);
        }
        Ok(self.ocr.as_ref().unwrap().clone())
    }
}

/// One-liner to find an image template on screen.
#[pyfunction]
#[pyo3(signature = (template_path, confidence=0.90, threads=2))]
fn find_template(
    py: Python<'_>,
    template_path: &str,
    confidence: f32,
    threads: usize,
) -> PyResult<Option<Match>> {
    let template = PreparedTemplate::new(template_path)?;
    template.find_on_screen(py, confidence, threads)
}

/// One-liner to capture the current screen to grayscale.
#[pyfunction]
fn capture_screen<'py>(py: Python<'py>) -> PyResult<(u32, u32, Bound<'py, PyBytes>)> {
    let mut session = ScreenCapture::new();
    session.capture_gray(py)
}

/// One-liner to read all text on screen or in a region.
#[pyfunction]
#[pyo3(signature = (region=None))]
fn read_screen_text(py: Python<'_>, region: Option<(u32, u32, u32, u32)>) -> PyResult<String> {
    let mut session = ScreenCapture::new();
    session.read_text(py, region)
}

/// One-liner to check if text/phrase is visible on screen.
#[pyfunction]
#[pyo3(signature = (phrase, region=None))]
fn screen_contains_text(
    py: Python<'_>,
    phrase: &str,
    region: Option<(u32, u32, u32, u32)>,
) -> PyResult<bool> {
    let mut session = ScreenCapture::new();
    session.contains_text(py, phrase, region)
}

/// One-liner to find the center coordinates of a text phrase on screen.
#[pyfunction]
#[pyo3(signature = (phrase, region=None))]
fn find_text_on_screen(
    py: Python<'_>,
    phrase: &str,
    region: Option<(u32, u32, u32, u32)>,
) -> PyResult<Option<TextBox>> {
    let mut session = ScreenCapture::new();
    session.find_text(py, phrase, region)
}

/// One-liner to read (x, y, z) coordinates from screen.
#[pyfunction]
#[pyo3(signature = (region=None))]
fn read_coords(
    py: Python<'_>,
    region: Option<(u32, u32, u32, u32)>,
) -> PyResult<Option<(f64, f64, f64)>> {
    let mut session = ScreenCapture::new();
    session.read_coords(py, region)
}

/// Fast check for trick-or-treat prompt ("knock" / "knok").
#[pyfunction]
#[pyo3(signature = (region=None))]
fn is_knock_visible(py: Python<'_>, region: Option<(u32, u32, u32, u32)>) -> PyResult<bool> {
    let mut session = ScreenCapture::new();
    session.is_knock_visible(py, region)
}

/// Hardware mouse click injection via Win32 SendInput.
#[pyfunction]
#[pyo3(signature = (button="left", click_type="single", pos=None))]
fn click(
    button: &str,
    click_type: &str,
    pos: Option<(i32, i32)>,
) -> PyResult<()> {
    let btn = match button.to_lowercase().as_str() {
        "left" => rusty_clicker_core::input::ClickButton::Left,
        "right" => rusty_clicker_core::input::ClickButton::Right,
        "middle" => rusty_clicker_core::input::ClickButton::Middle,
        other => return Err(PyValueError::new_err(format!("Unknown button: '{}'. Use 'left', 'right', or 'middle'", other))),
    };

    let ct = match click_type.to_lowercase().as_str() {
        "single" => rusty_clicker_core::input::ClickType::Single,
        "double" => rusty_clicker_core::input::ClickType::Double,
        other => return Err(PyValueError::new_err(format!("Unknown click_type: '{}'. Use 'single' or 'double'", other))),
    };

    rusty_clicker_core::input::send_mouse_click_type(btn, ct, pos)
        .map_err(|e| PyRuntimeError::new_err(format!("Mouse click failed: {}", e)))
}

/// RustyClicker Python Extension Module
#[pymodule]
fn rusty_clicker(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Match>()?;
    m.add_class::<PreparedTemplate>()?;
    m.add_class::<ScreenCapture>()?;
    m.add_class::<OcrWord>()?;
    m.add_class::<TextBox>()?;
    m.add_function(wrap_pyfunction!(find_template, m)?)?;
    m.add_function(wrap_pyfunction!(capture_screen, m)?)?;
    m.add_function(wrap_pyfunction!(read_screen_text, m)?)?;
    m.add_function(wrap_pyfunction!(screen_contains_text, m)?)?;
    m.add_function(wrap_pyfunction!(find_text_on_screen, m)?)?;
    m.add_function(wrap_pyfunction!(read_coords, m)?)?;
    m.add_function(wrap_pyfunction!(is_knock_visible, m)?)?;
    m.add_function(wrap_pyfunction!(click, m)?)?;
    Ok(())
}
