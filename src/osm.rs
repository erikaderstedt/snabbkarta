extern crate osm_xml as osm;
extern crate minreq;
extern crate colored;

use std::sync::mpsc::Sender;
use super::sweref_to_wgs84::{Wgs84,Sweref};
use std::fs::{File};
use std::io::Write;
use colored::*;
use osm::Node;
use super::ocad::{self, GraphSymbol};
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

fn post_way(ways: Vec<Vec<&Node>>, symbols: &Vec<ocad::GraphSymbol>, post_box: &Sender<ocad::Object>, bounding_box: &geometry::Rectangle) {
    let pts: Vec<Vec<Sweref>> = ways.iter().map(|w| to_sweref(w)).collect();

    ocad::post_objects(pts, symbols, post_box, bounding_box);
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
        Err(_) => {
            let query = format!("node({},{},{},{})->.x;.x;way(bn);rel[wetland=swamp](bw)->.swamps;rel[wetland=bog](bw)->.bogs;rel[route=power](bw)->.pwr;rel[landuse=meadow](bw)->.meadows;.x;way[highway](bn)->.highways;way[building](bn)->.buildings;way[landuse=residential](bn)->.plots;way[landuse=meadow](bn)->.smallmeadows;way[wetland=swamp](bn)->.smallswamps;way[wetland=bog](bn)->.smallbogs;way[power=line](bn)->.smallpwr;way[waterway=ditch](bn)->.ditches;way[waterway=stream](bn)->.streams;( (  .swamps;.streams; .ditches;  .bogs;.pwr;.highways;.plots;.meadows;  .buildings;.smallswamps;  .smallmeadows;.smallbogs;.smallpwr;  ); >; );out;",
            southwest.latitude, southwest.longitude,
            northeast.latitude, northeast.longitude);

            let mut res = match minreq::post("https://lz4.overpass-api.de/api/interpreter")
                .with_body(query)
                .send() {
                Ok(r) => r,
                Err(e) => {
                    println!("[{}] OSM fetch error: {}", &module, e);
                    return
                },
            };
            { 
                let mut f = File::create(&cache_path).expect("Unable to create OSM cache path.");
                f.write(res.as_bytes());
            };
            File::open(&cache_path).expect("Unable to open the cache I just wrote!")
        }
    };
    let doc = osm::OSM::parse(reader).expect("Unable to parse OSM file.");

    let bounding_box = geometry::Rectangle { southwest: Sweref::from_wgs84(&southwest), northeast: Sweref::from_wgs84(&northeast), };
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
                ("landuse","residential") => vec![GraphSymbol::Stroke(520001,false), GraphSymbol::Fill(520000)],
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
    if verbose {
        println!("[{}] OSM complete.", &module);
    }
}