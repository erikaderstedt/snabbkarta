extern crate osm_xml as osm;
extern crate reqwest;
extern crate colored;

use std::sync::mpsc::Sender;
use super::sweref_to_wgs84::{Wgs84,Sweref};
use std::fs::{File};
use colored::*;
use std::io;
use osm::Node;
use super::ocad;
use super::geometry;

fn resolve_way<'a>(way: &'a osm::Way, doc: &'a osm::OSM) -> Vec<&'a Node> {
    way.nodes.iter().filter_map(|unresolved_reference| 
        match doc.resolve_reference(unresolved_reference) {
            osm::Reference::Node(n) => Some(n),
            _ => None,
        }).collect()
}

fn to_sweref(nodes: &Vec<&Node>) -> Vec<Sweref> {
    nodes.iter().map(|node| Sweref::from_wgs84( &Wgs84 { latitude: node.lat, longitude: node.lon } )).collect()
}

fn post_way(ways: Vec<Vec<&Node>>, symbols: (Option<u32>, Option<u32>, bool), post_box: &Sender<ocad::Object>, bounding_box: &Vec<geometry::LineSegment>) {
    let sw = &bounding_box[0].p0;
    let ne = &bounding_box[2].p0;
    let outside = |vertex: &Sweref| vertex.east < sw.east || vertex.east > ne.east || vertex.north > ne.north || vertex.north < sw.north;
    let intersect = |segment: &geometry::LineSegment| { bounding_box.iter()
        .filter_map(|edge| edge.intersection_with(&segment))
        .next()
        .expect("No intersection with bounding box edge") };

    if let Some(stroke_symbol_number) = symbols.0 {
        for way in ways.iter() {            
            let vertices = to_sweref(&way);
            if vertices.len() == 0 { continue }

            // if first vertex is outside, skip until *next* is inside. 
            let first_inside = match vertices.iter().enumerate().skip_while(|x| outside(x.1)).next() {
                Some(x) => x.0,
                None => continue, // No points are inside. We can skip the whole way.
            };
            let mut current_point = match first_inside {
                0 => vertices[0].clone(),
                i => {
                    let p0 = vertices[i-1].clone();
                    let p1 = vertices[i].clone();
                    let segment = geometry::LineSegment::create(&p0, &p1);
                    intersect(&segment)
                }
            };

            let mut segments = Vec::new();
            segments.push(ocad::Segment::Move(current_point.clone()));
            let mut is_outside = false;

            for vertex in vertices.iter().skip(first_inside) {
                let this_is_outside = outside(vertex);
                match (is_outside, this_is_outside) {
                    (false, true) => { // Going outside
                        let segment = geometry::LineSegment::create(&current_point, vertex);
                        segments.push(ocad::Segment::Line(intersect(&segment)));
                    },
                    (true, false) => { // Going inside
                        let segment = geometry::LineSegment::create(&current_point, vertex);
                        segments.push(ocad::Segment::Move(intersect(&segment)));
                        segments.push(ocad::Segment::Line(vertex.clone()));
                    },
                    (false,false) => { // Only inside
                        segments.push(ocad::Segment::Line(vertex.clone()));
                    },
                    (true,true) => { continue }, // Only outside
                }
                current_point = vertex.clone();
                is_outside = this_is_outside;
            }

            post_box.send( ocad::Object {
                object_type: ocad::ObjectType::Area,
                symbol_number: stroke_symbol_number,
                segments: segments,
            }).expect("Unable to post OSM object to OCAD.");
        }
    }

    if let Some(fill_symbol_number) = symbols.1 {
        // TODO: cut these too!
        
        // First way is the main, subsequent ways are holes.
        let mut segments = Vec::new();
        for way in ways.iter() {            
            let vertices = to_sweref(&way);
            let mut first = true;
            for v in vertices.into_iter() {
                segments.push(
                    match first {
                        true => ocad::Segment::Move(v),
                        false => ocad::Segment::Line(v),
                    }
                );
                first = false;
            }
        }
        post_box.send( ocad::Object {
            object_type: ocad::ObjectType::Area,
            symbol_number: fill_symbol_number,
            segments: segments,
        }).expect("Unable to post OSM object to OCAD.");
    }
}

