use std::fs;
use std::sync::mpsc::{Sender,Receiver};
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use byteorder::{LittleEndian, ReadBytesExt};
use std::path::PathBuf;
use super::ffi_helpers::*;
use std::convert::TryInto;
use std::mem;
use super::geometry;

static SOFT_ISOM_2017: &'static [u8] = include_bytes!("../20170608_symboluppsattning_isom_2017_ocad_12.ocd");

use super::sweref_to_wgs84::Sweref as Point;

enum PointType {
    Normal,
    FirstBezier,
    SecondBezier,
    Corner,
    HoleStart,
}

#[derive(Clone,Debug)]
enum Segment {
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
            Self::Area => 3,
            Self::Line(_) => 2,
            Self::Rectangle => 4,
            Self::Terminate => panic!("No valid object type for Terminate request.")
        }
    }
}

#[derive(Debug)]
pub struct Object {
    pub object_type: ObjectType,
    pub symbol_number: i32,
    segments: Vec<Segment>,
}

#[derive(Debug)]
pub enum GraphSymbol {
    Stroke(i32, bool),
    Fill(i32),
}

impl Object {

    pub fn termination() -> Object {
        Object {
            object_type: ObjectType::Terminate,
            symbol_number: 0i32,
            segments: Vec::new(),
        }
    }

    fn empty_object(gsymbol: &GraphSymbol) -> Object {
        match gsymbol {
            GraphSymbol::Stroke(symbol_number, cornerize) => Object { 
                object_type: ObjectType::Line(*cornerize), 
                symbol_number: *symbol_number, segments: vec![],
            },
            GraphSymbol::Fill(symbol_number) => Object {
                object_type: ObjectType::Area,
                symbol_number: *symbol_number,
                segments: vec![],
            },
        }
    }

    fn push(&mut self, s: Segment) {
        self.segments.push(s)
    }

