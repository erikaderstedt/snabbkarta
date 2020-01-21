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

fn resolve_way<'a>(way: &'a osm::Way, doc: &'a osm::OSM) -> Vec<&'a Node> {
    way.nodes.iter().filter_map(|unresolved_reference| 
        match doc.resolve_reference(unresolved_reference) {
            osm::Reference::Node(n) => Some(n),
            _ => None,
        }).collect()
}

fn polys_from_nodes(nodes: &Vec<&Node>) -> Vec<Sweref> {
    nodes.iter().map(|node| Sweref::from_wgs84( &Wgs84 { latitude: node.lat, longitude: node.lon } )).collect()
}

fn post_way(ways: Vec<Vec<&Node>>, symbols: (Option<u32>, Option<u32>, bool), post_box: &mut Sender<ocad::Object>, sw: Sweref, ne: Sweref) {

    let outside = |vertex: &Sweref| vertex.east < sw.east || vertex.east > ne.east || vertex.north > ne.north || vertex.north < sw.north;

    if let Some(stroke_symbol_number) = symbols.0 {
        // First is outer, subsequent ones are inner. 
        for way in ways.iter() {            
            let vertices = polys_from_nodes(&way);
            let mut v = vertices.iter().enumerate().skip_while(|x| outside(x.1));
            

            let clipped_vertices = vertices.iter().enumerate().filter_map(|vertex_info|

                

            // If vertex is inside
        }
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

    let sw = Sweref::from_wgs84(&soutwest);
    let ne = Sweref::from_wgs84(&northeast);

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
        
        if let Some(symbols) = t { post_way(ways, symbols, &mut file); }
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

        if let Some(symbols) = t { post_way(vec![way], symbols, &mut file); }
    }
    // Ok(r) => {
    //         
    // }
}