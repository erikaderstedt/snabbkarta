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

enum GraphSymbol {
    Stroke(i32, bool),
    Fill(i32),
}

fn post_way(ways: Vec<Vec<&Node>>, symbols: &Vec<GraphSymbol>, post_box: &Sender<ocad::Object>, bounding_box: &Vec<geometry::LineSegment>) {
    let sw = &bounding_box[0].p0;
    let ne = &bounding_box[2].p0;
    let outside = |vertex: &Sweref| vertex.east < sw.east || vertex.east > ne.east || vertex.north > ne.north || vertex.north < sw.north;
    let intersect = |segment: &geometry::LineSegment| { bounding_box.iter()
        .filter_map(|edge| edge.intersection_with(&segment))
        .next()
        .expect("No intersection with bounding box edge") };

    for symbol in symbols.iter() {
        let mut segments = Vec::new();
        let finish_symbol = |segments: Vec<ocad::Segment>| -> Vec<ocad::Segment> {
            if segments.len() > 0 {
                post_box.send(
                    match symbol {
                        GraphSymbol::Stroke(symbol_number, cornerize) => ocad::Object { 
                            object_type: ocad::ObjectType::Line(*cornerize), 
                            symbol_number: *symbol_number, segments: segments,
                        },
                        GraphSymbol:: Fill(symbol_number) => ocad::Object {
                            object_type: ocad::ObjectType::Area,
                            symbol_number: *symbol_number,
                            segments: segments,
                        },
                    }
                ).expect("Unable to post OSM object to OCAD.");
            }
            vec![]
        };

        for way in ways.iter() {            
            let vertices = to_sweref(&way);

            let mut current_point: Option<Sweref> = None;
            let mut is_outside = true;

            for vertex in vertices.iter() {
                let this_is_outside = outside(vertex);
                match (is_outside, this_is_outside) {
                    (false, true) => { // Going outside
                        let segment = geometry::LineSegment::create(&current_point.unwrap(), vertex);
                        segments.push(ocad::Segment::Line(intersect(&segment)));

                        segments = finish_symbol(segments);
                    },
                    (true, false) => { // Going inside
                        if let Some(p) = current_point {
                            let segment = geometry::LineSegment::create(&p, vertex);
                            let intersect_point = intersect(&segment);
                            segments.push(match symbol {
                                GraphSymbol::Stroke(_,_) => ocad::Segment::Move(intersect_point),
                                GraphSymbol::Fill(_) => ocad::Segment::Line(intersect_point),
                            });
                            segments.push(ocad::Segment::Line(vertex.clone()));
                        } else {
                            segments.push(ocad::Segment::Move(vertex.clone()));
                        }
                    },
                    (false,false) => { // Only inside
                        segments.push(ocad::Segment::Line(vertex.clone()));
                    },
                    (true,true) => { continue }, // Only outside
                }
                current_point = Some(vertex.clone());
                is_outside = this_is_outside;
            }
            segments = finish_symbol(segments);
        }
        segments = finish_symbol(segments);

    }

}

pub fn load_osm(southwest: &Wgs84, northeast: &Wgs84, file: &Sender<ocad::Object>, verbose: bool) {
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

        let mut t = Vec::new();
        for tag in relation.tags.iter() {
            t = match (&tag.key[..], &tag.val[..]) {
                ("wires","single") => vec![GraphSymbol::Stroke(510000,true)],
                ("route","power") if t.len() == 0 => vec![GraphSymbol::Stroke(511000,true)],              
                ("wetland","bog") =>  vec![GraphSymbol::Stroke(415000,false), GraphSymbol::Fill(307000)],
                ("wetland","swamp") => vec![GraphSymbol::Fill(308000)],
                ("landuse","meadow") => vec![GraphSymbol::Stroke(415000,false), GraphSymbol::Fill(412000)],
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
        
        post_way(ways, &t, file, &bounding_box);
    }

    for (_, way) in doc.ways.iter() {
        let mut t = Vec::new();
        for tag in way.tags.iter() {
            t = match (&tag.key[..], &tag.val[..]) {
                ("landuse","residential") => vec![GraphSymbol::Stroke(520000,false), GraphSymbol::Fill(521001)],
                ("landuse","meadow") => vec![GraphSymbol::Stroke(415000,false), GraphSymbol::Fill(412000)],
                ("highway","path") => vec![GraphSymbol::Stroke(506000,false)],
                ("highway","track") => vec![GraphSymbol::Stroke(505000,false)],
                ("highway","tertiary") => vec![GraphSymbol::Stroke(502000,false)],
                ("highway","service") => vec![GraphSymbol::Stroke(504000,false)],
                ("highway","secondary") => vec![GraphSymbol::Stroke(502001,false)],
                ("highway","primary") => vec![GraphSymbol::Stroke(502002,false)],
                ("highway", _) => vec![GraphSymbol::Stroke(503000,false)],
                ("building",_) => vec![GraphSymbol::Fill(521000)],
                ("wetland","bog") => vec![GraphSymbol::Stroke(415000,false), GraphSymbol::Fill(307000)],
                ("wetland",_) => vec![GraphSymbol::Stroke(415000,false), GraphSymbol::Fill(308000)],
                ("power","line") => vec![GraphSymbol::Stroke(510000,true)],
                ("waterway","stream") => vec![GraphSymbol::Stroke(305000,false)],
                ("waterway","ditch") => vec![GraphSymbol::Stroke(306000,false)],
                _ => t,
            };
        }
        let resolved = resolve_way(way, &doc);

        post_way(vec![resolved], &t, file, &bounding_box);
    }
}