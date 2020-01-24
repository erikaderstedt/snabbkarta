use std::fs;
use std::sync::mpsc::Receiver;
use std::mem;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::slice;
use std::convert::TryInto;
use byteorder::{LittleEndian, ReadBytesExt};
use std::path::PathBuf;

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

fn write_instances<T: Sized>(x: &Vec<T>, writer: &mut dyn Write) -> std::io::Result<usize> {
    let slice = unsafe { slice::from_raw_parts(x.as_ptr() as *const u8, mem::size_of::<T>()*x.len()) };
    writer.write(slice)
}

fn ftell(file: &mut dyn Seek) -> u32 {
    file.seek(SeekFrom::Current(0)).expect("Unable to obtain file position").try_into().expect("File size too large")
}

use super::sweref_to_wgs84::Sweref as Point;

enum PointType {
    Normal,
    FirstBezier,
    SecondBezier,
    Corner,
    HoleStart,
}

#[derive(Clone,Debug)]
pub enum Segment {
    Move(Point),
    Bezier(Point,Point,Point),
    Line(Point),
}

#[derive(Debug,PartialEq)]
pub enum ObjectType {
    Point(f64),
    Area,
    Line(bool),
    Rectangle,

    Terminate,
}

impl ObjectType {
    fn ocad_object_type(&self) -> u8 {
        match self {
            Self::Point(_) => 1,
            Self::Area => 2,
            Self::Line(_) => 3,
            Self::Rectangle => 4,
            Self::Terminate => panic!("No valid object type for Terminate request.")
        }
    }
}

#[derive(Debug)]
pub struct Object {
    pub object_type: ObjectType,
    pub symbol_number: i32,
    pub segments: Vec<Segment>,
}

impl Object {

    fn polys(&self, angle: f64, xoff: f64, yoff: f64) -> Vec<TDPoly> {

        fn convert_to_upper_24_bits(p: f64) -> i32 {
            if p < 0f64 {
                (0x1000000i32 - ((-p) as i32)) << 8
            } else {
                (p as i32) << 8
            }
        }

        let c = f64::cos(-angle.to_radians());
        let s = f64::sin(-angle.to_radians());

        let from_point = |p: &Point, t: &PointType| -> TDPoly {
            // 1 bit = 0.01 mm.
            // Map scale  1:15000 =>
            // 1 bit 0.15 m.
            let vx = (p.east - xoff) / 0.15f64;
            let vy = (p.north - yoff) / 0.15f64;
            let x = convert_to_upper_24_bits( vx * c + vy * s);
            let y = convert_to_upper_24_bits(-vx * s + vy * c);
            match t {
                PointType::Normal       => TDPoly { x: x,       y: y },
                PointType::FirstBezier  => TDPoly { x: x | 1,   y: y },
                PointType::SecondBezier => TDPoly { x: x | 2,   y: y },
                PointType::Corner       => TDPoly { x: x,       y: y | 1 },
                PointType::HoleStart    => TDPoly { x: x,       y: y | 2 },
            }
        };

        let mut polys = Vec::new();

        let cornerize: bool = match self.object_type { ObjectType::Line(c) => c, _ => false };
        for segment in self.segments.iter().enumerate() {
                
            match segment.1 {
                Segment::Move(p) if segment.0 > 0 => { polys.push(from_point(p, &PointType::HoleStart)) },
                Segment::Move(p) => { polys.push(from_point(p, &PointType::Normal)) },
                Segment::Line(p) if cornerize => { polys.push(from_point(p, &PointType::Normal)) },
                Segment::Line(p) => { polys.push(from_point(p, &PointType::Corner)) },
                Segment::Bezier(p1,p2,p3) => {
                    polys.push(from_point(p1, &PointType::FirstBezier));
                    polys.push(from_point(p2, &PointType::SecondBezier));
                    polys.push(from_point(p3, &PointType::Normal))
                },
            }    
        }
        polys            
    }
}

struct Strings {
    s: Vec<u8>,
    record_type: i32,
}

fn load_from_isom() -> (Vec<Vec<u8>>, Vec<Strings>) {
    let mut data = Cursor::new(SOFT_ISOM_2017);

    let header: RawFileHeader = read_instance(&mut data);

    let mut symbols: Vec<Vec<u8>> = Vec::new();

    let mut next_symbol_index: u64 = header.symbolindex.into();
    while next_symbol_index != 0 {
        data.seek(SeekFrom::Start(next_symbol_index)).expect("Unable to seek to start of symbol index.");
        let symbol_block: SymbolBlock = read_instance(&mut data);

        next_symbol_index = symbol_block.nextsymbolblock as u64;
        let indices = symbol_block.symbol_indices;
        for position in indices.iter().filter(|x| **x > 0) {
            let p: u64 = (*position).into();
            data.seek(SeekFrom::Start(p)).expect("Unable to seek to start of symbol."); 
            // Read a single u32
            let sz: usize = data.read_u32::<LittleEndian>().expect("Unable to read symbol size.") as usize;
            let mut v = vec![0u8;sz];
            data.seek(SeekFrom::Start(p)).expect("Unable to seek to start of symbol again."); 
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
            let sz: usize = string_index.length.try_into().expect("Unable to convert string index length to usize.");
            let mut buffer = vec![0u8;sz];
            data.seek(SeekFrom::Start(string_index.position as u64)).expect("Unable to seek to start of string.");
            data.read_exact(&mut buffer).expect("Unable to read string from OCAD file.");
            strings.push( Strings {
                s: buffer.to_vec(),
                record_type: string_index.rectype,
            });
        }
    }

    (symbols, strings.into_iter().filter(|x| match x.record_type { 9 | 10 => true, _ => false }).collect())
}

