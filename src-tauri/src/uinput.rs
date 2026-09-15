//! A minimal uinput binding: enough to create a virtual joystick with force
//! feedback and to receive the effects games upload to it.
//!
//! Written against `libc`'s structs directly because the `evdev` crate drops
//! the condition parameters of damper/inertia effects, which racing games use
//! all the time. The ioctl numbers are computed the way `<linux/uinput.h>` does.

use std::fs::{File, OpenOptions};
use std::io;
use std::mem::size_of;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;

use libc::{ff_effect, input_event, uinput_abs_setup, uinput_ff_erase, uinput_ff_upload, uinput_setup};

// --- event types and codes (linux/input-event-codes.h) ----------------------
pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_ABS: u16 = 0x03;
pub const EV_FF: u16 = 0x15;
pub const EV_UINPUT: u16 = 0x0101;
pub const SYN_REPORT: u16 = 0;

pub const ABS_X: u16 = 0x00;
pub const ABS_Y: u16 = 0x01;
pub const ABS_Z: u16 = 0x02;
pub const ABS_RZ: u16 = 0x05;
pub const ABS_HAT0X: u16 = 0x10;
pub const ABS_HAT0Y: u16 = 0x11;

pub const EV_REL: u16 = 0x02;
pub const REL_WHEEL: u16 = 0x08;
pub const REL_HWHEEL: u16 = 0x06;

pub const BTN_LEFT: u16 = 0x110;
pub const BTN_RIGHT: u16 = 0x111;
pub const BTN_MIDDLE: u16 = 0x112;
pub const BTN_SIDE: u16 = 0x113;
pub const BTN_EXTRA: u16 = 0x114;
pub const BTN_FORWARD: u16 = 0x115;
pub const BTN_BACK: u16 = 0x116;
pub const BTN_TASK: u16 = 0x117;
pub const BTN_TRIGGER: u16 = 0x120;
pub const BTN_TRIGGER_HAPPY: u16 = 0x2c0;
/// Highest key/button code we declare on virtual keyboards.
pub const KEY_MAX: u16 = 0x2ff;

pub const FF_RUMBLE: u16 = 0x50;
pub const FF_PERIODIC: u16 = 0x51;
pub const FF_CONSTANT: u16 = 0x52;
pub const FF_SPRING: u16 = 0x53;
pub const FF_FRICTION: u16 = 0x54;
pub const FF_DAMPER: u16 = 0x55;
pub const FF_INERTIA: u16 = 0x56;
pub const FF_RAMP: u16 = 0x57;
pub const FF_SQUARE: u16 = 0x58;
pub const FF_TRIANGLE: u16 = 0x59;
pub const FF_SINE: u16 = 0x5a;
pub const FF_SAW_UP: u16 = 0x5b;
pub const FF_SAW_DOWN: u16 = 0x5c;
pub const FF_GAIN: u16 = 0x60;
pub const FF_AUTOCENTER: u16 = 0x61;

pub const UI_FF_UPLOAD: u16 = 1;
pub const UI_FF_ERASE: u16 = 2;

pub const BUS_USB: u16 = 0x03;

// --- ioctl numbers --------------------------------------------------------------
const IOC_WRITE: u64 = 1;
const IOC_READ: u64 = 2;
const UINPUT_IOCTL_BASE: u64 = b'U' as u64;

const fn ioc(dir: u64, nr: u64, size: u64) -> u64 {
    (dir << 30) | (size << 16) | (UINPUT_IOCTL_BASE << 8) | nr
}
const fn io(nr: u64) -> u64 {
    ioc(0, nr, 0)
}
const fn iow(nr: u64, size: usize) -> u64 {
    ioc(IOC_WRITE, nr, size as u64)
}
const fn iowr(nr: u64, size: usize) -> u64 {
    ioc(IOC_READ | IOC_WRITE, nr, size as u64)
}

