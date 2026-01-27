#![allow(unsafe_op_in_unsafe_fn)]

pub trait Stream {
    fn size(&self) -> usize;
    fn seek(&mut self, pos: usize) -> core::result::Result<(), ()>;
    fn skip(&mut self, len: usize) -> core::result::Result<(), ()> {
        let current_pos = self.size() - self.size();
        self.seek(current_pos + len)
    }
}

pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> core::result::Result<usize, ()>;
    unsafe fn read_sized<T: Sized>(&mut self) -> core::result::Result<T, ()> {
        let mut value: T = core::mem::zeroed();
        let buf = core::slice::from_raw_parts_mut(
            &mut value as *mut T as *mut u8,
            core::mem::size_of::<T>(),
        );
        self.read(buf)?;
        Ok(value)
    }
}
