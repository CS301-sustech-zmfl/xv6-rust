use array_macro::array;
use crate::register::{ tp, sstatus };
use crate::define::param::NCPU;
use crate::lock::spinlock::{SpinlockGuard, Spinlock};
use core::ptr::NonNull;
use super::*;
pub struct CPU{
    pub process:Option<NonNull<Process>>, // The process running on this cpu, or null.
    pub context:Context, // swtch() here to enter scheduler().
    pub noff:usize, // Depth of push_off() nesting.
    pub intena:usize // Were interrupts enabled before push_off()?
}

pub struct CPUManager{
    cpus: [CPU; NCPU]
}

pub static mut CPU_MANAGER:CPUManager = CPUManager::new();

pub unsafe fn cpuid() ->usize{
    let id = tp::read();
    id
}

impl CPUManager{
    pub const fn new() -> Self{
        Self{
            cpus: array![_ => CPU::new(); NCPU],
        }
    }

    pub unsafe fn mycpu(&mut self) -> &mut CPU{
        let cpu_id = cpuid();
        &mut self.cpus[cpu_id]
    }

    pub unsafe fn myproc(&mut self) -> Option<&mut Process>{
        push_off();
        let c = CPU_MANAGER.mycpu();
        if let Some(proc) = c.process{
           let p = &mut *(proc.as_ptr());
           pop_off();
           return Some(p)
        }
        pop_off();
        None

    }

    pub fn yield_proc(&mut self) {
        if let Some(my_proc) = unsafe{ self.myproc() } {
            let guard = my_proc.data.acquire();
            if guard.state == Procstate::RUNNING {
                drop(guard);
                my_proc.yielding();
            }else {
                drop(guard);
            }
        }
    }
}

impl CPU{
    pub const fn new() -> Self{
        Self{
            process:None,
            context:Context::new(),
            noff:0,
            intena:0
        }
    }

    // pub fn get_proc(&self) -> &Process {
    //     self.process.unwrap().as_ref()
    // }

    pub fn set_proc(&mut self, proc:Option<NonNull<Process>>){
        self.process = proc;
    }

    pub fn get_context_mut(&mut self) -> *mut Context{
        &mut self.context as *mut Context
    }


    // Switch to scheduler.  Must hold only p->lock
    // and have changed proc->state. Saves and restores
    // intena because intena is a property of this
    // kernel thread, not this CPU. It should
    // be proc->intena and proc->noff, but that would
    // break in the few places where a lock is held but
    // there's no process.

    pub unsafe fn sched<'a>
    (
        &mut self, 
        guard: SpinlockGuard<'a, ProcData>, 
        ctx: *mut Context
    ) 
    -> SpinlockGuard<'a, ProcData>
    {
        extern "C" {
            fn swtch(old: *mut Context, new: *mut Context);
        }

        if !guard.holding() {
            panic!("sched: not holding proc's lock");
        }

        if self.noff != 1 {
            panic!("sched: cpu hold mutliple locks");
        }

        if guard.state == Procstate::RUNNING {
            panic!("sched: proc is running");
        }

        if sstatus::intr_get() {
            panic!("sched: interruptible");
        }

        let intena = self.intena;
        swtch(ctx, &mut self.context as *mut Context);
        self.intena = intena;

        guard
        
    }

}

// push_off/pop_off are like intr_off()/intr_on() except that they are matched:
// it takes two pop_off()s to undo two push_off()s.  Also, if interrupts
// are initially off, then push_off, pop_off leaves them off.

pub fn push_off(){
    let old_enable;
    unsafe{
        old_enable = sstatus::intr_get();
        sstatus::intr_off();
    }
    let my_cpu = unsafe{ CPU_MANAGER.mycpu() };
    if my_cpu.noff == 0 {
        my_cpu.intena = old_enable as usize;
    }


    my_cpu.noff += 1;
}


pub fn pop_off() {
    if unsafe{ sstatus::intr_get() } {
        panic!("pop_off(): interruptable");
    }
    let c = unsafe { CPU_MANAGER.mycpu() };
    if c.noff.checked_sub(1).is_none() {
        panic!("pop_off(): count not match");
    }
    c.noff -= 1;
    if c.noff == 0 && c.intena != 0 {
        unsafe{ sstatus::intr_on() };
    }

}