const UI_DEV_CREATE: u64 = io(1);
const UI_DEV_DESTROY: u64 = io(2);
const UI_DEV_SETUP: u64 = iow(3, size_of::<uinput_setup>());
const UI_ABS_SETUP: u64 = iow(4, size_of::<uinput_abs_setup>());
const UI_SET_EVBIT: u64 = iow(100, size_of::<libc::c_int>());
const UI_SET_KEYBIT: u64 = iow(101, size_of::<libc::c_int>());
const UI_SET_RELBIT: u64 = iow(102, size_of::<libc::c_int>());
const UI_SET_ABSBIT: u64 = iow(103, size_of::<libc::c_int>());
const UI_SET_FFBIT: u64 = iow(107, size_of::<libc::c_int>());
const UI_BEGIN_FF_UPLOAD: u64 = iowr(200, size_of::<uinput_ff_upload>());
const UI_END_FF_UPLOAD: u64 = iow(201, size_of::<uinput_ff_upload>());
const UI_BEGIN_FF_ERASE: u64 = iowr(202, size_of::<uinput_ff_erase>());
const UI_END_FF_ERASE: u64 = iow(203, size_of::<uinput_ff_erase>());
/// `EVIOCGRAB` = `_IOW('E', 0x90, int)`.
const EVIOCGRAB: u64 = (IOC_WRITE << 30) | ((size_of::<libc::c_int>() as u64) << 16) | ((b'E' as u64) << 8) | 0x90;

fn check(ret: libc::c_int) -> io::Result<libc::c_int> {
    if ret < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(ret)
    }
}

unsafe fn ioctl_int(fd: libc::c_int, req: u64, value: libc::c_int) -> io::Result<()> {
    check(libc::ioctl(fd, req as _, value)).map(|_| ())
}

unsafe fn ioctl_ptr<T>(fd: libc::c_int, req: u64, value: *mut T) -> io::Result<()> {
    check(libc::ioctl(fd, req as _, value)).map(|_| ())
}

/// An absolute axis and its range.
pub struct Axis {
    pub code: u16,
    pub min: i32,
    pub max: i32,
    pub fuzz: i32,
    pub flat: i32,
}

/// A virtual input device with force feedback.
pub struct VirtualDevice {
    file: File,
}

impl VirtualDevice {
    /// Creates the device. `ff_effects_max` is how many effects a game may
    /// keep uploaded at once.
    pub fn create(
        name: &str,
        vendor: u16,
        product: u16,
        axes: &[Axis],
        buttons: &[u16],
        ff_effects_max: u32,
    ) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/uinput")?;
        let fd = file.as_raw_fd();

