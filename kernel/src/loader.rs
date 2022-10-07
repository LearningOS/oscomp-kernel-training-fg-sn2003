use alloc::vec::Vec;
use spin::lazy::Lazy;

/* note: temp, 只用来加载initproc */
pub fn get_num_app() -> usize {
    extern "C" {
        fn _num_app();
    }
    unsafe { (_num_app as usize as *const usize).read_volatile() }
}

pub fn get_app_data(app_id: usize) -> &'static [u8] {
    extern "C" {
        fn _num_app();
    }
    let num_app_ptr = _num_app as usize as *const usize;
    let num_app = unsafe { num_app_ptr.read_volatile() };
    let app_start = unsafe { core::slice::from_raw_parts( 
        num_app_ptr.add(1), num_app + 1)};
    assert!(app_id < num_app);
    unsafe {
        core::slice::from_raw_parts(
            app_start[app_id] as *const u8,
            app_start[app_id + 1] - app_start[app_id],
        )
    }
}

pub fn get_app_data_by_name(path: &str) -> Option<&'static [u8]> {
    let num_app = get_num_app();
    (0..num_app)
        .find(|&i| APP_NAME[i] == path)
        .map(get_app_data)
}

pub static APP_NAME: Lazy<Vec<&'static str>> = Lazy::new(||{
    extern "C" {
        fn _app_names();
    }
    let num_app = get_num_app();
    let mut v: Vec<&'static str> = Vec::new();
    let mut start = _app_names as usize as *const u8;
    unsafe {
        for _ in 0..num_app {
            let mut end =start;
            while end.read_volatile() != b'\0' {
                end = end.add(1)
            }
            let str = core::slice::from_raw_parts(
                start,
                end as usize - start as usize,
            );
            let str = core::str::from_utf8(str).unwrap();
            v.push(str);
            start = end.add(1);
        }
    }
    v
});
