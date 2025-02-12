use core::ptr::*;
use core::cell::UnsafeCell;
use crate::lock::spinlock::{ Spinlock, SpinlockGuard };
use crate::memory::{
    kalloc::*,
    address::{ PhysicalAddress, VirtualAddress, Addr },
    mapping::{ page_table::PageTable, page_table_entry::PteFlags},
    RawPage
};
use crate::define::memlayout::{ PGSIZE, TRAMPOLINE, TRAPFRAME };
use super::*;

use alloc::boxed::Box;

#[derive(PartialEq, Copy, Clone)]
pub enum Procstate{
    UNUSED,
    USED,
    SLEEPING,
    RUNNABLE,
    RUNNING,
    ZOMBIE,
    ALLOCATED
}


pub struct Process {
    pub data: Spinlock<ProcData>,
    pub extern_data: UnsafeCell<ProcExtern>,
}

pub struct ProcData {
    // p->lock must be held when using these
    pub state:Procstate,
    pub channel:usize, // If non-zero, sleeping on chan
    pub killed:usize, // If non-zero, have been killed
    pub xstate:usize, // Exit status to be returned to parent's wait
    pub pid: usize,   // Process ID


}

impl ProcData {
    pub const fn new() -> Self {
        Self {
            state: Procstate::UNUSED,
            channel: 0,
            killed: 0,
            xstate: 0,
            pid: 0,

        }
    }

    pub fn set_state(&mut self, state: Procstate) {
        self.state = state;
    }



}

pub struct ProcExtern {
    // these are private to the process, so p->lock need to be held
    pub kstack:usize,  // Virtual address of kernel stack
    pub size:usize, // size of process memory
    pub pagetable: Option<Box<PageTable>>, // User page table
    pub trapframe: *mut Trapframe, // data page for trampoline.S
    pub context: Context, // swtch() here to run processs

    pub name: &'static str,   // Process name (debugging)

    // proc_tree_lock must be held when using this:
    pub parent: Option<NonNull<Process>>,
    
    // TODO: Open files and Current directory
}

impl ProcExtern {
    pub const fn new() -> Self {
        Self {
            kstack:0,
            size: 0,
            pagetable: None,
            trapframe: null_mut(),
            context: Context::new(),
            name: "process",
            parent: None,
        }
    }

    pub fn set_name(&mut self, name:&'static str) {
        self.name = name;
    }

    pub fn set_parent(&mut self, parent: Option<NonNull<Process>>) {
        self.parent = parent;
    }

    pub fn set_kstack(&mut self, ksatck: usize) {
        self.kstack = ksatck;
    }

    pub fn set_trapframe(&mut self, trapframe: *mut Trapframe) {
        self.trapframe = trapframe;
    }

    pub fn set_pagetable(&mut self, pagetable: Option<Box<PageTable>>) {
        self.pagetable = pagetable
    }

    pub fn set_context(&mut self, ctx: Context) {
        self.context = ctx
    }

    pub fn get_context_mut(&mut self) -> *mut Context {
        &mut self.context as *mut Context
    }

    pub fn init_context(&mut self) {

        let kstack = self.kstack;
        self.context.write_zero();
        self.context.write_ra(fork_ret as usize);
        self.context.write_sp(kstack + PGSIZE);
    }

    // Create a user page table for a given process,
    // with no user memory, but with trampoline pages
    pub unsafe fn proc_pagetable(&mut self) {

        extern "C" {
            fn trampoline();
        }

        // An empty page table
        let mut page_table = PageTable::uvmcreate();
        // map the trampoline code (for system call return )
        // at the highest user virtual address.
        // only the supervisor uses it, on the way
        // to/from user space, so not PTE_U. 
        // let page_table = &mut *page_table;
        let is_ok = page_table.mappages(
            VirtualAddress::new(TRAMPOLINE),
            PhysicalAddress::new(trampoline as usize),
            PGSIZE,
            PteFlags::R | PteFlags::X
        );

        if !is_ok {
            page_table.uvmfree(0);
        }

        // map the trapframe just below TRAMPOLINE, for trampoline.S 
        let is_ok = page_table.mappages(
            VirtualAddress::new(TRAPFRAME), 
            PhysicalAddress::new(self.trapframe as usize),
            PGSIZE,
            PteFlags::R | PteFlags::X
        );

        if !is_ok {
            page_table.uvmfree(0);
        }

        self.pagetable = Some(page_table);
    }
}