        unsafe {
            ioctl_int(fd, UI_SET_EVBIT, EV_SYN as _)?;
            ioctl_int(fd, UI_SET_EVBIT, EV_KEY as _)?;
            ioctl_int(fd, UI_SET_EVBIT, EV_ABS as _)?;
            ioctl_int(fd, UI_SET_EVBIT, EV_FF as _)?;
            for b in buttons {
                ioctl_int(fd, UI_SET_KEYBIT, *b as _)?;
            }
            for a in axes {
                ioctl_int(fd, UI_SET_ABSBIT, a.code as _)?;
                let mut setup: uinput_abs_setup = std::mem::zeroed();
                setup.code = a.code;
                setup.absinfo.minimum = a.min;
                setup.absinfo.maximum = a.max;
                setup.absinfo.fuzz = a.fuzz;
                setup.absinfo.flat = a.flat;
                ioctl_ptr(fd, UI_ABS_SETUP, &mut setup)?;
            }
            for code in [
                FF_CONSTANT, FF_SPRING, FF_DAMPER, FF_FRICTION, FF_INERTIA, FF_RAMP, FF_PERIODIC, FF_SQUARE,
                FF_TRIANGLE, FF_SINE, FF_SAW_UP, FF_SAW_DOWN, FF_GAIN, FF_AUTOCENTER,
            ] {
                ioctl_int(fd, UI_SET_FFBIT, code as _)?;
            }

            let mut setup: uinput_setup = std::mem::zeroed();
            setup.id.bustype = BUS_USB;
            setup.id.vendor = vendor;
            setup.id.product = product;
            setup.id.version = 1;
            setup.ff_effects_max = ff_effects_max;
            let bytes = name.as_bytes();
            let n = bytes.len().min(libc::UINPUT_MAX_NAME_SIZE - 1);
            for (i, b) in bytes[..n].iter().enumerate() {
                setup.name[i] = *b as libc::c_char;
            }
            ioctl_ptr(fd, UI_DEV_SETUP, &mut setup)?;
            ioctl_int(fd, UI_DEV_CREATE, 0)?;
        }
        Ok(VirtualDevice { file })
    }

    /// A virtual keyboard + mouse-button + scroll device, for injecting the
    /// keys and clicks that assignments and macros produce. Every key code up
    /// to `KEY_MAX` is declared so any mapping works.
    pub fn create_keyboard(name: &str, vendor: u16, product: u16) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/uinput")?;
        let fd = file.as_raw_fd();
        unsafe {
            ioctl_int(fd, UI_SET_EVBIT, EV_SYN as _)?;
            ioctl_int(fd, UI_SET_EVBIT, EV_KEY as _)?;
            ioctl_int(fd, UI_SET_EVBIT, EV_REL as _)?;
            for code in 1..=KEY_MAX {
                let _ = ioctl_int(fd, UI_SET_KEYBIT, code as _);
            }
            ioctl_int(fd, UI_SET_RELBIT, REL_WHEEL as _)?;
            ioctl_int(fd, UI_SET_RELBIT, REL_HWHEEL as _)?;

            let mut setup: uinput_setup = std::mem::zeroed();
            setup.id.bustype = BUS_USB;
            setup.id.vendor = vendor;
            setup.id.product = product;
            setup.id.version = 1;
            let bytes = name.as_bytes();
            let n = bytes.len().min(libc::UINPUT_MAX_NAME_SIZE - 1);
            for (i, b) in bytes[..n].iter().enumerate() {
                setup.name[i] = *b as libc::c_char;
            }
            ioctl_ptr(fd, UI_DEV_SETUP, &mut setup)?;
            ioctl_int(fd, UI_DEV_CREATE, 0)?;
        }
        Ok(VirtualDevice { file })
    }

    /// Emits events followed by a SYN_REPORT.
    pub fn emit(&self, events: &[(u16, u16, i32)]) -> io::Result<()> {
        let mut buf: Vec<input_event> = Vec::with_capacity(events.len() + 1);
        for (type_, code, value) in events {
            let mut ev: input_event = unsafe { std::mem::zeroed() };
            ev.type_ = *type_;
            ev.code = *code;
            ev.value = *value;
            buf.push(ev);
        }
        let mut syn: input_event = unsafe { std::mem::zeroed() };
        syn.type_ = EV_SYN;
        syn.code = SYN_REPORT;
        buf.push(syn);
        let bytes = unsafe {
            std::slice::from_raw_parts(buf.as_ptr() as *const u8, buf.len() * size_of::<input_event>())
        };
        let n = unsafe { libc::write(self.file.as_raw_fd(), bytes.as_ptr() as *const _, bytes.len()) };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Reads pending events from the kernel (FF uploads, plays, gain…).
    pub fn read_events(&self) -> io::Result<Vec<input_event>> {
        let mut out = Vec::new();
        let mut buf = [0u8; size_of::<input_event>() * 32];
        loop {
            let n = unsafe { libc::read(self.file.as_raw_fd(), buf.as_mut_ptr() as *mut _, buf.len()) };
            if n < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::WouldBlock {
                    break;
                }
                return Err(err);
            }
            if n == 0 {
                break;
            }
            let count = n as usize / size_of::<input_event>();
            for i in 0..count {
                let ev: input_event = unsafe {
                    std::ptr::read_unaligned(buf.as_ptr().add(i * size_of::<input_event>()) as *const input_event)
                };
                out.push(ev);
            }
        }
        Ok(out)
    }

    /// Completes an `UI_FF_UPLOAD` request: fetches the effect the game sent,
    /// lets the caller accept it (returning the slot id) and acknowledges.
    pub fn handle_upload(&self, request_id: i32, accept: impl FnOnce(&ff_effect) -> Result<i16, i32>) -> io::Result<()> {
        let fd = self.file.as_raw_fd();
        let mut up: uinput_ff_upload = unsafe { std::mem::zeroed() };
        up.request_id = request_id as u32;
        unsafe { ioctl_ptr(fd, UI_BEGIN_FF_UPLOAD, &mut up)? };
        match accept(&up.effect) {
            Ok(id) => {
                up.effect.id = id;
                up.retval = 0;
            }
            Err(code) => up.retval = code,
        }
        unsafe { ioctl_ptr(fd, UI_END_FF_UPLOAD, &mut up) }
    }

    /// Completes an `UI_FF_ERASE` request.
    pub fn handle_erase(&self, request_id: i32, erase: impl FnOnce(u32)) -> io::Result<()> {
        let fd = self.file.as_raw_fd();
        let mut er: uinput_ff_erase = unsafe { std::mem::zeroed() };
        er.request_id = request_id as u32;
        unsafe { ioctl_ptr(fd, UI_BEGIN_FF_ERASE, &mut er)? };
        erase(er.effect_id);
        er.retval = 0;
        unsafe { ioctl_ptr(fd, UI_END_FF_ERASE, &mut er) }
    }
}

