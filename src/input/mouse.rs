use crate::core::CoreError;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEINPUT, SendInput,
};

pub(crate) fn send_left_click() -> Result<(), CoreError> {
    let input_size = std::mem::size_of::<INPUT>() as i32;
    let inputs = [
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dwFlags: MOUSEEVENTF_LEFTDOWN,
                    ..Default::default()
                },
            },
        },
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dwFlags: MOUSEEVENTF_LEFTUP,
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
