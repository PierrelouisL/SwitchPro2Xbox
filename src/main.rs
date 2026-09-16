extern crate hidapi;
extern crate vigem_rust;

use hidapi::{DeviceInfo, HidApi};
use vigem_rust::{Client, ClientError, TargetHandle, X360Button, X360Report, Xbox360};

const VID: u16 = 0x057e; // Vendor ID for Nintendo
const PID: u16 = 0x2009; // Product ID for Nintendo Switch Pro

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ControllerState {
    // Left stick position
    left_stick_x: u8,
    left_stick_y: u8,
    left_stick_pressed: bool,
    // Right stick position
    right_stick_x: u8,
    right_stick_y: u8,
    right_stick_pressed: bool,
    // Button states
    button_a: bool,
    button_b: bool,
    button_x: bool, 
    button_y: bool,
    button_plus: bool,
    button_minus: bool,
    button_r1: bool,
    button_r2: bool,
    button_l1: bool,
    button_l2: bool,
    arrow_state: u8, // 0-8 with 8 being no arrow pressed, 0-7 being the direction pressed
}

#[derive(Debug, Clone)]
struct DeviceAndTarget {
    device_info: DeviceInfo,
    target_handle: Option<TargetHandle<Xbox360>>,
}

fn convert_bytes_to_controller_state(bytes: &[u8]) -> Option<ControllerState> {
    if bytes[0] != 0x3F || bytes.len() != 64 {
        // Only 0x3F corresponds to controller input data, other are internal data, so we ignore them
        return None;
    }
    Some(ControllerState {
        button_b: bytes[1] & 0x01 != 0,
        button_a: bytes[1] & 0x02 != 0,
        button_y: bytes[1] & 0x04 != 0,
        button_x: bytes[1] & 0x08 != 0,
        button_l1: bytes[1] & 0x10 != 0,
        button_r1: bytes[1] & 0x20 != 0,
        button_l2: bytes[1] & 0x40 != 0,
        button_r2: bytes[1] & 0x80 != 0,
        button_minus: bytes[2] & 0x01 != 0,
        button_plus: bytes[2] & 0x02 != 0,
        left_stick_pressed: bytes[2] & 0x04 != 0,
        right_stick_pressed: bytes[2] & 0x08 != 0,
        arrow_state: bytes[3],
        left_stick_x: bytes[5], // Invert the value to match the expected range
        left_stick_y: 255 - bytes[7],
        right_stick_x: bytes[9],
        right_stick_y: 255 - bytes[11],
    })
}

fn build_xbox_report(controller_state: &ControllerState) -> X360Report {
    let mut report = X360Report::default();

    if controller_state.button_a {
        report.buttons.insert(X360Button::A);
    }
    if controller_state.button_b {
        report.buttons.insert(X360Button::B);
    }
    if controller_state.button_x {
        report.buttons.insert(X360Button::X);
    }
    if controller_state.button_y {
        report.buttons.insert(X360Button::Y);
    }
    if controller_state.button_l1 {
        report.buttons.insert(X360Button::LEFT_SHOULDER);
    }
    if controller_state.button_r1 {
        report.buttons.insert(X360Button::RIGHT_SHOULDER);
    }
    if controller_state.button_l2 {
        report.left_trigger = 255; // Full press for left trigger
    }
    if controller_state.button_r2 {
        report.right_trigger = 255; // Full press for right trigger
    }
    if controller_state.button_minus {
        report.buttons.insert(X360Button::BACK);
    }
    if controller_state.button_plus {
        report.buttons.insert(X360Button::START);
    }

    // Map the arrow state to D-pad buttons
    match controller_state.arrow_state {
        0 => { 
            report.buttons.insert(X360Button::DPAD_UP); 
        },
        1 => { 
            report.buttons.insert(X360Button::DPAD_UP);
            report.buttons.insert(X360Button::DPAD_RIGHT); 
        },
        2 => { 
            report.buttons.insert(X360Button::DPAD_RIGHT); 
        },
        3 => { 
            report.buttons.insert(X360Button::DPAD_RIGHT); 
            report.buttons.insert(X360Button::DPAD_DOWN);
        },
        4 => { 
            report.buttons.insert(X360Button::DPAD_DOWN); 
        },
        5 => {
            report.buttons.insert(X360Button::DPAD_DOWN); 
            report.buttons.insert(X360Button::DPAD_LEFT);
        },
        6 => { 
            report.buttons.insert(X360Button::DPAD_LEFT); 
        },
        7 => { 
            report.buttons.insert(X360Button::DPAD_LEFT);
            report.buttons.insert(X360Button::DPAD_UP);
        },
        _ => {  }, // No direction pressed
    }

    // Map stick positions (assuming they are in the range of 0-255)
    report.thumb_lx = ((controller_state.left_stick_x as u16) << 8).wrapping_sub(32768) as i16;
    report.thumb_ly = ((controller_state.left_stick_y as u16) << 8).wrapping_sub(32768) as i16;
    report.thumb_rx = ((controller_state.right_stick_x as u16) << 8).wrapping_sub(32768) as i16;
    report.thumb_ry = ((controller_state.right_stick_y as u16) << 8).wrapping_sub(32768) as i16;

    report
}

fn create_device_and_target(api : &HidApi, client : &Client) -> Option<Vec<DeviceAndTarget>> {
    let dev: Vec<DeviceAndTarget> = api.device_list().filter(|device| { device.vendor_id() == VID && device.product_id() == PID }).map(|device| {
        DeviceAndTarget {
            device_info: device.clone(),
            target_handle: Some(client.new_x360_target().plug().unwrap().wait_for_ready().unwrap()),
        }
    }).collect();
    for d in &dev {
        println!("Found Switch Pro controller with sn : {}", d.device_info.serial_number().unwrap_or("Unknown"));
        println!("Device Path: {}", d.device_info.path().to_string_lossy());
        println!("Linked to virtual Xbox 360 controller sn {}", d.target_handle.as_ref().unwrap().serial_no());
    }
    if dev.is_empty() {
        None
    } else {
        Some(dev)
    }
}

fn main() -> Result<(), ClientError> {
    let api = hidapi::HidApi::new().unwrap();
    
    let client = Client::connect()?;
    let mut switch_remotes = create_device_and_target(&api, &client);
    let mut buf = [0u8; 64];

    loop {
        if let Some(ret) = &mut switch_remotes {
            for sw in ret {
                if let Ok(device) = sw.device_info.open_device(&api) {
                    let res = device.read_timeout(&mut buf[..], 100).unwrap();
                    if let Some(controller_state) = convert_bytes_to_controller_state(&buf[..res]) {
                        let report = build_xbox_report(&controller_state);
                        sw.target_handle.as_ref().unwrap().update(&report)?;
                    }
                }
            }
        } else {
            println!("No Switch Pro controllers found. Please connect one.");
            std::thread::sleep(std::time::Duration::from_secs(5));
            switch_remotes = create_device_and_target(&api, &client);
            continue;
        }
    }
}
