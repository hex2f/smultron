use lazy_static::lazy_static;
use pc_keyboard::{layouts, DecodedKey, HandleControl, KeyCode, Keyboard, ScancodeSet1};
use spin::Mutex;
use x86_64::instructions::port::Port;

lazy_static! {
    static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> = Mutex::new(
        Keyboard::new(
            ScancodeSet1::new(),
            layouts::Us104Key,
            HandleControl::Ignore,
        )
    );
}

pub fn try_read_byte() -> Option<u8> {
    // 0x64: PS/2 status. Bit 0 means output buffer has a byte.
    let mut status_port = Port::<u8>::new(0x64);
    let status = unsafe { status_port.read() };
    if status & 0x01 == 0 {
        return None;
    }

    let mut data_port = Port::<u8>::new(0x60);
    let scancode = unsafe { data_port.read() };

    let mut keyboard = KEYBOARD.lock();
    let event = keyboard.add_byte(scancode).ok().flatten()?;
    match keyboard.process_keyevent(event)? {
        DecodedKey::Unicode(c) if c.is_ascii() => Some(c as u8),
        DecodedKey::RawKey(KeyCode::Return) | DecodedKey::RawKey(KeyCode::NumpadEnter) => {
            Some(b'\n')
        }
        DecodedKey::RawKey(KeyCode::Backspace) => Some(8),
        _ => None,
    }
}