impl Process{
    pub const fn new() -> Self{
        Self{    
            data: Spinlock::new(ProcData::new(), "process"),
            extern_data: UnsafeCell::new(ProcExtern::new()),
        }
    }


    pub fn as_ptr(&self) -> *const Process{
        self as *const Process
    }

    pub fn as_mut_ptr(&mut self) -> *mut Process{
        self as *mut Process
    }

    pub fn as_ptr_addr(&self) -> usize{
        self as *const Process as usize
    }

    pub fn as_mut_ptr_addr(&mut self) -> usize{
        self as *mut Process as usize
    }





    // free a proc structure and the data hanging from it,
    // including user pages.
    // p.acquire() must be held.

    pub fn freeproc(&mut self) {
        let mut extern_data = self.extern_data.get_mut();
        if !extern_data.trapframe.is_null() {
            // kfree(PhysicalAddress::new(extern_data.trapframe as usize));
            drop(extern_data.trapframe as *mut RawPage);
            extern_data.set_trapframe(0 as *mut Trapframe);

            if let Some(page_table) = extern_data.pagetable.as_mut() {
                page_table.proc_freepagetable(extern_data.size);
            }


            let mut guard = self.data.acquire();

            extern_data.set_pagetable(None);
            extern_data.set_parent(None);
            extern_data.size = 0;

            guard.pid = 0;
            guard.channel = 0;
            guard.killed = 0;
            guard.xstate = 0;
            guard.set_state(Procstate::UNUSED);

            drop(guard);
            
        }
    }

    
    // Grow or shrink user memory by n bytes. 
    // Return true on success, false on failure. 
    pub fn growproc(&mut self, n: isize) -> Result<(), &'static str> {
        let mut extern_data = self.extern_data.get_mut();
        let mut size = extern_data.size; 
        let page_table = extern_data.pagetable.as_mut().unwrap();
        if n > 0 {
            match unsafe { page_table.uvmalloc(size, size + n as usize) } {
                Some(new_size) => {
                    size = new_size;
                },

                None => {
                    return Err("Fail to allocate virtual memory for user")
                }
            }
        }else if n < 0 {
            let new_size = (size as isize + n) as usize;
            size = page_table.uvmdealloc(size, new_size);
        }

        extern_data.size = size;

        Ok(())
    }


    // Give up the CPU for one scheduling round.
    // yield is a keyword in rust
    pub fn yielding(&mut self) {
        let mut guard = self.data.acquire();
        let ctx = self.extern_data.get_mut().get_context_mut();
        guard.set_state(Procstate::RUNNABLE);

        unsafe {
            let my_cpu = CPU_MANAGER.mycpu();
            guard = my_cpu.sched(
                guard,
                ctx
            );
        }
        drop(guard)
    }

    // Atomically release lock and sleep on chan
    // Reacquires lock when awakened.
    pub fn sleep<T>(&self, channel: usize, lock: SpinlockGuard<T>) {
        // Must acquire p->lock in order to 
        // change p->state and then call sched.
        // Once we hold p->lock, we can be
        // guaranteed that we won't miss any wakeup
        // (wakeup locks p->lock)
        // so it's okay to release lk;
        let mut guard = self.data.acquire();
        // let extern_data = self.extern_data.get_mut();
        drop(lock);

        // Go to sleep.
        guard.channel = channel;
        guard.set_state(Procstate::SLEEPING);

        unsafe {
            let my_cpu = CPU_MANAGER.mycpu();
            let ctx = (&mut (*self.extern_data.get())).get_context_mut();
            
            // get schedule process
            guard = my_cpu.sched(
                guard, 
                ctx
            );

            // Tide up
            guard.channel = 0;
            drop(guard);
        }
        

    }
}

extern "C" {
    fn trampoline();
}