    fn polys(&self, angle: f64, middle: &Point) -> Vec<TDPoly> {

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
            let vx = (p.east - middle.east) / 0.15f64;
            let vy = (p.north - middle.north) / 0.15f64;
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

fn is_clockwise(p: &Vec<Point>) -> bool {
    p.windows(2).fold(0f64, |sum, pts| sum + pts[0].east*pts[1].north - pts[1].east*pts[0].north) < 0f64
}

pub fn post_objects_without_clipping(vertex_lists: Vec<Vec<Point>>, symbols: &Vec<GraphSymbol>, post_box: &Sender<Object>) {

    for symbol in symbols.iter() {
        let mut object = Object::empty_object(symbol); 

        for vertices in vertex_lists.iter() {   
            let mut segments: Vec<Segment> = vertices.iter().enumerate().map(|vertex| {
                match vertex.0 {
                    0 => Segment::Move(vertex.1.clone()),
                    _ => Segment::Line(vertex.1.clone()),
                }
            }).collect();
            object.segments.append(&mut segments);

            match object.object_type {
                ObjectType::Line(_) if object.segments.len() > 0 => { post_box.send(object).expect("Unable to send OSM object to OCAD."); object = Object::empty_object(symbol);},
                ObjectType::Line(_) => {
                    println!("Line, but no segment. Symbol {:?} {}",symbol, vertices.len());
                }
                _ => {}
            }
        }
        match object.object_type {
            ObjectType::Area if object.segments.len() > 0 => { post_box.send(object).expect("Unable to send OSM object to OCAD."); },
            _ => {}
        }
    }
}

pub fn post_objects(vertex_lists: Vec<Vec<Point>>, symbols: &Vec<GraphSymbol>, post_box: &Sender<Object>, bounding_box: &geometry::Rectangle) {

    let intersect = |segment: &geometry::LineSegment| { bounding_box.segments().into_iter()
        .filter_map(|edge| edge.intersection_with(&segment))
        .next()
        .expect("No intersection with bounding box edge") };

    for symbol in symbols.iter() {
        let mut object = Object::empty_object(symbol); 

        for vertices in vertex_lists.iter() {   

            let mut current_point: Option<Point> = None;
            let mut is_outside = true;

            for vertex in vertices.iter() {
                
                let this_is_outside = !bounding_box.contains(vertex);
                match (is_outside, this_is_outside) {
                    (false, true) => { // Going outside
                        let segment = geometry::LineSegment::create(&current_point.unwrap(), vertex);
                        object.push(Segment::Line(intersect(&segment)));

                        match object.object_type {
                            ObjectType::Line(_) => { post_box.send(object).expect("Unable to send OSM object to OCAD."); object = Object::empty_object(symbol);},
                            _ => {}
                        }
                    },
                    (true, false) => { // Going inside
                        if let Some(p) = current_point {
                            let segment = geometry::LineSegment::create(&p, vertex);
                            let intersect_point = intersect(&segment);
                            object.push(match symbol {
                                GraphSymbol::Stroke(_,_) => Segment::Move(intersect_point),
                                GraphSymbol::Fill(_) => Segment::Line(intersect_point),
                            });
                            object.push(Segment::Line(vertex.clone()));
                        } else {
                            object.push(Segment::Move(vertex.clone()));
                        }
                    },
                    (false,false) => { // Only inside
                        object.push(Segment::Line(vertex.clone()));
                    },
                    (true,true) => { continue }, // Only outside
                }
                current_point = Some(vertex.clone());
                is_outside = this_is_outside;
            }
            match object.object_type {
                ObjectType::Line(_) if object.segments.len() > 0 => { post_box.send(object).expect("Unable to send OSM object to OCAD."); object = Object::empty_object(symbol);},
                _ => {}
            }
        }
        match object.object_type {
            ObjectType::Area if object.segments.len() > 0 => { post_box.send(object).expect("Unable to send OSM object to OCAD."); },
            _ => {}
        }
    }
}

fn load_from_isom() -> (Vec<Vec<u8>>, Vec<Strings>) {
    let mut data = Cursor::new(SOFT_ISOM_2017);

    let header: RawFileHeader = read_instance(&mut data).expect("Unable to load ISOM file header.");

    let mut symbols: Vec<Vec<u8>> = Vec::new();

    let mut next_symbol_index: u64 = header.symbolindex.into();
    while next_symbol_index != 0 {
        data.seek(SeekFrom::Start(next_symbol_index)).expect("Unable to seek to start of symbol index.");
        let symbol_block: SymbolBlock = read_instance(&mut data).expect("Unable to load symbol block.");

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
        let string_block: StringIndexBlock = read_instance(&mut data).expect("Unable to load string index block.");

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

pub fn create(path: &PathBuf, bounding_box: &geometry::Rectangle, angle: f64, queue: &Receiver<Object>) {
    let (mut soft_symbols, mut soft_strings) = load_from_isom();

    let middle = bounding_box.middle();
    let m_string = format!("\tm15000.0000\tg0.0000\tr1\tx{:.8}\ty{:.8}\ta{:.8}", middle.east, middle.north, angle);

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

    let mut object_index = ObjectIndexBlock { nextindexblock: 0, indices: [ObjectIndex { 
        rc: LRect { lower_left: TDPoly { x: 0, y: 0 }, upper_right: TDPoly { x: 0, y: 0 } },
        position: 0, length: 0, symbol: 0, object_type: 0, encrypted_mose: 0, status: 0, viewtype: 0, color: 0, reserved1: 0, imported_layer: 0, reserved2: 0 }; 256] };
    let mut current_index = 0;
    let mut object_indices: Vec<ObjectIndexBlock> = Vec::new();

    loop {
        let object = queue.recv().expect("Unable to receive message on OCAD thread.");
        if object.object_type == ObjectType::Terminate { break; }

        let p = object.polys(angle, &middle);

        // Create Element, and fill out object index
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
            position: ftell(&mut file) as u32,
            length: (mem::size_of::<ObjectIndex>() + (mem::size_of::<TDPoly>()) * p.len()) as u32,
            symbol: element.symbol_number,
            object_type: element.object_type,
            encrypted_mose: 0u8,
            status: 1u8,
            viewtype: 0u8,
            color: 0u16,
            reserved1: 0u16, reserved2: 0u16, imported_layer: 0u16,
        };

        write_instance(&element, &mut file).expect("Unable to write OCAD element.");
        write_instances(&p, &mut file).expect("Unable to write TDPoly vector.");

        if current_index == 255 {
            object_indices.push(object_index);
            object_index = ObjectIndexBlock { nextindexblock: 0, indices: [ObjectIndex { 
                rc: LRect { lower_left: TDPoly { x: 0, y: 0 }, upper_right: TDPoly { x: 0, y: 0 } },
                position: 0, length: 0, symbol: 0, object_type: 0, encrypted_mose: 0, status: 0, viewtype: 0, color: 0, reserved1: 0, imported_layer: 0, reserved2: 0 }; 256] };
            current_index = 0
        } else {
            current_index = current_index+1;
        }
    }

    if current_index > 0 { object_indices.push(object_index); }

    header.objectindex = ftell(&mut file) as u32;
    for i in 0..(object_indices.len()-1) {
        object_indices[i].nextindexblock = header.objectindex + (mem::size_of::<ObjectIndexBlock>()*(i+1)) as u32;
    }

    write_instances(&object_indices, &mut file).expect("Unable to write object indices.");

    file.seek(SeekFrom::Start(0)).expect("Unable to seek back to beginning of file.");
    write_instance(&header, &mut file).expect("Unable to write OCAD header.");
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

#[derive(Copy,Clone)]
#[repr(C,packed)]
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

#[repr(C,packed)]
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
