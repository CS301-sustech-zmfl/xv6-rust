use super::raw_vec::RawVec;
use core::ptr::{write, read};

pub struct Vec<T>{
    buf: RawVec<T>,
    len: usize
}

impl<T> Vec<T>{
    #[inline]
    pub const fn new() -> Self{
        Vec{
            buf: RawVec::NEW,
            len: 0
        }
    }
    #[inline]
    pub fn capacity(&self) -> usize{
        self.buf.capacity()
    }

    #[inline]
    pub fn as_ptr(&self) -> *const T{
        self.buf.as_ptr()
    }

    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T{
        self.buf.as_ptr()
    }

    pub fn reserve(&mut self, additional:usize){
        self.buf.reserve(self.len, additional);
    }

    #[inline]
    pub fn push(&mut self, value: T){
        if self.len == self.buf.capacity() {
            self.reserve(1);
        }

        unsafe {
            let end = self.as_mut_ptr().add(self.len);
            write(end, value);
            self.len += 1;
        }
    }

    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0{
            return None
        }
        self.len -= 1;
        unsafe{
            let end  = self.as_mut_ptr().add(self.len);
            let ret = read(end as *const T);
            Some(ret)
        }
    }
}

