// Supervisor Status Register, sstatus
pub enum SSTATUS {
    SPP = 1 << 8,  // Previous mode, 1=Supervisor, 0=User
    SPIE = 1 << 5, // Supervisor Previous Interrupt Enable
    UPIE = 1 << 4, // User Previous Interrupt Enable
    SIE = 1 << 1, // Supervisor Interrupt Enable
    UIE = 1 << 0  // User Interrupt Enable
}

#[inline]
pub unsafe fn read() -> usize {
    let ret:usize;
    llvm_asm!("csrr $0, sstatus":"=r"(ret):::"volatile");
    ret
}

#[inline]
pub unsafe fn write(x:usize){
    llvm_asm!("csrw sstatus, $0"::"r"(x)::"volatile");
}

#[inline]
pub unsafe fn is_from_supervisor() -> bool {
    (read() & SSTATUS::SPP as usize) != 0
}

#[inline]
pub unsafe fn is_from_user() -> bool {
    (read() & SSTATUS::SPP as usize) == 0
}

// enable device interrupts
#[inline]
pub unsafe fn intr_on(){
    write(read() | SSTATUS::SIE as usize);
}

// disable device interrupts
#[inline]
pub unsafe fn intr_off(){
    write(read() & SSTATUS::SIE as usize);
}


// are device interrupts enabled?
#[inline]
pub unsafe fn intr_get() -> bool{
    let x = read();
    return (x & SSTATUS::SIE as usize) != 0;
}

// clear SPP to 0
#[inline]
pub fn clear_spp(sstatus: usize) -> usize {
    sstatus & !(SSTATUS::SPP as usize)
}

// enable interrupts in user mode
#[inline]
pub fn user_intr_on(sstatus: usize) -> usize {
    sstatus | (SSTATUS::SPIE as usize)
}