impl Drop for VirtualDevice {
    fn drop(&mut self) {
        unsafe {
            let _ = libc::ioctl(self.file.as_raw_fd(), UI_DEV_DESTROY as _, 0);
        }
    }
}

/// Holds an exclusive grab on a real evdev node so games only see the virtual
/// wheel. Released on drop.
pub struct Grab {
    _file: File,
}

impl Grab {
    pub fn take(path: &str) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(path)?;
        unsafe { ioctl_int(file.as_raw_fd(), EVIOCGRAB, 1)? };
        Ok(Grab { _file: file })
    }
}

// --- reading an ff_effect's union --------------------------------------------------

/// The parts of `ff_effect.u` we act on, decoded from the raw union bytes.
#[derive(Debug, Clone, Copy, Default)]
pub struct Envelope {
    pub attack_length: u16,
    pub attack_level: u16,
    pub fade_length: u16,
    pub fade_level: u16,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Condition {
    pub right_saturation: u16,
    pub left_saturation: u16,
    pub right_coeff: i16,
    pub left_coeff: i16,
    pub deadband: u16,
    pub center: i16,
}

#[derive(Debug, Clone, Copy)]
pub enum EffectKind {
    Constant { level: i16, envelope: Envelope },
    Ramp { start: i16, end: i16, envelope: Envelope },
    Periodic { waveform: u16, period: u16, magnitude: i16, offset: i16, phase: u16, envelope: Envelope },
    /// Spring, damper, friction, inertia: the first condition block (the
    /// wheel has one axis).
    Condition { type_: u16, condition: Condition },
    Unsupported(u16),
}

#[derive(Debug, Clone, Copy)]
pub struct Effect {
    pub id: i16,
    pub direction: u16,
    pub replay_length: u16,
    pub replay_delay: u16,
    pub kind: EffectKind,
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn i16_at(b: &[u8], i: usize) -> i16 {
    i16::from_le_bytes([b[i], b[i + 1]])
}
fn envelope_at(b: &[u8], i: usize) -> Envelope {
    Envelope {
        attack_length: u16_at(b, i),
        attack_level: u16_at(b, i + 2),
        fade_length: u16_at(b, i + 4),
        fade_level: u16_at(b, i + 6),
    }
}

impl Effect {
    pub fn from_raw(e: &ff_effect) -> Self {
        // `u` is a [u64; 4] in libc; view it as little-endian bytes laid out
        // like the kernel union.
        let u: [u8; 32] = unsafe { std::mem::transmute(e.u) };
        let kind = match e.type_ {
            FF_CONSTANT => EffectKind::Constant { level: i16_at(&u, 0), envelope: envelope_at(&u, 2) },
            FF_RAMP => EffectKind::Ramp { start: i16_at(&u, 0), end: i16_at(&u, 2), envelope: envelope_at(&u, 4) },
            FF_PERIODIC => EffectKind::Periodic {
                waveform: u16_at(&u, 0),
                period: u16_at(&u, 2),
                magnitude: i16_at(&u, 4),
                offset: i16_at(&u, 6),
                phase: u16_at(&u, 8),
                envelope: envelope_at(&u, 10),
            },
            FF_SPRING | FF_DAMPER | FF_FRICTION | FF_INERTIA => EffectKind::Condition {
                type_: e.type_,
                condition: Condition {
                    right_saturation: u16_at(&u, 0),
                    left_saturation: u16_at(&u, 2),
                    right_coeff: i16_at(&u, 4),
                    left_coeff: i16_at(&u, 6),
                    deadband: u16_at(&u, 8),
                    center: i16_at(&u, 10),
                },
            },
            other => EffectKind::Unsupported(other),
        };
        Effect {
            id: e.id,
            direction: e.direction,
            replay_length: e.replay.length,
            replay_delay: e.replay.delay,
            kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ioctl_numbers_match_the_header() {
        // Values from <linux/uinput.h> on x86_64.
        assert_eq!(UI_DEV_CREATE, 0x5501);
        assert_eq!(UI_DEV_DESTROY, 0x5502);
        assert_eq!(UI_SET_EVBIT, 0x40045564);
        assert_eq!(UI_SET_KEYBIT, 0x40045565);
        assert_eq!(UI_SET_ABSBIT, 0x40045567);
        assert_eq!(UI_SET_FFBIT, 0x4004556b);
        assert_eq!(UI_DEV_SETUP, 0x405c5503);
        assert_eq!(UI_ABS_SETUP, 0x401c5504);
        assert_eq!(UI_BEGIN_FF_UPLOAD, 0xc06855c8);
        assert_eq!(UI_END_FF_UPLOAD, 0x406855c9);
        assert_eq!(UI_BEGIN_FF_ERASE, 0xc00c55ca);
        assert_eq!(UI_END_FF_ERASE, 0x400c55cb);
        assert_eq!(EVIOCGRAB, 0x40044590);
    }

    #[test]
    fn decodes_a_damper_with_its_condition() {
        let mut e: ff_effect = unsafe { std::mem::zeroed() };
        e.type_ = FF_DAMPER;
        e.id = 3;
        let mut u = [0u8; 32];
        u[0..2].copy_from_slice(&0xffffu16.to_le_bytes()); // right sat
        u[2..4].copy_from_slice(&0xffffu16.to_le_bytes()); // left sat
        u[4..6].copy_from_slice(&0x4000i16.to_le_bytes()); // right coeff
        u[6..8].copy_from_slice(&0x4000i16.to_le_bytes()); // left coeff
        e.u = unsafe { std::mem::transmute(u) };
        match Effect::from_raw(&e).kind {
            EffectKind::Condition { type_, condition } => {
                assert_eq!(type_, FF_DAMPER);
                assert_eq!(condition.right_coeff, 0x4000);
                assert_eq!(condition.left_saturation, 0xffff);
            }
            other => panic!("wrong kind {other:?}"),
        }
    }
}
