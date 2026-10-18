use crate::core::CoreError;
use serde::{Deserialize, Serialize};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_MOUSE, MOUSE_EVENT_FLAGS, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEINPUT, SendInput,
};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ClickButton {
    #[default]
    Left,
    Right,
    Middle,
}

impl ClickButton {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Left => "Left Click",
            Self::Right => "Right Click",
            Self::Middle => "Middle Click",
        }
    }

    pub(crate) fn flags(self) -> (MOUSE_EVENT_FLAGS, MOUSE_EVENT_FLAGS) {
        match self {
            Self::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
            Self::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
            Self::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
        }
    }
}

pub(crate) fn send_left_click() -> Result<(), CoreError> {
    send_mouse_click(ClickButton::Left)
}

pub(crate) fn send_mouse_click(button: ClickButton) -> Result<(), CoreError> {
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
