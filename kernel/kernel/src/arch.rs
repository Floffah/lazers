//! x86_64 architecture support for segmentation, traps, page-table activation,
//! and user-mode entry.
//!
//! This module owns the low-level CPU contracts that the higher-level runtime
//! relies on: switching CR3, loading the GDT/TSS/IDT, entering ring 3, and
//! reflecting traps back into Rust code.

use core::arch::{asm, global_asm};
use core::cell::UnsafeCell;

use crate::memory::AddressSpace;

const KERNEL_CODE_SELECTOR: u16 = 0x08;
const KERNEL_DATA_SELECTOR: u16 = 0x10;
const USER_DATA_SELECTOR: u16 = 0x18;
const USER_CODE_SELECTOR: u16 = 0x20;
const TSS_SELECTOR: u16 = 0x28;

const TRAP_INVALID_OPCODE: u8 = 6;
const TRAP_GENERAL_PROTECTION: u8 = 13;
const TRAP_PAGE_FAULT: u8 = 14;
pub const TRAP_SYSCALL: u8 = 0x80;

global_asm!(
    include_str!("arch.asm"),
    kcode = const KERNEL_CODE_SELECTOR,
    kdata = const KERNEL_DATA_SELECTOR,
    tss = const TSS_SELECTOR,
    vector_invalid = const TRAP_INVALID_OPCODE,
    vector_gp = const TRAP_GENERAL_PROTECTION,
    vector_pf = const TRAP_PAGE_FAULT,
    vector_syscall = const TRAP_SYSCALL,
);

unsafe extern "C" {
    fn load_gdt_tss(pointer: *const DescriptorTablePointer);
    fn trap_invalid_opcode();
    fn trap_general_protection();
    fn trap_page_fault();
    fn trap_syscall();
}

static ARCH: ArchCell = ArchCell::new();

pub fn init() {
    with_arch_mut(|arch| {
        arch.setup_gdt();
        arch.setup_idt();
    });

    let gdt_pointer = with_arch(|arch| arch.gdt_pointer());
    unsafe {
        load_gdt_tss(&gdt_pointer as *const DescriptorTablePointer);
    }
    load_idt();
}

/// Activates the target address space and kernel stack for the next thread.
///
/// The scheduler calls this before every context switch so the CPU will use the
/// correct CR3 and ring-0 stack when the next thread traps back into the
/// kernel.
pub fn activate_address_space(space: AddressSpace, kernel_stack_top: u64) {
    set_kernel_stack_top(kernel_stack_top);
    load_page_table(space.root_paddr());
}

/// Loads a new page-table root into CR3.
pub fn load_page_table(root_paddr: u64) {
    unsafe {
        asm!(
            include_str!("load_page_table.arch.asm"),
            in(reg) root_paddr,
            options(nostack, preserves_flags)
        );
    }
}

/// Updates the ring-0 stack pointer stored in the TSS.
pub fn set_kernel_stack_top(stack_top: u64) {
    with_arch_mut(|arch| {
        arch.tss.rsp0 = stack_top;
    });
}

/// Performs the first transition of a thread from kernel mode into ring 3.
///
/// The scheduler prepares the user entry point and stack top ahead of time; this
/// function builds the minimal `iretq` frame needed to begin executing that
/// user thread.
pub fn enter_user_mode(entry_point: u64, user_stack_top: u64) -> ! {
    unsafe {
        asm!(
            include_str!("enter_user_mode.arch.asm"),
            user_data = const ((USER_DATA_SELECTOR | 0x3) as u64),
            user_code = const ((USER_CODE_SELECTOR | 0x3) as u64),
            in("rdi") entry_point,
            in("rsi") user_stack_top,
            options(noreturn)
        )
    }
}

/// Reads CR2, primarily for page-fault diagnostics.
pub fn read_cr2() -> u64 {
    let value: u64;
    unsafe {
        asm!(
            include_str!("read_cr2.arch.asm"),
            out(reg) value,
            options(nostack, preserves_flags)
        );
    }
    value
}

#[no_mangle]
/// Handles all trap entrypoints after the assembly prologue has saved the
/// register frame.
pub extern "C" fn rust_trap_entry(frame: &mut TrapFrame) {
    match frame.vector as u8 {
        TRAP_SYSCALL => crate::syscall::dispatch(frame),
        TRAP_INVALID_OPCODE | TRAP_GENERAL_PROTECTION | TRAP_PAGE_FAULT => {
            if frame.from_user_mode() {
                kprintln!(
                    "user trap {} rip={:#x} err={:#x} cr2={:#x}",
                    frame.vector,
                    frame.rip,
                    frame.error_code,
                    read_cr2()
                );
                crate::scheduler::exit_current_process(1);
            } else {
                panic!(
                    "kernel trap {} rip={:#x} err={:#x} cr2={:#x}",
                    frame.vector,
                    frame.rip,
                    frame.error_code,
                    read_cr2()
                );
            }
        }
        _ => panic!("unexpected trap {}", frame.vector),
    }
}

fn load_idt() {
    let pointer = with_arch(|arch| arch.idt_pointer());
    unsafe {
        asm!(
            include_str!("load_idt.arch.asm"),
            in(reg) &pointer,
            options(readonly, nostack, preserves_flags)
        );
    }
}

fn with_arch<F, T>(operation: F) -> T
where
    F: FnOnce(&ArchState) -> T,
{
    unsafe { operation(ARCH.get()) }
}

fn with_arch_mut<F, T>(operation: F) -> T
where
    F: FnOnce(&mut ArchState) -> T,
{
    unsafe { operation(ARCH.get()) }
}

