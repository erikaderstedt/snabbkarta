use std::slice;
use std::mem;
use std::io::{Read, Seek, SeekFrom, Write};
use std::convert::TryInto;

pub fn read_instance<T: Sized>(reader: &mut dyn Read) -> std::io::Result<T> {
    let mut x: T = unsafe { mem::zeroed() };
    let sz = mem::size_of::<T>();
    let slice = unsafe { slice::from_raw_parts_mut(&mut x as *mut _ as *mut u8, sz) };
    reader.read_exact(slice)?;
    Ok(x)
}

pub fn read_instances<T: Sized>(reader: &mut dyn Read, count: usize) -> std::io::Result<Vec<T>> {
    let mut v: Vec<T> = Vec::with_capacity(count);
    let sz = mem::size_of::<T>();
    unsafe { 
        let s = slice::from_raw_parts_mut(v.as_mut_ptr() as *mut u8, sz * count);
        reader.read_exact(s)?;
        v.set_len(count);  
    };    
    Ok(v)
}

pub fn write_instance<T: Sized>(x: &T, writer: &mut dyn Write) -> std::io::Result<usize> {
    let slice = unsafe { slice::from_raw_parts((x as *const T) as *const u8, mem::size_of::<T>()) };
    writer.write(slice)
}

pub fn write_instances<T: Sized>(x: &Vec<T>, writer: &mut dyn Write) -> std::io::Result<usize> {
    let slice = unsafe { slice::from_raw_parts(x.as_ptr() as *const u8, mem::size_of::<T>()*x.len()) };
    writer.write(slice)
}

pub fn ftell(file: &mut dyn Seek) -> u32 {
    file.seek(SeekFrom::Current(0)).expect("Unable to obtain file position").try_into().expect("File size too large")
}

pub fn fseek(file: &mut dyn Seek, pos: u32) {
    file.seek(SeekFrom::Start(pos as u64)).expect("Unable to obtain file position");
}
