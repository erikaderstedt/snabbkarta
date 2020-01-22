use std::fs;
use std::sync::mpsc::Receiver;
use std::mem;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::slice;
use std::convert::TryInto;
use byteorder::{LittleEndian, ReadBytesExt};

static SOFT_ISOM_2017: &'static [u8] = include_bytes!("../20170608_symboluppsattning_isom_2017_ocad_12.ocd");

fn read_instance<T: Sized>(reader: &mut dyn Read) -> T {
    let mut x: T = unsafe { mem::zeroed() };
    let sz = mem::size_of::<T>();
    let slice = unsafe { slice::from_raw_parts_mut(&mut x as *mut _ as *mut u8, sz) };
    reader.read_exact(slice).expect("Unable to read instance from OCAD file");
    x
}

fn write_instance<T: Sized>(x: &T, writer: &mut dyn Write) -> std::io::Result<usize> {
    let slice = unsafe { slice::from_raw_parts((x as *const T) as *const u8, mem::size_of::<T>()) };
    writer.write(slice)
}

fn ftell(file: &mut dyn Seek) -> u32 {
    file.seek(SeekFrom::Current(0)).expect("Unable to obtain file position").try_into().expect("File size too large")
}

use super::sweref_to_wgs84::Sweref as Point;

#[derive(Clone)]
pub enum Segment {
    Move(Point),
    Bezier(Point,Point,Point),
    Line(Point),
}

impl Segment {
    
    fn ends(&self) -> &Point {
        match self {
            Segment::Move(p) => &p,
            Segment::Bezier(_,_,p) => &p,
            Segment::Line(p) => &p,
        }
    }
}

pub enum ObjectType {
    Point,
    Area,
    Line,
    Rectangle,
}

pub struct Object {
    pub object_type: ObjectType,
    pub symbol_number: u32,
    pub segments: Vec<Segment>,
}

struct Strings {
    s: Vec<u8>,
    record_type: i32,
}
 
// impl Object {

//     pub fn clip_to_bounds(&mut self, sw: &Point, ne: &Point) {
//         if self.segments.len() == 0 { return }

//         let is_outside = |p: &Point| p.east < sw.east || p.east > ne.east || p.north < sw.north || p.north > ne.north;
//         let mut new_segments = Vec::new();
//         let previous_was_outside: Option<bool> = None;
//         let current_point: Option<Point> = None;

//         for (i, segment) in self.segments.iter().enumerate() {
//             let outside = is_outside(segment.ends());
//             let mut first = true;
//             match previous_was_outside {
//                 Some(true) if outside => {
//                     current_point = Some(segment.ends().clone());
//                 },
//                 Some(true) if !outside => {
//                     // We wee outside, but are moving into the area. 
//                     match segment {
//                         Segment::Move(p) => { new_segments.push(segment); },
//                         Segment::Line(p) => { // Move to edge, then line
//                             // Use line intersection crate?
//                             // Also "flo_curves" for fit_curve_cubic. och solve_curve_for_t.
//                     }
//                 },
//                 Some(false) if !outside {

//                 },
//             }
//                 true if previous_was_outside.is_none() => continue, // First segment, and outside
//                 false if previous_was_outside.is_none() => 

//             }
//             if this != previous {

//             } else 
//         }
//     }

//     fn polys(&self, sw: &Point, ne: &Point) -> Vec<TDPoly> {

//         let mut p = Vec::new();

        
//     }
// }