struct ArchCell {
    state: UnsafeCell<ArchState>,
}

impl ArchCell {
    const fn new() -> Self {
        Self {
            state: UnsafeCell::new(ArchState::new()),
        }
    }

    unsafe fn get(&self) -> &mut ArchState {
        &mut *self.state.get()
    }
}

unsafe impl Sync for ArchCell {}

struct ArchState {
    gdt: GlobalDescriptorTable,
    tss: TaskStateSegment,
    idt: InterruptDescriptorTable,
}

impl ArchState {
    const fn new() -> Self {
        Self {
            gdt: GlobalDescriptorTable::new(),
            tss: TaskStateSegment::new(),
            idt: InterruptDescriptorTable::new(),
        }
    }

    fn setup_gdt(&mut self) {
        self.gdt.entries[0] = 0;
        self.gdt.entries[1] = code_descriptor(0);
        self.gdt.entries[2] = data_descriptor(0);
        self.gdt.entries[3] = data_descriptor(3);
        self.gdt.entries[4] = code_descriptor(3);

        let tss_base = &self.tss as *const TaskStateSegment as u64;
        let tss_limit = (core::mem::size_of::<TaskStateSegment>() - 1) as u32;
        let (low, high) = tss_descriptor(tss_base, tss_limit);
        self.gdt.entries[5] = low;
        self.gdt.entries[6] = high;
    }

    fn setup_idt(&mut self) {
        self.idt.set_handler(
            TRAP_INVALID_OPCODE as usize,
            trap_invalid_opcode as *const () as usize as u64,
            KERNEL_CODE_SELECTOR,
            0,
        );
        self.idt.set_handler(
            TRAP_GENERAL_PROTECTION as usize,
            trap_general_protection as *const () as usize as u64,
            KERNEL_CODE_SELECTOR,
            0,
        );
        self.idt.set_handler(
            TRAP_PAGE_FAULT as usize,
            trap_page_fault as *const () as usize as u64,
            KERNEL_CODE_SELECTOR,
            0,
        );
        self.idt.set_handler(
            TRAP_SYSCALL as usize,
            trap_syscall as *const () as usize as u64,
            KERNEL_CODE_SELECTOR,
            3,
        );
    }

    fn gdt_pointer(&self) -> DescriptorTablePointer {
        DescriptorTablePointer {
            limit: (core::mem::size_of::<GlobalDescriptorTable>() - 1) as u16,
            base: &self.gdt as *const GlobalDescriptorTable as u64,
        }
    }

    fn idt_pointer(&self) -> DescriptorTablePointer {
        DescriptorTablePointer {
            limit: (core::mem::size_of::<InterruptDescriptorTable>() - 1) as u16,
            base: &self.idt as *const InterruptDescriptorTable as u64,
        }
    }
}

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

#[repr(C, align(16))]
struct GlobalDescriptorTable {
    entries: [u64; 8],
}

impl GlobalDescriptorTable {
    const fn new() -> Self {
        Self { entries: [0; 8] }
    }
}

#[repr(C, packed)]
struct TaskStateSegment {
    reserved0: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reserved1: u64,
    ist: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    io_map_base: u16,
}

impl TaskStateSegment {
    const fn new() -> Self {
        Self {
            reserved0: 0,
            rsp0: 0,
            rsp1: 0,
            rsp2: 0,
            reserved1: 0,
            ist: [0; 7],
            reserved2: 0,
            reserved3: 0,
            io_map_base: core::mem::size_of::<Self>() as u16,
        }
    }
}

#[repr(C)]
struct InterruptDescriptorTable {
    entries: [InterruptGate; 256],
}

impl InterruptDescriptorTable {
    const fn new() -> Self {
        Self {
            entries: [InterruptGate::missing(); 256],
        }
    }

    fn set_handler(&mut self, index: usize, handler: u64, selector: u16, dpl: u8) {
        self.entries[index] = InterruptGate::new(handler, selector, dpl);
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct InterruptGate {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attributes: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl InterruptGate {
    const fn missing() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            ist: 0,
            type_attributes: 0,
            offset_mid: 0,
            offset_high: 0,
            reserved: 0,
        }
    }

    fn new(handler: u64, selector: u16, dpl: u8) -> Self {
        Self {
            offset_low: handler as u16,
            selector,
            ist: 0,
            type_attributes: 0x8e | ((dpl & 0x3) << 5),
            offset_mid: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }
}

#[repr(C)]
pub struct TrapFrame {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub vector: u64,
    pub error_code: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

impl TrapFrame {
    pub fn from_user_mode(&self) -> bool {
        (self.cs & 0x3) == 0x3
    }
}

const fn code_descriptor(dpl: u8) -> u64 {
    let access = 0x9a | (((dpl as u64) & 0x3) << 5);
    let flags = 0x2;
    (0xffff) | (access << 40) | (flags << 52) | (0xf << 48)
}

const fn data_descriptor(dpl: u8) -> u64 {
    let access = 0x92 | (((dpl as u64) & 0x3) << 5);
    (0xffff) | (access << 40) | (0xf << 48)
}

const fn tss_descriptor(base: u64, limit: u32) -> (u64, u64) {
    let low = (limit as u64 & 0xffff)
        | ((base & 0x00ff_ffff) << 16)
        | (0x89u64 << 40)
        | (((limit as u64 >> 16) & 0xf) << 48)
        | (((base >> 24) & 0xff) << 56);
    let high = base >> 32;
    (low, high)
}
