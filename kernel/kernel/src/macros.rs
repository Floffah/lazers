#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {{
        $crate::console::write_fmt(core::format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! kprintln {
    () => {{
        $crate::kprint!("\n");
    }};
    ($($arg:tt)*) => {{
        $crate::kprint!("{}\n", core::format_args!($($arg)*));
    }};
}