pub fn create(path: &PathBuf, southwest_corner: &Point, northeast_corner: &Point, angle: f64, queue: &Receiver<Object>) {
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

    let mut file = fs::File::create(path.to_str().expect("Unable to convert output path to str.")).expect("Unable to create output file.");
    write_instance(&header, &mut file).expect("Unable to write OCAD header.");
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
        file.seek(SeekFrom::Start(index.into())).expect("Unable to seek to start of symbol index.");
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
        file.seek(SeekFrom::Start(index.into())).expect("Unable to seek to start of string index.");
        write_instance(&string_index, &mut file).expect("Could not write final string index to OCAD file.");
        file.seek(SeekFrom::End(0)).expect("Unable to seek back to end of file.");
    }

    header.objectindex = ftell(&mut file);

    // BEGIN object index
    fn begin_object_index(file: &mut fs::File) -> ObjectIndexBlock { 
        let o = ObjectIndexBlock { nextindexblock: 0, indices: [ObjectIndex { 
            rc: LRect { lower_left: TDPoly { x: 0, y: 0 }, upper_right: TDPoly { x: 0, y: 0 } },
            position: 0, length: 0, symbol: 0, object_type: 0, encrypted_mose: 0, status: 0, viewtype: 0, color: 0, reserved1: 0, imported_layer: 0, reserved2: 0 }; 256] };
        write_instance(&o, file).expect("Unable to write object index instance.");
        o
    }

    fn end_object_index(position: u32, object_index: &mut ObjectIndexBlock, file: &mut fs::File) -> u32 {
        object_index.nextindexblock = ftell(file);
        file.seek(SeekFrom::Start(position.into())).expect("Unable to seek to object index.");
        write_instance(object_index, file).expect("Unable to write object index instance.");
        file.seek(SeekFrom::End(0)).expect("Unable to seek back to end of file.").try_into().expect("Value too large")
    }
    
    let mut start_pos = header.objectindex;
    let mut object_index = begin_object_index(&mut file);
    let mut current_index = 0;

    loop {
        let object = queue.recv().expect("Unable to receive message on OCAD thread.");
        if object.object_type == ObjectType::Terminate { break; }

        let p = object.polys(angle, x0, y0);

        let element = Element {
            symbol_number: object.symbol_number,
            object_type: object.object_type.ocad_object_type(),
            angle: match object.object_type { ObjectType::Point(a) => (a*10f64) as i16, _ => 0i16 },
            _color: 0u32,
            _line_width: 0u16,
            _diam_flags: 0u16,
            _server_object_id: 0u32,
            _height: 0i32,
            _creation_date: 0f64,
            _multirepresentationid: 0u32,
            _modification_date: 0f64,
            n_coordinates: p.len() as u32,
            _n_text: 0i16, _n_object_string: 0i16, _n_database_string: 0i16,
            _object_string_type: 0u8,
            _reserved0: 0u8, _reserved1: 0u8,
        };

        let position = ftell(&mut file) as u32;

        write_instance(&element, &mut file).expect("Unable to write OCAD element.");
        write_instances(&p, &mut file).expect("Unable to write TDPoly vector.");

        // Create Element, and fill out object index
        object_index.indices[current_index] = ObjectIndex {
            rc: LRect { 
                lower_left: TDPoly { 
                    x: (p.iter().map(|j| j.x).min().unwrap() >> 8) << 8,
                    y: (p.iter().map(|j| j.y).min().unwrap() >> 8) << 8,
                },
                upper_right: TDPoly {
                    x: (p.iter().map(|j| j.x).max().unwrap() >> 8) << 8,
                    y: (p.iter().map(|j| j.y).max().unwrap() >> 8) << 8,
                },
            },
            position: position,
            length: (mem::size_of::<ObjectIndex>() + (mem::size_of::<TDPoly>()) * p.len()) as u32,
            symbol: element.symbol_number,
            object_type: element.object_type,
            encrypted_mose: 0u8,
            status: 1u8,
            viewtype: 0u8,
            color: 0u16,
            reserved1: 0u16, reserved2: 0u16, imported_layer: 0u16,
        };

        if current_index == 255 {
            start_pos = end_object_index(start_pos, &mut object_index, &mut file);
            object_index = begin_object_index(&mut file);
            current_index = 0
        } else {
            current_index = current_index+1;
        }
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
#[derive(Debug,Copy,Clone)]
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
    _reserved0: u8,
    angle: i16,
    _color: u32,
    _line_width: u16,
    _diam_flags: u16,
    _server_object_id: u32,
    _height: i32,
    _creation_date: f64,
    _multirepresentationid: u32,
    _modification_date: f64,
    n_coordinates: u32,
    _n_text: i16,
    _n_object_string: i16,
    _n_database_string: i16,
    _object_string_type: u8,
    _reserved1: u8,
}
