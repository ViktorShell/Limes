use std::alloc::{alloc_zeroed, Layout};
use std::slice;
use std::str;

fn main() {}

// NOTE: Secure Memory Allocator
#[no_mangle]
pub extern "C" fn wasm_alloc(size: usize) -> *mut u8 {
    let layout = if let Ok(val) = Layout::array::<u8>(size) {
        val
    } else {
        return std::ptr::null_mut();
    };

    let ptr_memory: *mut u8 = unsafe { alloc_zeroed(layout) };
    if ptr_memory.is_null() {
        return std::ptr::null_mut();
    }
    ptr_memory
}

// NOTE: Lambra function wrapper
#[no_mangle]
pub extern "C" fn wrapper(ptr: *const u8, len: i32) -> i32 {
    // Check pointer validity
    if ptr.is_null() {
        return 0;
    }

    // Convert to slice from raw pointer
    let params = unsafe { slice::from_raw_parts(ptr, len as usize) };

    // Convert to UTF-8 or return error
    let params_str = match str::from_utf8(params) {
        Ok(v) => v,
        Err(_) => return 0,
    };

    // Exec run
    let mut result = run(params_str);
    if !result.contains('\0') {
        // C compliant string
        result.push('\0');
    }

    let result_bytes = result.as_bytes();
    let result_ptr = result_bytes.as_ptr();

    // Prevent call of Drop trait on result
    std::mem::forget(result);

    // Return pointer
    result_ptr as i32
}

// NOTE: Example, append custom function
#[no_mangle]
fn run(json_data: &str) -> String {
    let mut local = String::from(json_data);
    let numbers: Vec<i32> = local
        .split(',')
        .filter_map(|val| val.parse().ok())
        .collect();
    let sum: i32 = numbers.iter().sum();
    sum.to_string()
}