pub fn load_osm(southwest: &Wgs84, northeast: &Wgs84, file: Sender<ocad::Object>, verbose: bool) {
    let module = "OSM".yellow();
    let cache_path = format!("{:.6}_{:.6}_{:.6}_{:.6}.osm-cache.xml", 
            southwest.latitude, southwest.longitude,
            northeast.latitude, northeast.longitude);
    let reader = match File::open(&cache_path) {
        Ok(r) => {
            if verbose { println!("[{}] Using cached OSM data", &module) };
            r
        },
        Err(e) => {
            println!("{}", e);
            let query = format!("node({},{},{},{})->.x;.x;way(bn);rel[wetland=swamp](bw)->.swamps;rel[wetland=bog](bw)->.bogs;rel[route=power](bw)->.pwr;rel[landuse=meadow](bw)->.meadows;.x;way[highway](bn)->.highways;way[building](bn)->.buildings;way[landuse=residential](bn)->.plots;way[landuse=meadow](bn)->.smallmeadows;way[wetland=swamp](bn)->.smallswamps;way[wetland=bog](bn)->.smallbogs;way[power=line](bn)->.smallpwr;way[waterway=ditch](bn)->.ditches;way[waterway=stream](bn)->.streams;( (  .swamps;.streams; .ditches;  .bogs;.pwr;.highways;.plots;.meadows;  .buildings;.smallswamps;  .smallmeadows;.smallbogs;.smallpwr;  ); >; );out;",
            southwest.latitude, southwest.longitude,
            northeast.latitude, northeast.longitude);

            let client  = reqwest::blocking::Client::new();
            let mut res = match client.post("https://lz4.overpass-api.de/api/interpreter")
                    .body(query)
                    .send() {
                Ok(r) => r,
                Err(e) => {
                    println!("[{}] OSM fetch error: {}", &module, e);
                    return
                },
            };
            { 
                let mut f = File::create(&cache_path).expect("Unable to create OSM cache path.");
                io::copy(&mut res, &mut f).expect("Unable to write to OSM cache.");
            };
            File::open(&cache_path).expect("Unable to open the cache I just wrote!")
        }
    };
    let doc = osm::OSM::parse(reader).expect("Unable to parse OSM file.");

    let sw = Sweref::from_wgs84(&southwest);
    let ne = Sweref::from_wgs84(&northeast);
    let bounding_box = geometry::LineSegment::segments_from_bounding_box(&sw, &ne);

    if verbose { println!("[{}] {} nodes, {} ways and {} relations", &module, doc.nodes.len(), doc.ways.len(), doc.relations.len()); }

    for (_, relation) in doc.relations.iter() {

        let mut t: Option<(Option<u32>, Option<u32>, bool)> = None;
        for tag in relation.tags.iter() {
            t = match (&tag.key[..], &tag.val[..]) {
                ("wires","single") => Some((Some(510000), None, true)),
                ("route","power") if t == None => Some((Some(511000), None, true)),                
                ("wetland","bog") => Some((Some(415000),Some(307000),false)),
                ("wetland","swamp") => Some((None, Some(308000), false)),
                ("landuse","meadow") => Some((Some(415000),Some(412000), false)),
                _ => t,
            };
        }

        let ways: Vec<Vec<&Node>> = relation.members.iter().filter_map(|member| 
            match member {
                osm::Member::Way(unres, _) => {
                    match doc.resolve_reference(&unres) {
                        osm::Reference::Way(way) => Some(resolve_way(&way, &doc)),
                        _ => None,
                    }
                },
                _ => None,
            })
            .collect();
        
        if let Some(symbols) = t { post_way(ways, symbols, &file, &bounding_box); }
    }

    for (_, way) in doc.ways.iter() {
        let mut t: Option<(Option<u32>, Option<u32>, bool)> = None;
        for tag in way.tags.iter() {
            t = match (&tag.key[..], &tag.val[..]) {
                ("landuse","residential") => Some((Some(520000), Some(521001), false)),
                ("landuse","meadow") => Some((Some(415000), Some(412000), false)),
                ("highway","path") => Some((Some(506000), None, false)),
                ("highway","track") => Some((Some(505000), None, false)),
                ("highway","tertiary") => Some((Some(502000), None, false)),
                ("highway","service") => Some((Some(504000), None, false)),
                ("highway","secondary") =>Some((Some(502001), None, false)),
                ("highway","primary") => Some((Some(502002), None, false)),
                ("highway", _) => Some((Some(503000), None, false)),
                ("building",_) => Some((None, Some(521000), false)),
                ("water","lake") => None,
                ("wetland","bog") => Some((Some(415000), Some(307000), false)),
                ("wetland",_) => Some((Some(415000), Some(308000), false)),
                ("power","line") => Some((Some(510000), None, true)),
                ("waterway","stream") => Some((Some(305000), None, false)),
                ("waterway","ditch") => Some((Some(306000), None, false)),
                _ => t,
            };
        }
        let resolved = resolve_way(way, &doc);

        if let Some(symbols) = t { post_way(vec![resolved], symbols, &file, &bounding_box); }
    }
    // Ok(r) => {
    //         
    // }
}