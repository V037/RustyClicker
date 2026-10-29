use crate::core::CoreError;
use serde::{Deserialize, Serialize};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_MOUSE, MOUSE_EVENT_FLAGS, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEINPUT, SendInput,
};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClickButton {
    #[default]
    Left,
    Right,
    Middle,
}

impl ClickButton {
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "Left Click",
            Self::Right => "Right Click",
            Self::Middle => "Middle Click",
        }
    }

    pub fn flags(self) -> (MOUSE_EVENT_FLAGS, MOUSE_EVENT_FLAGS) {
        match self {
            Self::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
            Self::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
            Self::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClickType {
    #[default]
    Single,
    Double,
}

impl ClickType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Single => "Single",
            Self::Double => "Double",
        }
    }
}

pub fn send_left_click() -> Result<(), CoreError> {
    send_mouse_click(ClickButton::Left)
}

pub fn send_mouse_click(button: ClickButton) -> Result<(), CoreError> {
    let (down_flag, up_flag) = button.flags();
    let input_size = std::mem::size_of::<INPUT>() as i32;
    let inputs = [
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dwFlags: down_flag,
                    ..Default::default()
                },
            },
        },
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dwFlags: up_flag,
                    ..Default::default()
                },
            },
        },
    ];
    let inserted = unsafe { SendInput(&inputs, input_size) };
    if inserted == inputs.len() as u32 {
        return Ok(());
    }

    if inserted > 0 {
        let release = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dwFlags: MOUSEEVENTF_LEFTUP,
                    ..Default::default()
                },
            },
        };
        if unsafe { SendInput(&[release], input_size) } != 1 {
            return Err(CoreError::InputDelivery {
                inserted,
                expected: inputs.len() as u32,
                release_failed: true,
            });
        }
    }
    Err(CoreError::InputDelivery {
        inserted,
        expected: inputs.len() as u32,
        release_failed: false,
    })
}

pub fn send_mouse_click_type(
    button: ClickButton,
    click_type: ClickType,
    position: Option<(i32, i32)>,
) -> Result<(), CoreError> {
    if let Some((x, y)) = position {
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::SetCursorPos(x, y);
        }
    }
    match click_type {
        ClickType::Single => send_mouse_click(button),
        ClickType::Double => {
            send_mouse_click(button)?;
            std::thread::sleep(std::time::Duration::from_millis(15));
            send_mouse_click(button)
        }
    }
}