fn load_from_isom() -> (Vec<Vec<u8>>, Vec<Strings>) {
    let mut data = Cursor::new(SOFT_ISOM_2017);

    let header: RawFileHeader = read_instance(&mut data);

    let mut symbols: Vec<Vec<u8>> = Vec::new();

    let mut next_symbol_index: u64 = header.symbolindex.into();
    while next_symbol_index != 0 {
        data.seek(SeekFrom::Start(next_symbol_index)).unwrap();
        let symbol_block: SymbolBlock = read_instance(&mut data);

        next_symbol_index = symbol_block.nextsymbolblock as u64;
        let indices = symbol_block.symbol_indices;
        for position in indices.iter().filter(|x| **x > 0) {
            let p: u64 = (*position).into();
            data.seek(SeekFrom::Start(p)).unwrap(); 
            // Read a single u32
            let sz: usize = data.read_u32::<LittleEndian>().expect("Unable to read symbol size.") as usize;
            let mut v = vec![0u8;sz];
            data.seek(SeekFrom::Start(p)).unwrap(); 
            data.read_exact(&mut v).expect("Unable to read symbol.");
            symbols.push(v);
        }
    }

    let mut strings: Vec<Strings> = Vec::new();

    let mut next_string_index: u64 = header.stringindex.into();
    while next_string_index != 0 {
        data.seek(SeekFrom::Start(next_string_index)).unwrap();
        let string_block: StringIndexBlock = read_instance(&mut data);

        next_string_index = string_block.nextindexblock as u64;
        let indices = string_block.indices;
        for string_index in indices.iter().filter(|x| x.position > 0) {
            let sz: usize = string_index.length.try_into().unwrap();
            let mut buffer = vec![0u8;sz];
            data.seek(SeekFrom::Start(string_index.position as u64)).unwrap();
            data.read_exact(&mut buffer).expect("Unable to read string from OCAD file.");
            strings.push( Strings {
                s: buffer.to_vec(),
                record_type: string_index.rectype,
            });
        }
    }

    (symbols, strings.into_iter().filter(|x| match x.record_type { 9 | 10 => true, _ => false }).collect())
}

pub fn create(path: &str, southwest_corner: Point, northeast_corner: Point, angle: f64, queue: Receiver<Object>, finalize: Receiver<bool>) {
    let (mut soft_symbols, mut soft_strings) = load_from_isom();

    let x0 = (northeast_corner.east + southwest_corner.east)*0.5;
    let y0 = (northeast_corner.north + southwest_corner.north)*0.5;
    let m_string = format!("\tm15000.0000\tg0.0000\tr1\tx{:.8}\ty{:.8}\ta{:.8}", x0, y0, angle);

    soft_strings.push( Strings {
        s: m_string.as_bytes().to_vec(),
        record_type: 1039,
    });

    let mut header = RawFileHeader {
        _ocadmark: 0x0cad,
        _filetype:0, _status: 0, version: 12, _subversion: 2, _subsubversion: 3, 
        symbolindex: 0,  
        objectindex: 0,
        _reserved0: [0,2,0,0],
        stringindex: 0, 
        _filenamepos: 0,
        _filenamesize: 0, 
        _reserved1: [1200279, 0, 0],
        _mrstartblockposition: 0,
    };

    let mut file = fs::File::create(path).expect("Unable to create output file.");
    write_instance(&header, &mut file).expect("Unable to write OCAD header");
    header.symbolindex = ftell(&mut file);
    
    while soft_symbols.len() > 0 {
        let index = ftell(&mut file);
        let mut symbol_index = SymbolBlock { nextsymbolblock: 0, symbol_indices: [0u32; 256], };
        write_instance(&symbol_index, &mut file).expect("Could not write symbol index to OCAD file.");
        for (i, symbol) in soft_symbols.iter().take(256).enumerate() {
            symbol_index.symbol_indices[i] = ftell(&mut file);
            file.write(&symbol[..]).expect("Unable to write symbol to OCAD file.");
        }

        soft_symbols = soft_symbols.into_iter().skip(256).collect();
        if soft_symbols.len() > 0 {
            symbol_index.nextsymbolblock = ftell(&mut file);
        }
        file.seek(SeekFrom::Start(index.into())).unwrap();
        write_instance(&symbol_index, &mut file).expect("Could not write final symbol index to OCAD file.");
        file.seek(SeekFrom::End(0)).expect("Unable to seek back to end of file.");
    }

    header.stringindex = ftell(&mut file);
    while soft_strings.len() > 0 {
        let index = ftell(&mut file);
        let mut string_index = StringIndexBlock { nextindexblock: 0, indices: [StringIndex { position: 0, length: 0, rectype: 0, objectindex: 0 }; 256] };
        write_instance(&string_index, &mut file).expect("Unable to write string index block.");
        for (i, string_info) in soft_strings.iter().take(256).enumerate() {
            string_index.indices[i].position = ftell(&mut file);
            string_index.indices[i].length = string_info.s.len().try_into().expect("String is too long.");
            string_index.indices[i].rectype = string_info.record_type;
            file.write(&string_info.s[..]).expect("Unable to write string to file.");
        }
        soft_strings = soft_strings.into_iter().skip(256).collect();
        if soft_strings.len() > 0 {
            string_index.nextindexblock = ftell(&mut file);            
        }
        file.seek(SeekFrom::Start(index.into())).unwrap();
        write_instance(&string_index, &mut file).expect("Could not write final string index to OCAD file.");
        file.seek(SeekFrom::End(0)).expect("Unable to seek back to end of file.");
    }

    header.objectindex = ftell(&mut file);

    // BEGIN object index
    fn begin_object_index(file: &mut dyn Seek) -> ObjectIndexBlock { 
        ObjectIndexBlock { nextindexblock: 0, indices: [ObjectIndex { 
            rc: LRect { lower_left: TDPoly { x: 0, y: 0 }, upper_right: TDPoly { x: 0, y: 0 } },
            position: 0, length: 0, symbol: 0, object_type: 0, encrypted_mose: 0, status: 0, viewtype: 0, color: 0, reserved1: 0, imported_layer: 0, reserved2: 0 }; 256] }
    }

    fn end_object_index(position: u32, object_index: &mut ObjectIndexBlock, file: &mut fs::File) -> u32 {
        object_index.nextindexblock = ftell(file);
        file.seek(SeekFrom::Start(position.into())).expect("Unable to seek to object index.");
        write_instance(object_index, file).expect("Unable to write object index instance.");
        file.seek(SeekFrom::End(0)).expect("Unable to seek back to end of file.").try_into().expect("Value too large")
    }
    
    let mut start_pos = header.objectindex;
    let mut object_index = begin_object_index(&mut file);

    while finalize.try_recv().unwrap_or(false) {
        let mut current_index = 0;
        match queue.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(object) => {

                current_index = current_index + 1;
                if current_index == 256 {
                    start_pos = end_object_index(start_pos, &mut object_index, &mut file);
                    object_index = begin_object_index(&mut file);
                    current_index = 0
                }
            },
            _ => {},
        };
    }

    end_object_index(start_pos, &mut object_index, &mut file);
}

