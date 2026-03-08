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
    r#"
    .section .text.load_gdt_tss,"ax"
    .global load_gdt_tss
load_gdt_tss:
    lgdt [rdi]
    push {kcode}
    lea rax, [rip + 1f]
    push rax
    retfq
1:
    mov ax, {kdata}
    mov ds, ax
    mov es, ax
    mov ss, ax
    xor eax, eax
    mov fs, ax
    mov gs, ax
    mov ax, {tss}
    ltr ax
    ret

    .section .text.trap_invalid_opcode,"ax"
    .global trap_invalid_opcode
trap_invalid_opcode:
    push 0
    push {vector_invalid}
    jmp trap_common

    .section .text.trap_general_protection,"ax"
    .global trap_general_protection
trap_general_protection:
    push {vector_gp}
    jmp trap_common

    .section .text.trap_page_fault,"ax"
    .global trap_page_fault
trap_page_fault:
    push {vector_pf}
    jmp trap_common

    .section .text.trap_syscall,"ax"
    .global trap_syscall
trap_syscall:
    push 0
    push {vector_syscall}
    jmp trap_common

    .section .text.trap_common,"ax"
    .global trap_common
trap_common:
    cld
    push r15
    push r14
    push r13
    push r12
    push r11
    push r10
    push r9
    push r8
    push rsi
    push rdi
    push rbp
    push rdx
    push rcx
    push rbx
    push rax
    mov rdi, rsp
    call rust_trap_entry
    pop rax
    pop rbx
    pop rcx
    pop rdx
    pop rbp
    pop rdi
    pop rsi
    pop r8
    pop r9
    pop r10
    pop r11
    pop r12
    pop r13
    pop r14
    pop r15
    add rsp, 16
    iretq
"#,
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

pub fn activate_address_space(space: AddressSpace, kernel_stack_top: u64) {
    set_kernel_stack_top(kernel_stack_top);
    load_page_table(space.root_paddr());
}

pub fn load_page_table(root_paddr: u64) {
    unsafe {
        asm!("mov cr3, {}", in(reg) root_paddr, options(nostack, preserves_flags));
    }
}

pub fn set_kernel_stack_top(stack_top: u64) {
    with_arch_mut(|arch| {
        arch.tss.rsp0 = stack_top;
    });
}

pub fn enter_user_mode(entry_point: u64, user_stack_top: u64) -> ! {
    unsafe {
        asm!(
            "mov ax, {user_data}",
            "mov ds, ax",
            "mov es, ax",
            "push {user_data}",
            "push rsi",
            "push 0x202",
            "push {user_code}",
            "push rdi",
            "iretq",
            user_data = const ((USER_DATA_SELECTOR | 0x3) as u64),
            user_code = const ((USER_CODE_SELECTOR | 0x3) as u64),
            in("rdi") entry_point,
            in("rsi") user_stack_top,
            options(noreturn)
        )
    }
}

pub fn read_cr2() -> u64 {
    let value: u64;
    unsafe {
        asm!("mov {}, cr2", out(reg) value, options(nostack, preserves_flags));
    }
    value
}

#[no_mangle]
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
                crate::scheduler::block_current_thread_and_schedule();
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
        asm!("lidt [{}]", in(reg) &pointer, options(readonly, nostack, preserves_flags));
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
    (0xffff)
        | (access << 40)
        | (flags << 52)
        | (0xf << 48)
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
