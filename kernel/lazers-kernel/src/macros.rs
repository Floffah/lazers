macro_rules! kprint {
    ($($arg:tt)*) => {{
        $crate::console::write_fmt(core::format_args!($($arg)*));
    }};
}

macro_rules! kprintln {
    () => {{
        kprint!("\n");
    }};
    ($($arg:tt)*) => {{
        kprint!("{}\n", core::format_args!($($arg)*));
    }};
}