#[repr(C,packed)]
#[derive(Copy,Clone)]
struct TDPoly {
    x: i32,
    y: i32,
}

#[repr(C,packed)]
#[derive(Copy,Clone)]
struct LRect {
    lower_left: TDPoly,
    upper_right: TDPoly,
}

#[repr(C,packed)]
#[derive(Debug)]
struct RawFileHeader {
    _ocadmark: u16,
    _filetype: u8,
    _status: u8,
    version: u16,
    _subversion: u8,
    _subsubversion: u8,
    symbolindex: u32,
    objectindex: u32,
    _reserved0: [u32;4],
    stringindex: u32,
    _filenamepos: u32,
    _filenamesize: u32,
    _reserved1: [u32;3],
    _mrstartblockposition: u32,
}

#[repr(C,packed)]
struct SymbolBlock {
    nextsymbolblock: u32,
    symbol_indices: [u32;256],
}

#[repr(C,packed)]
#[derive(Copy,Clone)]
struct StringIndex {
    position: u32,
    length: u32,
    rectype: i32,
    objectindex: u32,
}

#[repr(C,packed)]
struct StringIndexBlock {
    nextindexblock: u32,
    indices: [StringIndex;256],
}

#[repr(C,packed)]
#[derive(Copy,Clone)]
struct ObjectIndex {
    rc: LRect,
    position: u32,
    length: u32,
    symbol: i32,
    object_type: u8,
    encrypted_mose: u8,
    status: u8,
    viewtype: u8,
    color: u16,
    reserved1: u16,
    imported_layer: u16,
    reserved2: u16,
}

#[repr(C,packed)]
struct ObjectIndexBlock {
    nextindexblock: u32,
    indices: [ObjectIndex;256],
}

struct Element {
    symbol_number: i32,
    object_type: u8,
    reserved0: u8,
    angle: i16,
    color: u32,
    line_width: u16,
    diam_flags: u16,
    server_object_id: u32,
    height: i32,
    creation_date: f64,
    multirepresentationid: u32,
    modification_date: f64,
    n_coordinates: u32,
    n_text: i16,
    n_object_string: i16,
    n_database_string: i16,
    object_string_type: u8,
    reserved1: u8,
}
