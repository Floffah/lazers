use crate::io::StdioHandles;

pub type TextTaskEntry = fn(&TextTask);

pub struct TextTask {
    stdio: StdioHandles,
    entry: TextTaskEntry,
}

impl TextTask {
    pub const fn new(entry: TextTaskEntry, stdio: StdioHandles) -> Self {
        Self { stdio, entry }
    }

    pub fn run_step(&self) {
        (self.entry)(self);
    }

    pub fn read_stdin_byte(&self) -> Option<u8> {
        self.stdio.stdin.read_byte()
    }

    pub fn write_stdout_byte(&self, byte: u8) -> bool {
        self.stdio.stdout.write_byte(byte)
    }

    #[allow(dead_code)]
    pub fn write_stderr_byte(&self, byte: u8) -> bool {
        self.stdio.stderr.write_byte(byte)
    }
}

pub fn echo_task_entry(task: &TextTask) {
    while let Some(byte) = task.read_stdin_byte() {
        match byte {
            b'\n' => {
                let _ = task.write_stdout_byte(b'\n');
            }
            0x7f => {
                let _ = task.write_stdout_byte(0x7f);
            }
            0x20..=0x7e => {
                let _ = task.write_stdout_byte(byte);
            }
            _ => {}
        }
    }
}
