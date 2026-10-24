//! Safe scalar ABI: commands in, bounded UTF-8 bytes out. No raw pointers.
use std::cell::RefCell;
#[path = "../../examples/convoy/scene.rs"]
mod scene;

struct Bridge {
    scene: scene::Convoy,
    output: [Vec<u8>; 3],
}
impl Bridge {
    fn new() -> Self {
        Self {
            scene: scene::Convoy::new(),
            output: std::array::from_fn(|_| Vec::new()),
        }
    }
    fn refresh(&mut self) {
        self.output =
            std::array::from_fn(|field| self.scene.output(field as u32).unwrap().into_bytes());
    }
}
thread_local! { static BRIDGE: RefCell<Bridge> = RefCell::new({ let mut b = Bridge::new(); b.refresh(); b }); }
#[unsafe(no_mangle)]
pub extern "C" fn dw_version() -> u32 {
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn dw_init() -> u32 {
    BRIDGE.with(|bridge| {
        let mut b = bridge.borrow_mut();
        *b = Bridge::new();
        b.refresh();
    });
    0
}
#[unsafe(no_mangle)]
pub extern "C" fn dw_act(action: u32) -> u32 {
    BRIDGE.with(|bridge| {
        let mut b = bridge.borrow_mut();
        let result = b.scene.act(action);
        if result == 0 {
            b.refresh();
        }
        result
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn dw_time() -> u32 {
    BRIDGE.with(|b| b.borrow().scene.time())
}
#[unsafe(no_mangle)]
pub extern "C" fn dw_enabled(action: u32) -> u32 {
    BRIDGE.with(|b| u32::from(b.borrow().scene.enabled(action)))
}
#[unsafe(no_mangle)]
pub extern "C" fn dw_len(field: u32) -> u32 {
    BRIDGE.with(|b| {
        b.borrow()
            .output
            .get(field as usize)
            .map_or(u32::MAX, |v| v.len() as u32)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn dw_byte(field: u32, index: u32) -> u32 {
    BRIDGE.with(|b| {
        b.borrow()
            .output
            .get(field as usize)
            .and_then(|v| v.get(index as usize))
            .map_or(256, |byte| u32::from(*byte))
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_commands_and_fields_preserve_state() {
        dw_init();
        let before = dw_len(0);
        assert_eq!(dw_act(99), 1);
        assert_eq!(dw_act(3), 1);
        assert_eq!(dw_len(0), before);
        assert_eq!(dw_len(99), u32::MAX);
        assert_eq!(dw_byte(99, 0), 256);
        assert_eq!(dw_byte(0, u32::MAX), 256);
        assert_eq!(dw_time(), 0);
        dw_act(1);
        dw_act(6);
        dw_init();
        assert_eq!(dw_time(), 0);
        assert_eq!(dw_enabled(1), 1);
    }
}
